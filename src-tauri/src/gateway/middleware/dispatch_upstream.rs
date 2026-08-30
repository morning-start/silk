use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::{Stream, StreamExt};
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
    let provider = ctx.provider.as_ref().cloned().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    let upstream_url = build_upstream_url_checked(&ctx, &provider)?;
    let reqwest_method = build_upstream_method(&ctx)?;
    // 渠道配置了代理（proxy_url）时使用带代理的客户端，否则默认直连客户端
    let client = runtime
        .streaming_client_for(provider.proxy_url.as_deref())
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Internal(e)))?;
    let upstream_request = build_upstream_request(&client, &ctx, upstream_url, reqwest_method, &provider)?;

    let max_retries = provider.max_retries as u32;
    let client_requested_stream = ctx.client_requested_stream;
    let stream_config = StreamConfig {
        max_retries,
        ..Default::default()
    };

    log_upstream_request(&ctx, &upstream_request, &provider);

    dispatch_with_retries(
        ctx,
        upstream_request,
        &stream_config,
        provider,
        client_requested_stream,
    )
    .await
}

/// 构建并验证上游 URL
fn build_upstream_url_checked(
    ctx: &RequestContext,
    provider: &crate::models::Provider,
) -> Result<reqwest::Url, StageError> {
    if let Some(ref url) = ctx.upstream_url {
        reqwest::Url::parse(url).map_err(|err| {
            StageError::new(
                ctx.clone(),
                GatewayError::BadRequest(format!("无效的上游地址: {err}")),
            )
        })
    } else {
        build_upstream_url(&provider.api_base_url, &ctx.uri).map_err(|error| {
            StageError::new(ctx.clone(), error)
        })
    }
}

/// 构建上游 HTTP 方法
fn build_upstream_method(ctx: &RequestContext) -> Result<reqwest::Method, StageError> {
    if let Some(ref method) = ctx.upstream_method {
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|err| {
            StageError::new(
                ctx.clone(),
                GatewayError::BadRequest(format!("无效的上游方法: {err}")),
            )
        })
    } else {
        reqwest::Method::from_bytes(ctx.method.as_str().as_bytes()).map_err(|err| {
            StageError::new(
                ctx.clone(),
                GatewayError::BadRequest(format!("不支持的方法: {err}")),
            )
        })
    }
}

/// 构建上游请求（URL + 方法 + Headers）
fn build_upstream_request(
    client: &reqwest::Client,
    ctx: &RequestContext,
    upstream_url: reqwest::Url,
    reqwest_method: reqwest::Method,
    provider: &crate::models::Provider,
) -> Result<reqwest::RequestBuilder, StageError> {
    let mut request = client.request(reqwest_method, upstream_url);

    // 1. 应用适配器生成的上游请求头（API Key、Content-Type 等）
    if let Some(ref adapter_headers) = ctx.upstream_headers {
        for (name, value) in adapter_headers.iter() {
            request = request.header(name, value);
        }
    }

    // 2. 转发客户端头（使用 HeaderConfig 配置）
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
        if header_config.should_forward(name.as_str()) {
            request = request.header(name, value);
        }
    }

    // 3. 应用 Provider 自定义请求头（最高优先级）
    let custom_headers = provider.custom_headers_vec();
    for entry in &custom_headers {
        if entry.enabled && !entry.name.is_empty() {
            request = request.header(&entry.name, &entry.value);
        }
    }

    Ok(request)
}

/// 输出上游请求调试日志
fn log_upstream_request(
    ctx: &RequestContext,
    _request: &reqwest::RequestBuilder,
    provider: &crate::models::Provider,
) {
    let masked_key = ctx
        .selected_api_key
        .as_deref()
        .map(mask_api_key)
        .unwrap_or_default();
    let body_preview = String::from_utf8_lossy(&ctx.request_body)
        .chars()
        .take(200)
        .collect::<String>();

    // 从 RequestBuilder 中提取 URL 和 method 信息
    tracing::debug!(
        api_key = %masked_key,
        body_len = ctx.request_body.len(),
        body_preview = %body_preview,
        provider = %provider.name,
        "转发上游请求"
    );
}

