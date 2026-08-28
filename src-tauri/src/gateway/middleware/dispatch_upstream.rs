use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext, StreamSharedState};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::stream_response::{
    self, SseConverter, SseEvent, SseParser, StreamConfig, StreamResponse, StreamState,
};
use crate::gateway::pipeline::StageError;
use super::build_upstream_url;
use super::mask_api_key;

/// 请求转发入口：统一以流式 SSE 方式转发上游请求
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

    // transform_request 已注入 stream:true，统一使用流式客户端
    let client = &runtime.http_client_streaming;

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
        if ctx
            .upstream_headers
            .as_ref()
            .is_some_and(|h| h.contains_key(name))
        {
            continue;
        }
        
        // 使用配置决定是否转发
        if header_config.should_forward(name.as_str()) {
            upstream_request = upstream_request.header(name, value);
        }
    }
    // 应用 Provider 自定义请求头（覆盖适配器头和转发头）
    if let Some(ref provider) = ctx.provider {
        let custom_headers = provider.custom_headers_vec();
        for entry in &custom_headers {
            if entry.enabled && !entry.name.is_empty() {
                upstream_request = upstream_request.header(&entry.name, &entry.value);
            }
        }
    }

    let max_retries = provider.max_retries as u32;
    let client_requested_stream = ctx.client_requested_stream;
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

        let result = request_clone.body(ctx.request_body.clone()).send().await;

        match result {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                tracing::debug!(
                    status = %status,
                    attempt = attempt,
                    "收到上游响应"
                );

                if !status.is_success() {
                    return handle_upstream_error(ctx, response, headers).await;
                }

                return handle_sse_response(ctx, response, headers, provider, &stream_config, client_requested_stream)
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


async fn handle_upstream_error(
    mut ctx: RequestContext,
    response: reqwest::Response,
    headers: axum::http::HeaderMap,
) -> Result<RequestContext, StageError> {
    let status = response.status();
    let body = response.bytes().await.unwrap_or_else(|err| {
        bytes::Bytes::from(
            serde_json::json!({
                "error": {
                    "message": err.to_string(),
                    "type": "upstream_error"
                }
            })
            .to_string(),
        )
    });
    let parsed_body = serde_json::from_slice(&body).unwrap_or_else(|_| {
        serde_json::json!({
            "error": {
                "message": String::from_utf8_lossy(&body).chars().take(500).collect::<String>(),
                "type": "upstream_error"
            }
        })
    });

    ctx.upstream_status = Some(status);
    ctx.upstream_headers = Some(headers);
    ctx.upstream_body = Some(body);

    Err(StageError::new(
        ctx,
        GatewayError::UpstreamError {
            status: status.as_u16(),
            body: parsed_body,
        },
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
///
/// 当客户端未请求流式（client_requested_stream = false）时，收集所有 SSE 事件
/// 并聚合为单个 JSON 响应返回，避免非流式客户端收到 SSE 流产生重复响应。
async fn handle_sse_response(
    mut ctx: RequestContext,
    response: reqwest::Response,
    headers: axum::http::HeaderMap,
    provider: crate::models::Provider,
    config: &StreamConfig,
    client_requested_stream: bool,
) -> Result<RequestContext, StageError> {
    if !client_requested_stream {
        return handle_nonstreaming_aggregate(ctx, response, headers, provider, config).await;
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, GatewayError>>(256);
    let shared = Arc::new(RwLock::new(StreamSharedState::default()));
    let mut stream_state = StreamState::new();
    let status = response.status();

    // 判断是否需要流式协议转换
    let inbound = ctx.inbound_protocol.clone().unwrap_or_default();
    let outbound = ctx.outbound_protocol.clone().unwrap_or_default();
    let inbound_clone = inbound.clone();
    let outbound_clone = outbound.clone();

    // 流结束通知通道：后台任务 → pipeline
    let (complete_tx, complete_rx) = tokio::sync::oneshot::channel::<()>();

    // 启动后台读取任务
    let response_stream = response.bytes_stream();
    let stream_config = config.clone();
    let shared_for_task = shared.clone();
    let stream_start = std::time::Instant::now();
    let _read_task = tokio::spawn(async move {
        let mut parser = SseParser::new();
        let mut pinned_stream = std::pin::pin!(response_stream);
        let mut heartbeat = tokio::time::interval(stream_config.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // 创建协议转换器
        let mut converter = SseConverter::new(&inbound_clone, &outbound_clone);
        let mut total_events: u64 = 0;

        loop {
            tokio::select! {
                chunk = pinned_stream.next() => {
                    match chunk {
                        Some(Ok(bytes)) => {
                            stream_state.record_data(bytes.len());

                            // 追踪接收字节数
                            {
                                let mut state = shared_for_task.write().await;
                                state.bytes_received += bytes.len() as u64;
                            }

                            // 解析 SSE 事件，追踪 last_event_id
                            let events = parser.feed(&bytes);
                            if events.is_empty() {
                                // 不完整 chunk：可能是部分 SSE 事件、纯注释行、或空行。
                                // 不能直接转发原始字节——否则上游心跳注释（": xxx"）或
                                // 不完整 JSON 会与 silk 自己的 keep-alive 合并，产生
                                // "\:" 等非法 JSON 转义。丢弃即可，完整事件会在后续
                                // chunk 中由 parser 重组后走正常转换路径。
                                continue;
                            }

                            let mut output = Vec::new();

                            for event in &events {
                                stream_state.record_event();
                                total_events += 1;

                                // 使用规范化结构提取 usage 信息
                                let stream_event = converter.normalize(event);
                                if let Some(usage) = stream_event.extract_usage() {
                                    let mut state = shared_for_task.write().await;
                                    if let Some(input_tokens) = usage.input_tokens {
                                        state.exact_prompt_tokens = Some(input_tokens);
                                    }
                                    if let Some(output_tokens) = usage.output_tokens {
                                        state.exact_completion_tokens = Some(output_tokens);
                                    }
                                }

                                // 更新 last_event_id
                                if let Some(ref id) = event.id {
                                    let mut state = shared_for_task.write().await;
                                    state.last_event_id = Some(id.clone());
                                }

                                if event.is_end() {
                                    // 冲刷转换器收尾事件（如 anthropic message_stop）
                                    if let Ok(bytes) = converter.finish() {
                                        if !bytes.is_empty() {
                                            let _ = tx.send(Ok(bytes)).await;
                                        }
                                    }
                                    let elapsed = stream_start.elapsed();
                                    let state = shared_for_task.read().await;
                                    tracing::info!(
                                        total_events,
                                        bytes_received = state.bytes_received,
                                        bytes_sent = state.bytes_sent,
                                        elapsed_ms = elapsed.as_millis(),
                                        prompt_tokens = state.exact_prompt_tokens.unwrap_or(0),
                                        completion_tokens = state.exact_completion_tokens.unwrap_or(0),
                                        "SSE 流正常结束"
                                    );
                                    let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                                    let _ = complete_tx.send(());
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
                                break;
                            }
                        }
                        Some(Err(err)) => {
                            let elapsed = stream_start.elapsed();
                            tracing::warn!(
                                error = %err,
                                total_events,
                                elapsed_ms = elapsed.as_millis(),
                                "SSE 流上游错误"
                            );
                            let _ = tx.send(Err(GatewayError::Upstream(err))).await;
                            let _ = complete_tx.send(());
                            return;
                        }
                        None => {
                            let elapsed = stream_start.elapsed();
                            tracing::info!(
                                total_events,
                                elapsed_ms = elapsed.as_millis(),
                                "SSE 流连接关闭（无 [DONE] 标记）"
                            );
                            if !stream_state.ended {
                                let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                            }
                            let _ = complete_tx.send(());
                            return;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if stream_state.is_timed_out(stream_config.stream_timeout) {
                        let elapsed = stream_start.elapsed();
                        tracing::warn!(
                            total_events,
                            elapsed_ms = elapsed.as_millis(),
                            timeout_secs = stream_config.stream_timeout.as_secs(),
                            "SSE 流超时"
                        );
                        let _ = tx.send(Err(GatewayError::Timeout)).await;
                        let _ = complete_tx.send(());
                        return;
                    }
                    if tx.send(Ok(stream_response::heartbeat_comment())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = complete_tx.send(());
    });

    // 构建流式响应
    let stream_response = StreamResponse::Sse {
        status,
        headers: headers.clone(),
        stream: Box::new(tokio_stream::wrappers::ReceiverStream::new(rx)),
    };

    ctx.provider = Some(provider);
    ctx.upstream_status = Some(status);
    ctx.upstream_headers = Some(headers);
    ctx.upstream_body = None;
    ctx.stream_shared = Some(shared);
    ctx.stream_complete_rx = Some(complete_rx);
    ctx.response = Some(stream_response.into_response());

    Ok(ctx)
}

/// 非流式响应聚合：收集所有 SSE 事件，聚合为单个 JSON 响应。
///
/// 当客户端未请求流式时使用。网关强制上游走流式（transform_request 注入 stream:true），
/// 但客户端期望单个 JSON 响应。此函数在后台收集所有 SSE chunk，通过 prism 转换后
/// 聚合为完整的 chat.completion JSON。
async fn handle_nonstreaming_aggregate(
    mut ctx: RequestContext,
    response: reqwest::Response,
    _headers: axum::http::HeaderMap,
    provider: crate::models::Provider,
    _config: &StreamConfig,
) -> Result<RequestContext, StageError> {
    let shared = Arc::new(RwLock::new(StreamSharedState::default()));
    let status = response.status();

    // 判断是否需要流式协议转换
    let inbound = ctx.inbound_protocol.clone().unwrap_or_default();
    let outbound = ctx.outbound_protocol.clone().unwrap_or_default();

    // 后台收集任务：读取所有 SSE 事件
    let response_stream = response.bytes_stream();
    let shared_for_task = shared.clone();
    let stream_start = std::time::Instant::now();

    let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Bytes>();

    let _read_task = tokio::spawn(async move {
        let mut parser = SseParser::new();
        let mut converter = SseConverter::new(&inbound, &outbound);
        let mut collected_events: Vec<SseEvent> = Vec::new();
        let mut pinned_stream = std::pin::pin!(response_stream);
        let mut total_events: u64 = 0;

        loop {
            match pinned_stream.next().await {
                Some(Ok(bytes)) => {
                    {
                        let mut state = shared_for_task.write().await;
                        state.bytes_received += bytes.len() as u64;
                    }

                    let events = parser.feed(&bytes);
                    for event in events {
                        total_events += 1;

                        // 提取 token 用量
                        if let Some(ref data) = event.data {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                let mut state = shared_for_task.write().await;
                                let usage = json.get("usage")
                                    .or_else(|| json.get("message").and_then(|m| m.get("usage")));
                                if let Some(u) = usage {
                                    if state.exact_prompt_tokens.is_none() {
                                        let inp = u.get("prompt_tokens")
                                            .or_else(|| u.get("input_tokens"))
                                            .and_then(|v| v.as_i64());
                                        if inp.is_some() {
                                            state.exact_prompt_tokens = inp;
                                        }
                                    }
                                    let out = u.get("completion_tokens")
                                        .or_else(|| u.get("output_tokens"))
                                        .and_then(|v| v.as_i64());
                                    if let Some(v) = out {
                                        state.exact_completion_tokens = Some(v);
                                    }
                                }
                            }
                        }

                        if event.is_end() {
                            // 冲刷转换器收尾事件
                            if let Ok(bytes) = converter.finish() {
                                if !bytes.is_empty() {
                                    // 解析 finish 输出的事件并加入收集列表
                                    let finish_events = SseParser::new().feed(&bytes);
                                    collected_events.extend(finish_events);
                                }
                            }
                            let elapsed = stream_start.elapsed();
                            tracing::info!(
                                total_events,
                                elapsed_ms = elapsed.as_millis(),
                                "非流式 SSE 收集完成（[DONE]）"
                            );
                            let aggregated = stream_response::aggregate_sse_to_json(&collected_events);
                            let _ = result_tx.send(aggregated);
                            return;
                        }

                        // 转换事件并收集
                        match converter.convert(&event) {
                            Ok(converted) => {
                                if !converted.is_empty() {
                                    let converted_events = SseParser::new().feed(&converted);
                                    collected_events.extend(converted_events);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("非流式协议转换失败: {e}");
                                collected_events.push(event);
                            }
                        }
                    }
                }
                Some(Err(err)) => {
                    tracing::warn!(error = %err, "非流式 SSE 上游错误");
                    // 出错时用已收集的事件聚合
                    let aggregated = stream_response::aggregate_sse_to_json(&collected_events);
                    let _ = result_tx.send(aggregated);
                    return;
                }
                None => {
                    let elapsed = stream_start.elapsed();
                    tracing::info!(
                        total_events,
                        elapsed_ms = elapsed.as_millis(),
                        "非流式 SSE 连接关闭"
                    );
                    let aggregated = stream_response::aggregate_sse_to_json(&collected_events);
                    let _ = result_tx.send(aggregated);
                    return;
                }
            }
        }
    });

    // 等待聚合结果
    let aggregated_body = result_rx.await.unwrap_or_else(|_| {
        Bytes::from(r#"{"error":{"message":"SSE 聚合失败","type":"internal_error"}}"#)
    });

    // 构建非流式 JSON 响应
    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );

    ctx.provider = Some(provider);
    ctx.upstream_status = Some(status);
    ctx.upstream_headers = Some(resp_headers);
    ctx.upstream_body = Some(aggregated_body);
    ctx.stream_shared = Some(shared);
    // 非流式无需 stream_complete_rx，同步返回
    ctx.stream_complete_rx = None;
    ctx.response = None; // 让 finalize::success 从 upstream_body 构建

    Ok(ctx)
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
