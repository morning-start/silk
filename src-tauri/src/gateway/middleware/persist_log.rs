use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;

use super::{headers_to_json, maybe_body_text};

/// 日志 body 最大存储字节数，超过部分截断并标注
const MAX_BODY_STORAGE: usize = 65536; // 64KB

/// 截断过长的 body 文本，保留尺寸标记
fn truncate_body(text: Option<String>) -> Option<String> {
    text.map(|s| {
        if s.len() > MAX_BODY_STORAGE {
            // 使用 floor_char_boundary 避免在多字节 UTF-8 字符边界处 panic
            let safe_end = s.floor_char_boundary(MAX_BODY_STORAGE);
            format!(
                "{}... [truncated, original_size: {}]",
                &s[..safe_end],
                s.len()
            )
        } else {
            s
        }
    })
}

/// 日志异步写入中间件
///
/// 只记录 /v1/* 路径的模型请求，记录协议、流式状态、模型名、token 用量等
pub async fn run(
    log_sender: &tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    ctx: &mut RequestContext,
) -> Result<(), GatewayError> {
    // 只记录 /v1/* 路径的请求（模型相关）
    if !ctx.path.starts_with("/v1/") {
        return Ok(());
    }

    let route = ctx.route.as_ref();
    let provider = ctx.provider.as_ref();
    let status = ctx
        .final_status
        .or(ctx.upstream_status)
        .map(|value| value.as_u16() as i64);

    // 从请求体 JSON 提取 model 和 stream 字段
    let request_body_full = maybe_body_text(&ctx.request_body);
    let (model_from_body, stream_from_body) = request_body_full
        .as_deref()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .map(|json| {
            let model = json
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let stream = json
                .get("stream")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (model, stream)
        })
        .unwrap_or((None, false));
    let request_body = truncate_body(request_body_full);

    // 流式判定：请求体 stream=true 优先，兜底看 upstream_body 是否为空
    let is_streaming = stream_from_body || ctx.upstream_body.is_none();

    // 模型名：remote_model_override > 请求体 model > 路由或 Provider
    let model_used = ctx
        .remote_model_override
        .clone()
        .or(model_from_body)
        .or_else(|| {
            route
                .and_then(|r| r.model_name_override.clone())
                .or_else(|| provider.and_then(|p| p.models_vec().first().cloned()))
        });

    // 从响应体提取 token 用量（非流式场景）
    let response_body_full = ctx
        .upstream_body
        .as_ref()
        .and_then(|body| maybe_body_text(body));
    let (tokens_input, tokens_output) = response_body_full
        .as_deref()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .and_then(|json| {
            json.get("usage").and_then(|usage| {
                let inp = usage.get("prompt_tokens").and_then(|v| v.as_i64());
                let out = usage.get("completion_tokens").and_then(|v| v.as_i64());
                if inp.is_some() || out.is_some() {
                    Some((inp, out))
                } else {
                    None
                }
            })
        })
        .unwrap_or((None, None));
    let response_body = truncate_body(response_body_full);

    // 计费：延迟到消费侧（log_writer）计算，不阻塞请求路径
    let cost = None;

    let response_headers = ctx.upstream_headers.as_ref().and_then(headers_to_json);
    let request_headers = headers_to_json(&ctx.headers);

    let retry_total = ctx.failed_keys.len() as i64 + ctx.failed_providers.len() as i64;

    let log = crate::models::NewRequestLog {
        request_id: ctx.request_id.clone(),
        method: ctx.method.to_string(),
        path: ctx.path.clone(),
        // 无 routing rule 时有认证 key 则记录 key 名
        route_id: route.map(|route| route.id.clone()),
        inbound_protocol: ctx.inbound_protocol.clone(),
        outbound_protocol: ctx.outbound_protocol.clone(),
        request_headers,
        request_body,
        status_code: status,
        response_headers,
        response_body,
        duration_ms: Some(ctx.elapsed_ms()),
        provider_id: provider.filter(|p| !p.id.is_empty()).map(|provider| provider.id.clone()),
        error_message: ctx.error_message.clone(),
        error_code: ctx.error_code.clone(),
        model_used,
        retry_count: Some(retry_total),
        stream_enabled: Some(is_streaming),
        cache_hit: Some(false),
        request_size_bytes: Some(ctx.request_size_bytes()),
        response_size_bytes: ctx.response_size_bytes(),
        tokens_input,
        tokens_output,
        cost,
        auth_key_name: ctx.auth_key_name.clone(),
    };

    match log_sender.try_send(log) {
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!("日志 channel 已满，丢弃一条日志");
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            // receiver 已 drop，无需额外处理
        }
        Ok(_) => {}
    }

    Ok(())
}
