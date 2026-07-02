use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use linguafranca::anthropic::convert::stream::{
    AnthropicMessagesToOpenResponsesStream, OpenResponsesToAnthropicMessagesStream,
};
use linguafranca::anthropic::stream::AnthropicStreamEvent;
use linguafranca::chat_completions_openai::convert::stream::{
    ChatCompletionsToOpenResponsesStream, OpenResponsesToChatCompletionsStream,
};
use linguafranca::chat_completions_openai::stream::ChatCompletionsStreamChunk;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext, StreamSharedState};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::stream_response::{
    self, SseEvent, SseParser, StreamConfig, StreamResponse, StreamState,
};
use crate::gateway::pipeline::StageError;
use super::build_upstream_url;
use super::mask_api_key;

fn is_streaming_body(ctx: &RequestContext) -> bool {
    if let Some(json) = &ctx.parsed_body {
        json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        let body_str = String::from_utf8_lossy(&ctx.request_body);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
            json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        }
    }
}

/// 请求转发入口：自动判断流式/非流式
pub async fn run(
    runtime: &GatewayContext,
    ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();
    let provider = ctx.provider.as_ref().cloned().ok_or_else(|| {
        StageError::new(
            error_ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    let upstream_url = if let Some(ref url) = ctx.upstream_url {
        reqwest::Url::parse(url).map_err(|err| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::BadRequest(format!("无效的上游地址: {err}")),
            )
        })?
    } else {
        build_upstream_url(&provider.api_base_url, &ctx.uri)
            .map_err(|error| StageError::new(error_ctx.clone(), error))?
    };

    // 使用 GatewayContext 中的共享客户端，避免每请求创建新 TLS 连接
    let is_streaming = is_streaming_body(&ctx);
    let client = if is_streaming {
        &runtime.http_client_streaming
    } else {
        &runtime.http_client
    };

    let reqwest_method = if let Some(ref method) = ctx.upstream_method {
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|err| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::BadRequest(format!("无效的上游方法: {err}")),
            )
        })?
    } else {
        reqwest::Method::from_bytes(ctx.method.as_str().as_bytes()).map_err(|err| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::BadRequest(format!("不支持的方法: {err}")),
            )
        })?
    };

    // 保存 URL 和方法的字符串形式（用于日志，因为后续会被 move）
    let url_str = upstream_url.to_string();
    let method_str = reqwest_method.as_str().to_string();

    let mut upstream_request = client.request(reqwest_method, upstream_url);
    // 应用适配器生成的上游请求头（API Key、Content-Type 等）
    if let Some(ref adapter_headers) = ctx.upstream_headers {
        for (name, value) in adapter_headers.iter() {
            upstream_request = upstream_request.header(name, value);
        }
    }
    // 转发客户端头（使用 HeaderConfig 配置）
    let header_config = crate::gateway::header_config::HeaderConfig::default();
    for (name, value) in ctx.headers.iter() {
        // 跳过已经被适配器设置的 header
        if ctx.upstream_headers.as_ref().map_or(false, |h| h.contains_key(name)) {
            continue;
        }
        
        // 使用配置决定是否转发
        if header_config.should_forward(name.as_str()) {
            upstream_request = upstream_request.header(name, value);
        }
    }

    let max_retries = provider.max_retries as u32;
    let stream_config = StreamConfig {
        max_retries,
        ..Default::default()
    };

    let mut last_error = None;

    // 调试日志：输出实际上游请求信息
    {
        let masked_key = ctx
            .selected_api_key
            .as_deref()
            .map(mask_api_key)
            .unwrap_or_default();
        let body_preview = String::from_utf8_lossy(&ctx.request_body)
            .chars().take(200).collect::<String>();
        tracing::debug!(
            url = %url_str,
            method = %method_str,
            api_key = %masked_key,
            body_len = ctx.request_body.len(),
            body_preview = %body_preview,
            "转发上游请求"
        );
    }

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let backoff = calculate_backoff(attempt, &stream_config);
            tokio::time::sleep(backoff).await;

            // SSE 断线重连：添加 Last-Event-ID
            let last_event_id = ctx.last_event_id.clone();
            if let Some(ref event_id) = last_event_id {
                upstream_request = upstream_request.header("Last-Event-ID", event_id);
            }
        }

        let request_clone = upstream_request.try_clone().ok_or_else(|| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::Internal("请求不可克隆".to_string()),
            )
        })?;

        // LeastConn 连接追踪：请求开始
        if let Some(ref member) = ctx.selected_group_member {
            runtime.group_manager.connection_started(member).await;
        }

        let result = request_clone.body(ctx.request_body.clone()).send().await;

        // LeastConn 连接追踪：请求结束
        if let Some(ref member) = ctx.selected_group_member {
            runtime.group_manager.connection_finished(member).await;
        }

        match result {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                tracing::debug!(
                    status = %status,
                    attempt = attempt,
                    "收到上游响应"
                );

                // 全部强制走流式 SSE 路径
                return handle_sse_response(
                    ctx, response, headers, provider, &stream_config,
                )
                .await;
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    // 所有重试都失败，返回最后一条错误（或兜底错误消息）
    let final_error = match last_error {
        Some(err) => GatewayError::UpstreamError {
            status: 0,
            body: serde_json::json!({"error": {"message": err.to_string(), "type": "upstream_error"}}),
        },
        None => GatewayError::Internal("上游请求失败（无详细错误）".to_string()),
    };
    Err(StageError::new(
        error_ctx,
        final_error,
    ))
}

