use crate::gateway::context::RequestContext;
use crate::gateway::pipeline::StageError;

/// 请求转换中间件
///
/// 职责：
/// 1. 应用模型名覆盖（route.model_name_override → provider.model_name）
/// 2. 协议转换准备：标记需要转换的协议类型
/// 3. 未来：Canonical Format → Upstream Protocol 的实际转换
pub async fn run(ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 如果路由规则指定了模型覆盖，记录到上下文中
    // 实际协议转换将在 Phase 3 实现，此处仅做标记
    if let Some(route) = &ctx.route {
        if route.model_name_override.is_some() {
            // 模型覆盖已设置，dispatch_upstream 会使用它
        }
        // 标记协议转换需求
        if route.needs_protocol_conversion() {
            // TODO: Phase 3 实现 Canonical → Upstream 转换
        }
    }

    Ok(ctx)
}
