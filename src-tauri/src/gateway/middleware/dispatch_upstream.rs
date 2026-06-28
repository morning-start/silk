use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext, StreamSharedState};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::stream_response::{
    self, is_sse_response, SseParser, StreamConfig, StreamResponse, StreamState,
};
use crate::gateway::pipeline::StageError;
use crate::protocol::{AdapterRegistry, UpstreamResponse};

use super::{build_upstream_url, should_forward_header};

/// 请求转发入口：自动判断流式/非流式
pub async fn run(
    _runtime: &GatewayContext,
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

    let client = Client::builder()
        .timeout(provider.timeout())
        .build()
        .map_err(|err| StageError::new(error_ctx.clone(), GatewayError::Upstream(err)))?;

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
                upstream_request = upstream_request.header("Last-Event-ID", event_id);
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
                let _status = response.status();
                let headers = response.headers().clone();

                if is_sse_response(&headers) {
                    let adapter_registry = ctx.adapter_registry.clone();
                    let outbound_protocol = ctx.outbound_protocol.clone();
                    return handle_sse_response(
                        ctx, response, headers, provider, &stream_config,
                        adapter_registry, outbound_protocol,
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
    let body = response
        .bytes()
        .await
        .map_err(|err| StageError::new(ctx.clone(), GatewayError::Upstream(err)))?;

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
/// 3. 后台任务读取上游 → 解析 SSE → 反向协议转换 → 更新 shared state → 推送 chunk
/// 4. 主线程从 channel 接收 → 构建 StreamBody → 返回响应
/// 5. 断线重连时携带 Last-Event-ID
async fn handle_sse_response(
    mut ctx: RequestContext,
    response: reqwest::Response,
    headers: axum::http::HeaderMap,
    provider: crate::models::Provider,
    config: &StreamConfig,
    adapter_registry: Arc<AdapterRegistry>,
    outbound_protocol: Option<String>,
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

        // 获取反向转换适配器（非 OpenAI 原生协议时需要）
        let protocol = outbound_protocol.as_deref().unwrap_or("openai_response");
        let converter = adapter_registry.get(protocol);

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
                            let mut all_converted = true;

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

                                // 尝试对每个事件做反向协议转换
                                if let Some(ref data) = event.data {
                                    if let Some(ref adapter) = converter {
                                        if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(data) {
                                            let upstream_resp = UpstreamResponse {
                                                status: 200,
                                                headers: axum::http::HeaderMap::new(),
                                                body: json_data,
                                            };
                                            match adapter.transform_response(&upstream_resp).await {
                                                Ok(converted) => {
                                                    let mut new_event = event.clone();
                                                    new_event.data = Some(serde_json::to_string(&converted).unwrap_or_default());
                                                    output.extend_from_slice(new_event.serialize().as_bytes());
                                                    continue;
                                                }
                                                Err(_) => {
                                                    all_converted = false;
                                                }
                                            }
                                        } else {
                                            all_converted = false;
                                        }
                                    }
                                }

                                // 没有转换器或转换失败：使用原始事件序列化
                                output.extend_from_slice(event.serialize().as_bytes());
                            }

                            // 更新已发送字节数
                            {
                                let mut state = shared_for_task.write().await;
                                state.bytes_sent += output.len() as u64;
                            }

                            let payload = if all_converted && !output.is_empty() {
                                Bytes::from(output)
                            } else {
                                // 有任何事件转换失败则回退到原始 bytes
                                bytes
                            };

                            if tx.send(Ok(payload)).await.is_err() {
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
