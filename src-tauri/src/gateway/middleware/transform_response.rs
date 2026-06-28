use linguafranca::open_responses::response::OpenResponsesResponse;
use linguafranca::traits::FromOpenResponses;

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::UpstreamResponse;

pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    if ctx.upstream_body.is_none() && ctx.response.is_some() {
        return Ok(ctx);
    }

    let outbound_protocol = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_response".to_string());

    let adapter = ctx
        .adapter_registry
        .get(&outbound_protocol)
        .or_else(|| ctx.adapter_registry.get("openai_response"))
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {outbound_protocol}")),
            )
        })?;

    let upstream_resp = UpstreamResponse {
        status: ctx.upstream_status.map(|s| s.as_u16()).unwrap_or(200),
        headers: ctx.upstream_headers.clone().unwrap_or_default(),
        body: serde_json::from_slice(&ctx.body).map_err(|e| {
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

    let final_body = match outbound_protocol.as_str() {
        "openai_response" => hub_body,
        _ => convert_hub_to_client(&hub_body, &outbound_protocol)
            .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e)))?,
    };

    let response_bytes = serde_json::to_vec(&final_body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    ctx.upstream_body = Some(bytes::Bytes::from(response_bytes));
    ctx.upstream_status = Some(axum::http::StatusCode::OK);

    Ok(ctx)
}

fn convert_hub_to_client(
    hub_body: &serde_json::Value,
    target_protocol: &str,
) -> Result<serde_json::Value, String> {
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
