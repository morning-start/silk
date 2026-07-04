use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;

use super::maybe_body_text;

// ---------------------------------------------------------------------------
// Token 估算（无需 tokenizer 的轻量化估计）
// ---------------------------------------------------------------------------

/// 从 JSON 请求体估算 token 数
///
/// 策略：
/// - chat 格式：提取 messages 中所有 content 的字符数，除以 4（~英文平均 chars/token）
/// - 其他格式：body 字节数 / 4
pub fn estimate_tokens_from_body(body: &[u8]) -> Option<i64> {
    let json: serde_json::Value = serde_json::from_slice(body).ok()?;

    // Chat 格式：统计 messages[].content 的字符数
    if let Some(messages) = json.get("messages").and_then(|v| v.as_array()) {
        let total_chars: usize = messages
            .iter()
            .filter_map(|m| m.get("content"))
            .filter_map(|c| c.as_str())
            .map(|s| s.chars().count())
            .sum();
        if total_chars > 0 {
            return Some((total_chars as f64 / 4.0).ceil() as i64);
        }
    }

    // Responses 格式：input 的字符数
    if let Some(input) = json.get("input").and_then(|v| v.as_str()) {
        let chars = input.chars().count();
        return Some((chars as f64 / 4.0).ceil() as i64);
    }

    // 兜底：body 字节数 / 4
    let bytes = body.len();
    if bytes > 0 {
        Some((bytes as f64 / 4.0).ceil() as i64)
    } else {
        None
    }
}

/// 从字节数估算 token（统一算法：bytes / 4）
pub fn estimate_tokens_from_bytes(bytes: i64) -> i64 {
    (bytes as f64 / 4.0).ceil() as i64
}

// ---------------------------------------------------------------------------
// 日志构建 + 发送
// ---------------------------------------------------------------------------

/// 从请求上下文构建日志条目（不发送）
#[must_use]
pub fn build_log(ctx: &RequestContext) -> crate::models::NewRequestLog {
    let route = ctx.route.as_ref();
    let provider = ctx.provider.as_ref();
    let status = ctx
        .final_status
        .or(ctx.upstream_status)
        .map(|value| value.as_u16() as i64);

    // 从请求体 JSON 提取 model 和 stream 字段
    let (model_from_body, _stream_from_body) = ctx
        .parsed_body
        .as_ref()
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
        .unwrap_or_else(|| {
            let request_body_full = maybe_body_text(&ctx.request_body);
            request_body_full
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
                .unwrap_or((None, false))
        });

    // 模型 ID：remote_model_override > 请求体 model > 路由或 Provider
    let model_id = ctx
        .remote_model_override
        .clone()
        .or(model_from_body)
        .or_else(|| {
            route
                .and_then(|r| r.model_name_override.clone())
                .or_else(|| provider.and_then(|p| p.models_vec().first().cloned()))
        });

    // 模型池名称：来自路由的 model_name_override
    let model_name = route.and_then(|r| r.model_name_override.clone());

    let retry_total = ctx.failed_keys.len() as i64 + ctx.failed_providers.len() as i64;

    crate::models::NewRequestLog {
        request_id: ctx.request_id.clone(),
        method: ctx.method.to_string(),
        path: ctx.path.clone(),
        route_id: route.map(|route| route.id.clone()),
        inbound_protocol: ctx.inbound_protocol.clone(),
        outbound_protocol: ctx.outbound_protocol.clone(),
        status_code: status,
        resp_ms: Some(ctx.elapsed_ms()),
        provider_id: provider
            .filter(|p| !p.id.is_empty())
            .map(|provider| provider.id.clone()),
        error_message: ctx.error_message.clone(),
        error_code: ctx.error_code.clone(),
        model_id,
        model_name,
        retry_count: Some(retry_total),
        stream_enabled: Some(true),
        cache_hit: Some(false),
        request_size_bytes: Some(ctx.request_size_bytes()),
        response_size_bytes: None, // 流结束后由 pipeline 填充
        tokens_input: estimate_tokens_from_body(&ctx.client_body),
        tokens_output: None, // 流结束后由 pipeline 填充
        tokens_sent: estimate_tokens_from_body(&ctx.request_body),
        total_duration_ms: None, // 流结束后由 pipeline 填充
        cost: None,
        auth_key_name: ctx.auth_key_name.clone(),
        channel_key_name: ctx.channel_key_name.clone(),
    }
}

/// 发送日志到异步写入通道
pub fn send_log(
    log_sender: &tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    log: crate::models::NewRequestLog,
) {
    match log_sender.try_send(log) {
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!("日志 channel 已满，丢弃一条日志");
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {}
        Ok(_) => {}
    }
}

/// 日志中间件（同步版本，用于非流式/错误路径兜底）
pub async fn run(
    log_sender: &tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    ctx: &mut RequestContext,
) -> Result<(), GatewayError> {
    if !ctx.path.starts_with("/v1/") {
        return Ok(());
    }
    let log = build_log(ctx);
    send_log(log_sender, log);
    Ok(())
}
