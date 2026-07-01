use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext, StreamSharedState};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::stream_response::{
    self, is_sse_response, SseParser, StreamConfig, StreamResponse, StreamState,
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

                if is_sse_response(&headers) {
                    return handle_sse_response(
                        ctx, response, headers, provider, &stream_config,
                    )
                    .await;
                } else {
                    return handle_single_response(ctx, response, provider).await;
                }
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

/// 非流式响应处理
async fn handle_single_response(
    mut ctx: RequestContext,
    response: reqwest::Response,
    provider: crate::models::Provider,
) -> Result<RequestContext, StageError> {
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|err| StageError::new(ctx.clone(), GatewayError::Upstream(err)))?;

    // 上游返回错误时，输出实际发送的请求体（用于调试）
    if status.as_u16() >= 400 {
        let req_body_preview = String::from_utf8_lossy(&ctx.request_body)
            .chars().take(1000).collect::<String>();
        let masked_key = ctx
            .selected_api_key
            .as_deref()
            .map(mask_api_key)
            .unwrap_or_default();
        tracing::warn!(
            upstream_status = %status,
            upstream_url = %ctx.upstream_url.as_deref().unwrap_or("(none)"),
            api_key = %masked_key,
            req_body = %req_body_preview,
            resp_body_preview = %String::from_utf8_lossy(&body).chars().take(500).collect::<String>(),
            "上游返回错误 — 实际发送的请求体如上"
        );
    }

    ctx.provider = Some(provider);
    ctx.upstream_status = Some(status);
    ctx.upstream_headers = Some(headers);
    ctx.upstream_body = Some(body);
    Ok(ctx)
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

    // 启动后台读取任务
    let response_stream = response.bytes_stream();
    let stream_config = config.clone();
    let shared_for_task = shared.clone();
    let _read_task = tokio::spawn(async move {
        let mut parser = SseParser::new();
        let mut pinned_stream = std::pin::pin!(response_stream);
        let mut heartbeat = tokio::time::interval(stream_config.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

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

                                // 流式场景透传原始 SSE 事件，不做协议转换
                                // （chunk 级增量数据无法用 transform_response 处理）
                                output.extend_from_slice(event.serialize().as_bytes());
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