/// 执行带重试的上游请求分发
async fn dispatch_with_retries(
    ctx: RequestContext,
    mut upstream_request: reqwest::RequestBuilder,
    stream_config: &StreamConfig,
    provider: crate::models::Provider,
    client_requested_stream: bool,
) -> Result<RequestContext, StageError> {
    let max_retries = stream_config.max_retries;
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let backoff = calculate_backoff(attempt, stream_config);
            tokio::time::sleep(backoff).await;

            // SSE 断线重连：添加 Last-Event-ID
            if let Some(ref event_id) = ctx.last_event_id {
                upstream_request = upstream_request.header("Last-Event-ID", event_id);
            }
        }

        let request_clone = upstream_request.try_clone().ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Internal("请求不可克隆".to_string()),
            )
        })?;

        let body_preview = String::from_utf8_lossy(&ctx.request_body).chars().take(500).collect::<String>();
        let req_url = ctx.upstream_url.as_ref().map(|u| u.to_string()).unwrap_or_default();
        tracing::info!(
            url = %req_url,
            body_bytes = ctx.request_body.len(),
            body_preview = %body_preview,
            "发送上游请求"
        );

        match request_clone.body(ctx.request_body.clone()).send().await {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                tracing::debug!(status = %status, attempt, "收到上游响应");

                if !status.is_success() {
                    return handle_upstream_error(ctx, response, headers).await;
                }

                return handle_sse_response(
                    ctx,
                    response,
                    headers,
                    provider,
                    stream_config,
                    client_requested_stream,
                )
                .await;
            }
            Err(err) => {
                tracing::warn!(
                    attempt,
                    error = %err,
                    error_debug = ?err,
                    url = %req_url,
                    "上游请求发送失败（send 返回错误）"
                );
                last_error = Some(err);
            }
        }
    }

    // 所有重试耗尽，返回最后一条错误
    Err(StageError::new(
        ctx,
        match last_error {
            Some(err) => GatewayError::UpstreamError {
                status: 0,
                body: serde_json::json!({
                    "error": { "message": err.to_string(), "type": "upstream_error" }
                }),
            },
            None => GatewayError::Internal("上游请求失败（无详细错误）".to_string()),
        },
    ))
}


