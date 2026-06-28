use linguafranca::anthropic::request::AnthropicRequest;
use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::open_responses::request::OpenResponsesRequest;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;

/// 请求转换中间件
///
/// 选择出站协议对应的适配器，处理跨协议请求体格式转换，
/// 将原始请求体（JSON）转为上游请求格式，更新 ctx.request_body。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    let inbound = ctx
        .inbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_chat".to_string());
    let outbound = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| inbound.clone());

    let target_protocol = &outbound;

    // 跨协议时转换请求体格式：inbound → hub → outbound
    let request_bytes = if inbound != outbound {
        tracing::debug!(
            "跨协议格式转换: inbound={}, outbound={}",
            inbound,
            outbound
        );
        convert_request_body(&ctx.request_body, &inbound, &outbound)
            .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e)))?
    } else {
        ctx.request_body.to_vec()
    };

    // 获取 outbound 适配器（生成正确的 URL、认证头、Content-Type）
    let adapter = ctx
        .adapter_registry
        .get(target_protocol)
        .or_else(|| ctx.adapter_registry.get("openai_chat"))
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {target_protocol}")),
            )
        })?;

    let provider = ctx.provider.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    let selected_api_key = ctx.selected_api_key.as_deref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少已选中的上游 Key".to_string()),
        )
    })?;

    // 调用 outbound 适配器转换（验证并生成请求头/URL/序列化 body）
    let upstream_req = adapter
        .transform_request(&request_bytes, provider, selected_api_key)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    let new_body = serde_json::to_vec(&upstream_req.body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    ctx.request_body = bytes::Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);
    ctx.upstream_url = Some(upstream_req.url);
    ctx.upstream_method = Some(upstream_req.method);

    Ok(ctx)
}

/// 将请求体从入站格式转换为出站格式（inbound → hub → outbound）
fn convert_request_body(
    body: &[u8],
    from: &str,
    to: &str,
) -> Result<Vec<u8>, String> {
    let hub: OpenResponsesRequest = match from {
        "openai_chat" => {
            let req: ChatCompletionsOpenAiRequest = serde_json::from_slice(body)
                .map_err(|e| format!("解析 Chat 请求失败: {e}"))?;
            req.into_open_responses(None)
                .map_err(|e| format!("Chat → OpenResponses 转换失败: {e}"))?
                .value
        }
        "claude_messages" => {
            let req: AnthropicRequest = serde_json::from_slice(body)
                .map_err(|e| format!("解析 Claude 请求失败: {e}"))?;
            req.into_open_responses(None)
                .map_err(|e| format!("Claude → OpenResponses 转换失败: {e}"))?
                .value
        }
        "openai_response" => serde_json::from_slice(body)
            .map_err(|e| format!("解析 OpenResponses 请求失败: {e}"))?,
        other => return Err(format!("不支持的入站协议: {other}")),
    };

    let outbound: serde_json::Value = match to {
        "openai_chat" => {
            let req = ChatCompletionsOpenAiRequest::from_open_responses(hub, None)
                .map_err(|e| format!("OpenResponses → Chat 转换失败: {e}"))?
                .value;
            serde_json::to_value(req)
                .map_err(|e| format!("序列化 Chat 请求失败: {e}"))?
        }
        "claude_messages" => {
            let req = AnthropicRequest::from_open_responses(hub, None)
                .map_err(|e| format!("OpenResponses → Claude 转换失败: {e}"))?
                .value;
            serde_json::to_value(req)
                .map_err(|e| format!("序列化 Claude 请求失败: {e}"))?
        }
        "openai_response" => serde_json::to_value(hub)
            .map_err(|e| format!("序列化 OpenResponses 请求失败: {e}"))?,
        other => return Err(format!("不支持的出站协议: {other}")),
    };

    serde_json::to_vec(&outbound).map_err(|e| format!("序列化失败: {e}"))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_placeholder() {
        // 集成测试在端到端测试中覆盖
        assert!(true);
    }
}
