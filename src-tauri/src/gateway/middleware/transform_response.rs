use crate::gateway::context::RequestContext;
use crate::gateway::pipeline::StageError;

/// 响应转换中间件
///
/// 请求已由 transform_request 强制注入 stream:true，dispatch_upstream 中的
/// handle_sse_response 已通过 SseConverter 在流中逐事件完成协议转换并构建
/// Axum Response。本步骤仅做透传——SSE 响应已就绪，直接返回。
pub async fn run(ctx: RequestContext) -> Result<RequestContext, StageError> {
    // 流式 SSE 已由 handle_sse_response 构建响应，跳过
    if ctx.upstream_body.is_none() && ctx.response.is_some() {
        return Ok(ctx);
    }

    // 非流式路径：当前版本强制流式，不应到达此处
    Ok(ctx)
}
