use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;

/// 请求转换中间件
///
/// 根据路由规则的 inbound_protocol 选择适配器，
/// 将原始请求体（JSON）转为上游请求格式，更新 ctx.request_body。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 获取入站协议
    let inbound_protocol = ctx
        .inbound_protocol
        .clone()
        .unwrap_or_else(|| "any".to_string());

    // 获取适配器
    let adapter = ctx
        .adapter_registry
        .get(&inbound_protocol)
        .or_else(|| ctx.adapter_registry.get("openai_chat"))
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {inbound_protocol}")),
            )
        })?;

    // 获取 Provider
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

    // 调用适配器转换（入站适配器负责验证并生成请求头/URL）
    let upstream_req = adapter
        .transform_request(&ctx.request_body, provider, selected_api_key)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    // 序列化上游请求体
    let new_body = serde_json::to_vec(&upstream_req.body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    // 更新上下文
    ctx.request_body = bytes::Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);
    ctx.upstream_url = Some(upstream_req.url);
    ctx.upstream_method = Some(upstream_req.method);

    // 跨协议场景：若 outbound != inbound，用 outbound 协议修正上游 URL
    // （适配器按入站协议生成 URL，但出站协议可能需要不同的路径）
    let outbound_protocol = ctx.outbound_protocol.as_deref();
    if let Some(outbound) = outbound_protocol {
        if outbound != inbound_protocol.as_str() {
            let current_url = ctx.upstream_url.as_deref().unwrap_or("");
            let corrected = upstream_url_for_protocol(provider.api_base_url.as_str(), outbound);
            tracing::debug!(
                "跨协议请求: inbound={}, outbound={}, URL: {} → {}",
                inbound_protocol, outbound, current_url, corrected
            );
            ctx.upstream_url = Some(corrected);
        }
    }

    Ok(ctx)
}

/// 根据出站协议返回上游 API 路径
fn upstream_url_for_protocol(api_base_url: &str, protocol: &str) -> String {
    let base = api_base_url.trim_end_matches('/');
    match protocol {
        "openai_chat" => format!("{base}/v1/chat/completions"),
        "claude_messages" => format!("{base}/v1/messages"),
        "openai_response" => format!("{base}/v1/responses"),
        _ => format!("{base}/v1/chat/completions"),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_placeholder() {
        // 集成测试在端到端测试中覆盖
        assert!(true);
    }
}
