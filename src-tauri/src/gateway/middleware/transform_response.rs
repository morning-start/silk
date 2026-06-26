use crate::gateway::context::RequestContext;
use crate::gateway::pipeline::StageError;

/// 响应转换中间件
///
/// 职责：
/// 1. 标记响应需要协议转换
/// 2. 未来：Upstream Protocol → Canonical Format (OpenAI Response) 的实际转换
pub async fn run(ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 如果路由规则启用了协议转换，标记响应需要转换
    if let Some(route) = &ctx.route {
        if route.needs_protocol_conversion() {
            // TODO: Phase 3 实现 Upstream → Canonical (OpenAI Response) 转换
        }
    }

    Ok(ctx)
}
