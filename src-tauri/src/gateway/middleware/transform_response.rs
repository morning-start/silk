use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::{CanonicalResponse, ProtocolError, UpstreamResponse};

/// 响应转换中间件
///
/// 根据路由规则的 outbound_protocol 选择适配器，
/// 将上游响应体（JSON）解析为 UpstreamResponse，
/// 然后转为 CanonicalResponse 格式，更新 ctx。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 流式响应已在 dispatch_upstream 中构建，无需转换
    if ctx.upstream_body.is_none() && ctx.response.is_some() {
        return Ok(ctx);
    }

    // 获取出站协议
    let outbound_protocol = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_response".to_string());

    // 获取适配器
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

    // 构建 UpstreamResponse
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

    // 调用适配器转换
    let canonical_resp = adapter
        .parse_response(&upstream_resp)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    // 序列化 CanonicalResponse
    let response_body = serde_json::to_vec(&canonical_resp).map_err(|e| {
        StageError::new(
            ctx.clone(),
            GatewayError::Serialization(e.to_string()),
        )
    })?;

    // 更新上下文
    ctx.upstream_body = Some(bytes::Bytes::from(response_body));
    ctx.upstream_status = Some(axum::http::StatusCode::OK);

    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        assert!(true);
    }
}
