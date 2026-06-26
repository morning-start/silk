use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;

use super::{headers_to_json, maybe_body_text};

/// 日志异步写入中间件
pub async fn run(
    log_sender: &tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    ctx: &mut RequestContext,
) -> Result<(), GatewayError> {
    let route = ctx.route.as_ref();
    let provider = ctx.provider.as_ref();
    let status = ctx
        .final_status
        .or(ctx.upstream_status)
        .map(|value| value.as_u16() as i64);

    // 流式响应没有 upstream_body
    let is_streaming = ctx.upstream_body.is_none();
    let response_body = ctx
        .upstream_body
        .as_ref()
        .and_then(|body| maybe_body_text(body));
    let response_headers = ctx.upstream_headers.as_ref().and_then(headers_to_json);
    let request_headers = headers_to_json(&ctx.headers);
    let request_body = maybe_body_text(&ctx.body);

    let log = crate::models::NewRequestLog {
        request_id: ctx.request_id.clone(),
        method: ctx.method.to_string(),
        path: ctx.path.clone(),
        route_id: route.map(|route| route.id.clone()),
        inbound_protocol: ctx.inbound_protocol.clone(),
        outbound_protocol: ctx.outbound_protocol.clone(),
        request_headers,
        request_body,
        response_status: status,
        status_code: status,
        response_headers,
        response_body: response_body.clone(),
        duration_ms: Some(ctx.elapsed_ms()),
        provider_id: provider.map(|provider| provider.id.clone()),
        error_message: ctx.error_message.clone(),
        error_code: ctx.error_code.clone(),
        model_used: route
            .and_then(|route| route.model_name_override.clone())
            .or_else(|| provider.and_then(|provider| provider.model_name.clone())),
        retry_count: Some(0),
        stream_enabled: Some(is_streaming),
        cache_hit: Some(false),
        request_size_bytes: Some(ctx.request_size_bytes()),
        response_size_bytes: ctx.response_size_bytes(),
        tokens_input: None,
        tokens_output: None,
    };

    if let Err(err) = log_sender.send(log).await {
        tracing::warn!(%err, "发送日志到 channel 失败");
    }

    Ok(())
}