async fn handle_upstream_error(
    mut ctx: RequestContext,
    response: reqwest::Response,
    headers: axum::http::HeaderMap,
) -> Result<RequestContext, StageError> {
    let status = response.status();
    tracing::warn!(status = %status, "上游返回错误状态码");
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
    let body_str = String::from_utf8_lossy(&body).chars().take(500).collect::<String>();
    tracing::error!(status = %status, body = %body_str, "上游错误响应内容");
    let parsed_body = serde_json::from_slice(&body).unwrap_or_else(|_| {
        serde_json::json!({
            "error": {
                "message": body_str,
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
        return handle_nonstreaming_aggregate(ctx, response, provider).await;
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, GatewayError>>(256);
    let shared = Arc::new(RwLock::new(StreamSharedState::default()));
    let status = response.status();

    // 流结束通知通道：后台任务 → pipeline
    let (complete_tx, complete_rx) = tokio::sync::oneshot::channel::<()>();

    // 启动后台读取任务
    let inbound = ctx.inbound_protocol.clone().unwrap_or_default();
    let outbound = ctx.outbound_protocol.clone().unwrap_or_default();

    tokio::spawn(run_sse_read_task(
        response.bytes_stream(),
        tx,
        complete_tx,
        shared.clone(),
        config.clone(),
        inbound,
        outbound.clone(),
    ));

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

/// 一个 chunk 解析出的 SSE 事件的处理结果
enum ChunkOutcome {
    /// 已转换为待下发字节（可能为空）
    Data(Vec<u8>),
    /// 上游标记流结束（如 `[DONE]`），携带转换器冲刷出的收尾事件
    /// （如 anthropic `message_stop`）的字节
    End(Vec<u8>),
}

/// SSE 后台读取任务：读取上游字节流 → 解析 SSE 事件 → 协议转换 → 推送到 `tx`
///
/// 任务退出前必定通过 `complete_tx` 通知 pipeline（正常结束、上游错误、超时或客户端断开）。
/// 心跳分支在 `stream_timeout` 内未收到任何数据时判定超时。
async fn run_sse_read_task(
    response_stream: impl Stream<Item = reqwest::Result<Bytes>>,
    tx: tokio::sync::mpsc::Sender<Result<Bytes, GatewayError>>,
    complete_tx: tokio::sync::oneshot::Sender<()>,
    shared: Arc<RwLock<StreamSharedState>>,
    config: StreamConfig,
    inbound: String,
    outbound: String,
) {
    // Anthropic Messages 客户端不识别 [DONE]，用 message_stop 结束流。
    // Responses 客户端使用 response.completed 结束流。
    // 只有 OpenAI Chat 客户端识别 [DONE] 作为标准 SSE 终止符。
    // 用 inbound（客户端协议）判断，而非 outbound（上游协议）。
    let send_done_marker = inbound == "openai";
    let mut parser = SseParser::new();
    let mut pinned_stream = std::pin::pin!(response_stream);
    let mut heartbeat = tokio::time::interval(config.heartbeat_interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut converter = SseConverter::new(&inbound, &outbound);
    let mut stream_state = StreamState::new();
    let mut total_events: u64 = 0;
    let stream_start = std::time::Instant::now();

    loop {
        tokio::select! {
            chunk = pinned_stream.next() => {
                let Some(chunk) = chunk else {
                    tracing::info!(
                        total_events,
                        elapsed_ms = stream_start.elapsed().as_millis(),
                        "SSE 流连接关闭（无结束标记）"
                    );
                    if !stream_state.ended && send_done_marker {
                        let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                    }
                    // tx 在此被 drop，ReceiverStream 结束，HTTP 响应完成
                    let _ = complete_tx.send(());
                    return;
                };

                match chunk {
                    Ok(bytes) => {
                        stream_state.record_data(bytes.len());
                        shared.write().await.bytes_received += bytes.len() as u64;

                        let events = parser.feed(&bytes);
                        if events.is_empty() {
                            // 不完整 chunk：可能是部分 SSE 事件、纯注释行、或空行。
                            // 不能直接转发原始字节——否则上游心跳注释（": xxx"）或
                            // 不完整 JSON 会与 silk 自己的 keep-alive 合并，产生
                            // "\:" 等非法 JSON 转义。丢弃即可，完整事件会在后续
                            // chunk 中由 parser 重组后走正常转换路径。
                            continue;
                        }

                        let outcome = process_sse_events(
                            &events,
                            &mut converter,
                            &shared,
                            &mut stream_state,
                            &mut total_events,
                        ).await;

                        match outcome {
                            ChunkOutcome::Data(output) => {
                                shared.write().await.bytes_sent += output.len() as u64;
                                if tx.send(Ok(Bytes::from(output))).await.is_err() {
                                    // 客户端已断开，确保 complete_tx 被发送
                                    let _ = complete_tx.send(());
                                    return;
                                }
                            }
                            ChunkOutcome::End(flush) => {
                                tracing::info!(
                                    end_output_bytes = flush.len(),
                                    end_output_preview = %String::from_utf8_lossy(&flush).chars().take(300).collect::<String>(),
                                    "发送流结束输出到客户端"
                                );
                                if !flush.is_empty() {
                                    let _ = tx.send(Ok(Bytes::from(flush))).await;
                                }
                                let state = shared.read().await;
                                tracing::info!(
                                    total_events,
                                    bytes_received = state.bytes_received,
                                    bytes_sent = state.bytes_sent,
                                    elapsed_ms = stream_start.elapsed().as_millis(),
                                    prompt_tokens = state.exact_prompt_tokens.unwrap_or(0),
                                    completion_tokens = state.exact_completion_tokens.unwrap_or(0),
                                    send_done_marker,
                                    "SSE 流正常结束"
                                );
                                // 仅对识别 [DONE] 的协议发送流结束标记
                                if send_done_marker {
                                    let _ = tx.send(Ok(stream_response::stream_end_marker())).await;
                                }
                                // tx 在此被 drop，ReceiverStream 结束，HTTP 响应完成
                                let _ = complete_tx.send(());
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            total_events,
                            elapsed_ms = stream_start.elapsed().as_millis(),
                            "SSE 流上游错误"
                        );
                        let _ = tx.send(Err(GatewayError::Upstream(err))).await;
                        // tx 在此被 drop，ReceiverStream 结束，HTTP 响应完成
                        let _ = complete_tx.send(());
                        return;
                    }
                }
            }
            _ = heartbeat.tick() => {
                if stream_state.is_timed_out(config.stream_timeout) {
                    tracing::warn!(
                        total_events,
                        elapsed_ms = stream_start.elapsed().as_millis(),
                        timeout_secs = config.stream_timeout.as_secs(),
                        "SSE 流超时"
                    );
                    let _ = tx.send(Err(GatewayError::Timeout)).await;
                    // tx 在此被 drop，ReceiverStream 结束，HTTP 响应完成
                    let _ = complete_tx.send(());
                    return;
                }
                if tx.send(Ok(stream_response::heartbeat_comment())).await.is_err() {
                    // 客户端已断开，确保 complete_tx 被发送
                    tracing::debug!("客户端断开连接（心跳检测）");
                    let _ = complete_tx.send(());
                    return;
                }
            }
        }
    }
}

/// 将一批 SSE 事件转换为待下发字节，并同步用量 / last_event_id 到共享状态
async fn process_sse_events(
    events: &[SseEvent],
    converter: &mut SseConverter,
    shared: &Arc<RwLock<StreamSharedState>>,
    stream_state: &mut StreamState,
    total_events: &mut u64,
) -> ChunkOutcome {
    let mut output = Vec::new();

    for event in events {
        stream_state.record_event();
        *total_events += 1;

        tracing::info!(
            event_type = ?event.event,
            data_len = event.data.as_deref().map(|d| d.len()).unwrap_or(0),
            data_preview = %event.data.as_deref().unwrap_or("").chars().take(200).collect::<String>(),
            is_end = event.is_end(),
            "收到 SSE 事件"
        );

        if let Some(usage) = event.parse_usage() {
            shared.write().await.record_usage(&usage);
        }
        if let Some(ref id) = event.id {
            shared.write().await.last_event_id = Some(id.clone());
        }

        if event.is_end() {
            // 先转换结束事件本身（message_stop → finish_reason:stop）
            let mut end_output = Vec::new();
            match converter.convert(event) {
                Ok(bytes) => {
                    tracing::info!(
                        end_event_bytes = bytes.len(),
                        end_event_preview = %String::from_utf8_lossy(&bytes).chars().take(200).collect::<String>(),
                        "结束事件转换输出"
                    );
                    end_output.extend_from_slice(&bytes);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "结束事件转换失败");
                }
            }
            // 冲刷转换器收尾事件（如 anthropic message_stop）
            let flush = converter.finish().unwrap_or_default();
            tracing::info!(
                flush_bytes = flush.len(),
                flush_preview = %String::from_utf8_lossy(&flush).chars().take(200).collect::<String>(),
                "转换器冲刷输出"
            );
            end_output.extend_from_slice(&flush);
            return ChunkOutcome::End(end_output);
        }

        // 协议转换（流式场景按事件逐条转换）
        match converter.convert(event) {
            Ok(bytes) => {
                tracing::debug!(
                    converted_bytes = bytes.len(),
                    converted_preview = %String::from_utf8_lossy(&bytes).chars().take(120).collect::<String>(),
                    "转换输出"
                );
                output.extend_from_slice(&bytes);
            }
            Err(e) => {
                tracing::warn!("流式协议转换失败: {e}");
                // 转换失败时透传原始事件，避免打断客户端流
                output.extend_from_slice(event.serialize().as_bytes());
            }
        }
    }

    ChunkOutcome::Data(output)
}

/// 非流式响应聚合：收集所有 SSE 事件，聚合为单个 JSON 响应。
///
/// 当客户端未请求流式时使用。网关强制上游走流式（transform_request 注入 stream:true），
/// 但客户端期望单个 JSON 响应。此函数在后台收集所有 SSE chunk，通过 prism 转换后
/// 聚合为完整的 chat.completion JSON。
async fn handle_nonstreaming_aggregate(
    mut ctx: RequestContext,
    response: reqwest::Response,
    provider: crate::models::Provider,
) -> Result<RequestContext, StageError> {
    let shared = Arc::new(RwLock::new(StreamSharedState::default()));
    let status = response.status();

    // 判断是否需要流式协议转换
    let inbound = ctx.inbound_protocol.clone().unwrap_or_default();
    let outbound = ctx.outbound_protocol.clone().unwrap_or_default();

    // 后台收集任务：读取所有 SSE 事件并聚合为 JSON
    let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Bytes>();
    tokio::spawn(collect_and_aggregate(
        response.bytes_stream(),
        shared.clone(),
        inbound,
        outbound,
        result_tx,
    ));

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

/// 后台收集任务：读完整个 SSE 流并聚合为单个 JSON 响应体
///
/// 无论正常结束、上游报错还是连接关闭，都会用已收集到的事件聚合后通过 `result_tx` 返回，
/// 确保调用方一定能拿到响应体。
async fn collect_and_aggregate(
    response_stream: impl Stream<Item = reqwest::Result<Bytes>>,
    shared: Arc<RwLock<StreamSharedState>>,
    inbound: String,
    outbound: String,
    result_tx: tokio::sync::oneshot::Sender<Bytes>,
) {
    let mut parser = SseParser::new();
    let mut converter = SseConverter::new(&inbound, &outbound);
    let mut collected_events: Vec<SseEvent> = Vec::new();
    let mut pinned_stream = std::pin::pin!(response_stream);
    let mut total_events: u64 = 0;
    let stream_start = std::time::Instant::now();

    loop {
        match pinned_stream.next().await {
            Some(Ok(bytes)) => {
                shared.write().await.bytes_received += bytes.len() as u64;

                for event in parser.feed(&bytes) {
                    total_events += 1;

                    let ended = collect_sse_event(
                        event,
                        &mut converter,
                        &shared,
                        &mut collected_events,
                    )
                    .await;

                    if ended {
                        tracing::info!(
                            total_events,
                            elapsed_ms = stream_start.elapsed().as_millis(),
                            "非流式 SSE 收集完成（[DONE]）"
                        );
                        let _ = result_tx.send(finish_aggregation(&collected_events));
                        return;
                    }
                }
            }
            Some(Err(err)) => {
                tracing::warn!(error = %err, "非流式 SSE 上游错误");
                // 出错时用已收集的事件聚合，尽量保留已生成的内容
                let _ = result_tx.send(finish_aggregation(&collected_events));
                return;
            }
            None => {
                tracing::info!(
                    total_events,
                    elapsed_ms = stream_start.elapsed().as_millis(),
                    "非流式 SSE 连接关闭"
                );
                let _ = result_tx.send(finish_aggregation(&collected_events));
                return;
            }
        }
    }
}

/// 聚合已收集的 SSE 事件为完整 JSON 响应体
fn finish_aggregation(collected_events: &[SseEvent]) -> Bytes {
    stream_response::aggregate_sse_to_json(collected_events)
}

/// 处理单个 SSE 事件：记录 token 用量，并将转换后的事件收集进列表
///
/// 返回 true 表示已收到流结束标记（如 `[DONE]`），调用方应立即聚合返回。
async fn collect_sse_event(
    event: SseEvent,
    converter: &mut SseConverter,
    shared: &Arc<RwLock<StreamSharedState>>,
    collected: &mut Vec<SseEvent>,
) -> bool {
    if let Some(usage) = event.parse_usage() {
        shared.write().await.record_usage(&usage);
    }

    if event.is_end() {
        // 冲刷转换器收尾事件（如 anthropic message_stop）
        if let Ok(bytes) = converter.finish() {
            if !bytes.is_empty() {
                collected.extend(SseParser::new().feed(&bytes));
            }
        }
        return true;
    }

    // 转换事件并收集
    match converter.convert(&event) {
        Ok(converted) if !converted.is_empty() => {
            collected.extend(SseParser::new().feed(&converted));
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("非流式协议转换失败: {e}");
            collected.push(event);
        }
    }

    false
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
