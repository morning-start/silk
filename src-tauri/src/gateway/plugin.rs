use async_trait::async_trait;
use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::pipeline::StageError;

/// 网关插件 Trait，定义生命周期拦截点（Hooks）
#[async_trait]
pub trait GatewayPlugin: Send + Sync {
    /// 插件的唯一名称，用于日志和调试
    fn name(&self) -> &'static str;

    /// 钩子 1：解析路由之前 (Before Route)
    /// - 触发点：刚刚读取完请求体，还未查找数据库 of 路由规则与渠道。
    /// - 适用场景：本地缓存直接拦截返回。
    /// - 拦截机制：如果在该阶段向 `ctx.response` 写入了响应，整个管道将跳过后续步骤直接返回给客户端。
    async fn before_route(
        &self,
        ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        Ok(ctx)
    }

    /// 钩子 2：转发上游之前 (Before Upstream)
    /// - 触发点：已经确定了渠道和上游 Key，并且请求体已转换完毕，即发送网络请求前的一刻。
    /// - 适用场景：大模型厂商 Prompt 缓存标记注入、终端冗余日志过滤、上下文对话窗口截断。
    async fn before_upstream(
        &self,
        ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        Ok(ctx)
    }

    /// 钩子 3：收到响应之后 (After Upstream)
    /// - 触发点：上游第三方响应成功返回，且响应体已被转换后。
    /// - 适用场景：缓存回填写入数据库。
    async fn after_upstream(
        &self,
        ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        Ok(ctx)
    }
}