/// SSE 流式响应处理
///
/// 架构：
/// 1. 创建 shared state（bytes_sent / last_event_id）用于读取任务与主线程同步
/// 2. 创建 mpsc channel 逐 chunk 推送数据
/// 3. 后台任务读取上游 → SSE 解析 → 更新 shared state → 推送 chunk
/// 4. 主线程从 channel 接收 → 构建 StreamBody → 返回响应
/// 5. 断线重连时携带 Last-Event-ID
///
/// 注意：流式场景下不做协议转换（chunk 级别的增量数据无法用 transform_response 处理）。
/// 同协议流转（inbound == outbound）直接透传；跨协议流式转发暂不支持转换。
async fn handle_sse_response(
    mut ctx: RequestContext,
    response: reqwest::Response,
    headers: axum::http::HeaderMap,
    provider: crate::models::Provider,
    config: &StreamConfig,
) -> Result<RequestContext, StageError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, GatewayError>>(256);
    let shared = Arc::new(RwLock::new(StreamSharedState::default()));
    let mut stream_state = StreamState::new();

    // 判断是否需要流式协议转换
    let inbound = ctx.inbound_protocol.clone().unwrap_or_default();
    let outbound = ctx.outbound_protocol.clone().unwrap_or_default();
    let inbound_clone = inbound.clone();
    let outbound_clone = outbound.clone();

    // 启动后台读取任务
    let response_stream = response.bytes_stream();
    let stream_config = config.clone();
    let shared_for_task = shared.clone();
    let _read_task = tokio::spawn(async move {
        let mut parser = SseParser::new();
        let mut pinned_stream = std::pin::pin!(response_stream);
        let mut heartbeat = tokio::time::interval(stream_config.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // 创建协议转换器
        let mut converter = SseConverter::new(&inbound_clone, &outbound_clone);

        loop {
            tokio::select! {
                chunk = pinned_stream.next() => {
                    match chunk {
                        Some(Ok(bytes)) => {
                            stream_state.record_data(bytes.len());

                            // 解析 SSE 事件，追踪 last_event_id
                            let events = parser.feed(&bytes);
                            if events.is_empty() {
                                // 不完整 chunk，直接转发
                                if tx.send(Ok(bytes)).await.is_err() { return; }
                                continue;
                            }

                            let mut output = Vec::new();

                            for event in &events {
                                stream_state.record_event();

                                // 更新 last_event_id
                                if let Some(ref id) = event.id {
                                    let mut state = shared_for_task.write().await;
                                    state.last_event_id = Some(id.clone());
                                }

                                if event.is_end() {
                                    let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                                    return;
                                }

                                // 协议转换（流式场景按事件逐条转换）
                                match converter.convert(event) {
                                    Ok(bytes) => output.extend_from_slice(&bytes),
                                    Err(e) => {
                                        tracing::warn!("流式协议转换失败: {e}");
                                        // 转换失败时透传原始事件
                                        output.extend_from_slice(event.serialize().as_bytes());
                                    }
                                }
                            }

                            // 更新已发送字节数
                            {
                                let mut state = shared_for_task.write().await;
                                state.bytes_sent += output.len() as u64;
                            }

                            if tx.send(Ok(Bytes::from(output))).await.is_err() {
                                return;
                            }
                        }
                        Some(Err(err)) => {
                            let _ = tx.send(Err(GatewayError::Upstream(err))).await;
                            return;
                        }
                        None => {
                            if !stream_state.ended {
                                let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                            }
                            return;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if stream_state.is_timed_out(stream_config.stream_timeout) {
                        let _ = tx.send(Err(GatewayError::Timeout)).await;
                        return;
                    }
                    if tx.send(Ok(stream_response::heartbeat_comment())).await.is_err() {
                        return;
                    }
                }
            }
        }
    });

    // 构建流式响应
    let stream_response = StreamResponse::Sse {
        status: ctx.upstream_status.unwrap_or(axum::http::StatusCode::OK),
        headers: headers.clone(),
        stream: Box::new(tokio_stream::wrappers::ReceiverStream::new(rx)),
    };

    ctx.provider = Some(provider);
    ctx.upstream_status = Some(axum::http::StatusCode::OK);
    ctx.upstream_headers = Some(headers);
    ctx.upstream_body = None;
    ctx.response = Some(stream_response.into_response());

    Ok(ctx)
}

/// 流式 SSE 事件协议转换器
///
/// 转换架构（hub = OpenResponsesStreamEvent）：
///   openai_chat    ──ChatToHub──→  hub  ──HubToChat──→  openai_chat
///   claude_messages ──ClaudeToHub→  hub  ──HubToClaude→ claude_messages
///   openai_response                  hub                openai_response
///
/// - 出站为 openai_response：数据已是 hub，跳过 outbound→hub
/// - 入站为 openai_response：hub 已是最终格式，跳过 hub→inbound
struct SseConverter {
    /// outbound → hub 转换器（None 表示出站已是 hub 格式）
    outbound_to_hub: Option<OutboundToHubConverter>,
    /// hub → inbound 转换器（None 表示入站已是 hub 格式）
    hub_to_inbound: Option<HubToInboundConverter>,
}

enum OutboundToHubConverter {
    ChatCompletionsToOpenResponses(ChatCompletionsToOpenResponsesStream),
    AnthropicToOpenResponses(AnthropicMessagesToOpenResponsesStream),
}

enum HubToInboundConverter {
    OpenResponsesToChatCompletions(OpenResponsesToChatCompletionsStream),
    OpenResponsesToAnthropic(OpenResponsesToAnthropicMessagesStream),
}

impl SseConverter {
    fn new(inbound: &str, outbound: &str) -> Self {
        if inbound == outbound || inbound.is_empty() || outbound.is_empty() {
            return Self { outbound_to_hub: None, hub_to_inbound: None };
        }
        Self {
            outbound_to_hub: match outbound {
                "openai_chat" => Some(OutboundToHubConverter::ChatCompletionsToOpenResponses(
                    ChatCompletionsToOpenResponsesStream::new(),
                )),
                "claude_messages" => Some(OutboundToHubConverter::AnthropicToOpenResponses(
                    AnthropicMessagesToOpenResponsesStream::new(),
                )),
                // openai_response: 已是 hub 格式，无需转换
                _ => None,
            },
            hub_to_inbound: match inbound {
                "openai_chat" => Some(HubToInboundConverter::OpenResponsesToChatCompletions(
                    OpenResponsesToChatCompletionsStream::new(),
                )),
                "claude_messages" => Some(HubToInboundConverter::OpenResponsesToAnthropic(
                    OpenResponsesToAnthropicMessagesStream::new(),
                )),
                // openai_response: hub 已是最终格式，无需转换
                _ => None,
            },
        }
    }

    fn convert(&mut self, event: &SseEvent) -> Result<Bytes, String> {
        let data = match &event.data {
            Some(d) => d,
            None => return Ok(Bytes::from(event.serialize())),
        };
        let json: serde_json::Value =
            serde_json::from_str(data).map_err(|e| format!("解析 SSE data JSON 失败: {e}"))?;

        // Step 1: 解析出站事件 → hub 事件
        let hub_events = if self.outbound_to_hub.is_some() {
            self.outbound_to_hub_events(&json)?
        } else {
            // 出站已是 hub 格式（openai_response），直接反序列化
            let ev: OpenResponsesStreamEvent = serde_json::from_value(json)
                .map_err(|e| format!("解析 OpenResponses SSE event 失败: {e}"))?;
            vec![ev]
        };

        // Step 2: hub 事件 → 入站格式
        if self.hub_to_inbound.is_some() {
            let mut output = Vec::new();
            for hub_event in &hub_events {
                let bytes = self.hub_to_inbound_events(hub_event)?;
                output.extend(bytes);
            }
            Ok(Bytes::from(output))
        } else {
            // 入站已是 hub 格式（openai_response），直接序列化为 SSE
            serialize_open_responses_events(&hub_events)
        }
    }

    fn outbound_to_hub_events(
        &mut self,
        json: &serde_json::Value,
    ) -> Result<Vec<OpenResponsesStreamEvent>, String> {
        match self.outbound_to_hub.as_mut().unwrap() {
            OutboundToHubConverter::ChatCompletionsToOpenResponses(c) => {
                let chunk: ChatCompletionsStreamChunk = serde_json::from_value(json.clone())
                    .map_err(|e| format!("解析 OpenAI Chat SSE chunk 失败: {e}"))?;
                c.transform(chunk)
                    .map_err(|e| format!("OpenAI Chat → OpenResponses 转换失败: {e}"))
            }
            OutboundToHubConverter::AnthropicToOpenResponses(c) => {
                let event: AnthropicStreamEvent = serde_json::from_value(json.clone())
                    .map_err(|e| format!("解析 Anthropic SSE event 失败: {e}"))?;
                c.transform(event)
                    .map_err(|e| format!("Anthropic → OpenResponses 转换失败: {e}"))
            }
        }
    }

    fn hub_to_inbound_events(
        &mut self,
        hub_event: &OpenResponsesStreamEvent,
    ) -> Result<Vec<u8>, String> {
        match self.hub_to_inbound.as_mut().unwrap() {
            HubToInboundConverter::OpenResponsesToChatCompletions(c) => {
                let out: Vec<ChatCompletionsStreamChunk> = c
                    .transform(hub_event.clone())
                    .map_err(|e| format!("OpenResponses → OpenAI Chat 转换失败: {e}"))?;
                let mut bytes = Vec::new();
                for chunk in out {
                    let s = serde_json::to_string(&chunk)
                        .map_err(|e| format!("序列化 OpenAI Chat chunk 失败: {e}"))?;
                    bytes.extend_from_slice(b"data: ");
                    bytes.extend_from_slice(s.as_bytes());
                    bytes.extend_from_slice(b"\n\n");
                }
                Ok(bytes)
            }
            HubToInboundConverter::OpenResponsesToAnthropic(c) => {
                let out: Vec<AnthropicStreamEvent> = c
                    .transform(hub_event.clone())
                    .map_err(|e| format!("OpenResponses → Anthropic 转换失败: {e}"))?;
                let mut bytes = Vec::new();
                for event in out {
                    let json_str = serde_json::to_string(&event)
                        .map_err(|e| format!("序列化 Anthropic event 失败: {e}"))?;
                    if let Some(et) = extract_anthropic_event_type(&event) {
                        bytes.extend_from_slice(b"event: ");
                        bytes.extend_from_slice(et.as_bytes());
                        bytes.extend_from_slice(b"\n");
                    }
                    bytes.extend_from_slice(b"data: ");
                    bytes.extend_from_slice(json_str.as_bytes());
                    bytes.extend_from_slice(b"\n\n");
                }
                Ok(bytes)
            }
        }
    }
}

/// 提取 AnthropicStreamEvent 的 type 字段值（用于 SSE event: 行）
fn extract_anthropic_event_type(event: &AnthropicStreamEvent) -> Option<String> {
    let json = serde_json::to_value(event).ok()?;
    json.get("type").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 将 OpenResponsesStreamEvent 列表序列化为 SSE 文本
fn serialize_open_responses_events(events: &[OpenResponsesStreamEvent]) -> Result<Bytes, String> {
    let mut bytes = Vec::new();
    for event in events {
        let json_val = serde_json::to_value(event)
            .map_err(|e| format!("序列化 OpenResponses event 失败: {e}"))?;
        let et = json_val
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let json_str = serde_json::to_string(&json_val)
            .map_err(|e| format!("JSON to string 失败: {e}"))?;
        if let Some(et) = et {
            bytes.extend_from_slice(b"event: ");
            bytes.extend_from_slice(et.as_bytes());
            bytes.extend_from_slice(b"\n");
        }
        bytes.extend_from_slice(b"data: ");
        bytes.extend_from_slice(json_str.as_bytes());
        bytes.extend_from_slice(b"\n\n");
    }
    Ok(Bytes::from(bytes))
}

/// 计算指数退避时间
fn calculate_backoff(attempt: u32, config: &StreamConfig) -> Duration {
    let base = config.initial_backoff.as_millis() as u64;
    let multiplier = 2u64.pow(attempt - 1);
    let backoff_ms = (base * multiplier).min(config.max_backoff.as_millis() as u64);
    Duration::from_millis(backoff_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_backoff() {
        let config = StreamConfig::default();
        assert_eq!(calculate_backoff(1, &config), Duration::from_millis(500));
        assert_eq!(calculate_backoff(2, &config), Duration::from_millis(1000));
        assert_eq!(calculate_backoff(3, &config), Duration::from_millis(2000));
        assert_eq!(calculate_backoff(4, &config), Duration::from_millis(4000));
        assert_eq!(calculate_backoff(5, &config), Duration::from_millis(8000));
        assert_eq!(calculate_backoff(10, &config), Duration::from_millis(8000));
    }
}
