use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext, StreamSharedState};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::gateway::middleware::stream_response::{
    self, is_sse_response, StreamConfig, StreamResponse, StreamState, SseParser,
};

use super::{build_upstream_url, should_forward_header};

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

    let upstream_url = build_upstream_url(&provider.api_base_url, &ctx.uri)
        .map_err(|error| StageError::new(error_ctx.clone(), error))?;

    let client = Client::builder()
        .timeout(provider.timeout())
        .build()
        .map_err(|err| StageError::new(error_ctx.clone(), GatewayError::Upstream(err)))?;

    let reqwest_method =
        reqwest::Method::from_bytes(ctx.method.as_str().as_bytes()).map_err(|err| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::BadRequest(format!("不支持的方法: {err}")),
            )
        })?;

    let mut upstream_request = client.request(reqwest_method, upstream_url);
    for (name, value) in ctx.headers.iter() {
        if should_forward_header(name) {
            upstream_request = upstream_request.header(name, value);
        }
    }

    let max_retries = provider.max_retries as u32;
    let stream_config = StreamConfig {
        max_retries,
        ..Default::default()
    };

    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let backoff = calculate_backoff(attempt, &stream_config);
            tokio::time::sleep(backoff).await;

            // SSE 断线重连：添加 Last-Event-ID
            let last_event_id = ctx.last_event_id.clone();
            if let Some(ref event_id) = last_event_id {
                upstream_request =
                    upstream_request.header("Last-Event-ID", event_id);
            }
        }

        let request_clone = upstream_request.try_clone().ok_or_else(|| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::Internal("请求不可克隆".to_string()),
            )
        })?;

        match request_clone.body(ctx.body.clone()).send().await {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();

                if is_sse_response(&headers) {
                    return handle_sse_response(
                        ctx,
                        response,
                        headers,
                        provider,
                        &stream_config,
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

    Err(StageError::new(
        error_ctx,
        GatewayError::Upstream(last_error.expect("至少有一次错误")),
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
    let body = response.bytes().await.map_err(|err| {
        StageError::new(ctx.clone(), GatewayError::Upstream(err))
    })?;

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
/// 3. 后台任务读取上游 → 解析 SSE → 更新 shared state → 推送 chunk
/// 4. 主线程从 channel 接收 → 构建 StreamBody → 返回响应
/// 5. 断线重连时携带 Last-Event-ID
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
                            for event in &events {
                                stream_state.record_event();

                                // 更新 last_event_id
                                if let Some(ref id) = event.id {
                                    let mut state = shared_for_task.write().await;
                                    state.last_event_id = Some(id.clone());
                                }

                                if event.is_end() {
                                    stream_state.ended = true;
                                    let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                                    return;
                                }
                            }

                            // 更新已发送字节数
                            {
                                let mut state = shared_for_task.write().await;
                                state.bytes_sent += bytes.len() as u64;
                            }

                            if tx.send(Ok(bytes)).await.is_err() {
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
