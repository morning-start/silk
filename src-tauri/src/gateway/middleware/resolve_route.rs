use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::persistence::ProviderRepo;

pub async fn run(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 1. 路由匹配
    let route = runtime
        .route_manager
        .resolve(
            ctx.host.as_deref(),
            ctx.method.as_str(),
            &ctx.path,
            ctx.content_type.as_deref(),
        )
        .await
        .ok_or_else(|| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::NotFound(format!("未找到匹配路由: {} {}", ctx.method, ctx.path)),
            )
        })?;

    // 2. 加载 Provider（优先缓存）
    let provider = load_provider_with_cache(runtime, &route.target_provider_id, error_ctx.clone())
        .await?;

    // 3. 填充上下文
    ctx.route = Some(route.clone());
    ctx.provider = Some(provider);
    ctx.inbound_protocol = route.inbound_protocol.clone();
    ctx.outbound_protocol = route.outbound_protocol.clone();
    ctx.adapter_registry = runtime.adapter_registry.clone();

    Ok(ctx)
}

/// 从缓存加载 Provider，miss 或过期则从 DB 加载并回填缓存
async fn load_provider_with_cache(
    runtime: &GatewayContext,
    provider_id: &str,
    error_ctx: RequestContext,
) -> Result<crate::models::Provider, StageError> {
    // 先查缓存
    if let Some(provider) = runtime.provider_cache.get(provider_id).await {
        return Ok(provider);
    }

    // 缓存 miss → 查 DB
    match ProviderRepo::find_by_id(&runtime.pool, provider_id).await {
        Ok(Some(provider)) => {
            // 回填缓存
            runtime.provider_cache.put(provider.clone()).await;
            Ok(provider)
        }
        Ok(None) => Err(StageError::new(
            error_ctx,
            GatewayError::NotFound(format!("未找到目标 Provider: {provider_id}")),
        )),
        Err(err) => Err(StageError::new(error_ctx, GatewayError::Database(err))),
    }
}
