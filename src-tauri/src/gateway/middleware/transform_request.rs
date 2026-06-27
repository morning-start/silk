use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::CanonicalRequest;

/// 请求转换中间件
///
/// 根据路由规则的 inbound_protocol 选择适配器，
/// 将原始请求体（JSON）解析为 CanonicalRequest，
/// 然后转为上游请求格式，更新 ctx.body。
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

    // 尝试解析请求体为 CanonicalRequest
    let canonical: CanonicalRequest = match serde_json::from_slice(&ctx.body) {
        Ok(c) => c,
        Err(_) => {
            // 不是标准 Canonical 格式，直接透传
            return Ok(ctx);
        }
    };

    // 获取 Provider
    let provider = ctx.provider.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    // 调用适配器转换
    let upstream_req = adapter
        .canonicalize_request(&canonical, provider)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    // 序列化上游请求体
    let new_body = serde_json::to_vec(&upstream_req.body).map_err(|e| {
        StageError::new(
            ctx.clone(),
            GatewayError::Serialization(e.to_string()),
        )
    })?;

    // 更新上下文
    ctx.body = bytes::Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);

    Ok(ctx)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_placeholder() {
        // 集成测试在端到端测试中覆盖
        assert!(true);
    }
}
