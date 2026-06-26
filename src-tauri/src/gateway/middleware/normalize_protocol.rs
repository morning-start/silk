use crate::gateway::context::RequestContext;
use crate::gateway::pipeline::StageError;

/// 协议归一化中间件
///
/// 职责：
/// 1. 从路由规则中获取入站/出站协议标签
/// 2. 标记到上下文中，供后续转换使用
/// 3. 未来：可在此处做协议校验/拒绝不兼容的组合
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    if let Some(route) = &ctx.route {
        ctx.inbound_protocol = route.inbound_protocol.clone();
        ctx.outbound_protocol = route.outbound_protocol.clone();
    }

    Ok(ctx)
}
