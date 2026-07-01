use axum::http::StatusCode;

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::UpstreamResponse;
use super::mask_api_key;

/// 响应转换中间件
///
/// 职责：
///   - 上游返回错误（4xx/5xx）时，透传原始错误体，不做协议转换
///   - 上游成功时，调用适配器的 transform_response 做协议转换
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 流式 SSE 已由 handle_sse_response 构建响应，跳过
    if ctx.upstream_body.is_none() && ctx.response.is_some() {
        return Ok(ctx);
    }

    let outbound_protocol = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_response".to_string());

    // 读取上游响应体
    let upstream_body = ctx.upstream_body.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少上游响应体".to_string()),
        )
    })?;
    let upstream_status = ctx
        .upstream_status
        .map(|s| s.as_u16())
        .unwrap_or(200);

    // --- 上游错误处理：透传原始状态码和错误体，不做协议转换 ---
    if upstream_status >= 400 {
        let reason_phrase = StatusCode::from_u16(upstream_status)
            .map(|s| s.canonical_reason().unwrap_or("Unknown"))
            .unwrap_or("Unknown");
        let body: serde_json::Value = serde_json::from_slice(upstream_body)
            .unwrap_or_else(|parse_err| {
                // 非 JSON 响应（如 Cloudflare HTML），提取关键信息
                let text = String::from_utf8_lossy(upstream_body);
                let preview = text.chars().take(2000).collect::<String>();
                tracing::warn!(
                    status = upstream_status,
                    reason = reason_phrase,
                    upstream_url = %ctx.upstream_url.as_deref().unwrap_or("(none)"),
                    selected_key_preview = %ctx
                        .selected_api_key
                        .as_deref()
                        .map(mask_api_key)
                        .unwrap_or_default(),
                    body_preview = %preview,
                    parse_error = %parse_err,
                    "上游返回非 JSON 错误响应 — 请检查上游 URL 和 API Key"
                );
                let summary = if text.contains("<html") || text.contains("<HTML") {
                    text.lines()
                        .find(|l| l.to_lowercase().contains("<title>"))
                        .and_then(|l| {
                            let lower = l.to_lowercase();
                            let start = lower.find("<title>").map(|i| i + 7).unwrap_or(0);
                            let end = lower.find("</title>").unwrap_or(l.len());
                            Some(l[start..end].trim().to_string())
                        })
                        .unwrap_or_else(|| format!("上游返回 HTML 错误页面 ({} bytes)", text.len()))
                } else {
                    text.chars().take(500).collect()
                };
                serde_json::json!({"error": {"message": summary, "type": "upstream_error"}})
            });
        return Err(StageError::new(
            ctx,
            GatewayError::UpstreamError {
                status: upstream_status,
                body,
            },
        ));
    }

    // --- 正常响应处理 ---
    let adapter = ctx
        .adapter_registry
        .as_ref()
        .and_then(|reg| reg.get(&outbound_protocol))
        .or_else(|| {
            ctx.adapter_registry
                .as_ref()
                .and_then(|reg| reg.get("openai_response"))
        })
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {outbound_protocol}")),
            )
        })?;

    let upstream_body_bytes = ctx.upstream_body.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("非流式响应缺少 upstream_body".to_string()),
        )
    })?;
    let upstream_resp = UpstreamResponse {
        status: ctx.upstream_status.map(|s| s.as_u16()).unwrap_or(200),
        headers: ctx.upstream_headers.clone().unwrap_or_default(),
        body: serde_json::from_slice(upstream_body_bytes).map_err(|e| {
            StageError::new(
                ctx.clone(),
                GatewayError::Serialization(format!("解析上游响应失败: {e}")),
            )
        })?,
    };

    let hub_body = adapter
        .transform_response(&upstream_resp)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    // 将 hub 格式转为客户端期望的格式（inbound_protocol）
    let inbound_protocol = ctx
        .inbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_chat".to_string());

    let final_body = match inbound_protocol.as_str() {
        "openai_response" => hub_body,
        _ => convert_hub_to_client(&hub_body, &inbound_protocol)
            .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e)))?,
    };

    let response_bytes = serde_json::to_vec(&final_body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    ctx.upstream_body = Some(bytes::Bytes::from(response_bytes));

    Ok(ctx)
}

fn convert_hub_to_client(
    hub_body: &serde_json::Value,
    target_protocol: &str,
) -> Result<serde_json::Value, String> {
    use linguafranca::open_responses::response::OpenResponsesResponse;
    use linguafranca::traits::FromOpenResponses;

    let openai_resp: OpenResponsesResponse = serde_json::from_value(hub_body.clone())
        .map_err(|e| format!("解析 OpenResponses 失败: {e}"))?;

    match target_protocol {
        "openai_chat" => {
            use linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse;
            let chat_resp = ChatCompletionsOpenAiResponse::from_open_responses(openai_resp, None)
                .map_err(|e| format!("转换到 Chat 格式失败: {e}"))?
                .value;
            serde_json::to_value(chat_resp).map_err(|e| format!("序列化 Chat 响应失败: {e}"))
        }
        "claude_messages" => {
            use linguafranca::anthropic::response::AnthropicResponse;
            let anthropic_resp = AnthropicResponse::from_open_responses(openai_resp, None)
                .map_err(|e| format!("转换到 Claude 格式失败: {e}"))?
                .value;
            serde_json::to_value(anthropic_resp).map_err(|e| format!("序列化 Claude 响应失败: {e}"))
        }
        _ => Ok(hub_body.clone()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert!(true);
    }
}
