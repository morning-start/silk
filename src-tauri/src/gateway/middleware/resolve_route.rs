use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::persistence::{ModelMappingRepo, ProviderRepo};

pub async fn run(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 1. 优先通过请求体中的 model 字段做模型映射路由
    let body_cloned = ctx.body.clone();
    let body_text = String::from_utf8_lossy(&body_cloned).into_owned();

    // 简单 JSON 提取 model
    if body_text.trim().starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_text) {
            if let Some(model_name) = json.get("model").and_then(|v| v.as_str()) {
                if let Ok(Some(mapping)) =
                    ModelMappingRepo::find_by_model_name(&runtime.pool, model_name).await
                {
                    if mapping.enabled != 0 {
                        if let Some(group_id) = &mapping.provider_group_id {
                            if let Some(member) =
                                runtime.group_manager.select_provider(group_id).await
                            {
                                // 从缓存或 DB 加载 Provider
                                let provider = if let Some(p) =
                                    runtime.provider_cache.get(&member.provider_id).await
                                {
                                    p
                                } else if let Ok(Some(p)) =
                                    ProviderRepo::find_by_id(&runtime.pool, &member.provider_id).await
                                {
                                    runtime.provider_cache.put(p.clone()).await;
                                    p
                                } else {
                                    return Err(StageError::new(
                                        error_ctx.clone(),
                                        GatewayError::NotFound(format!(
                                            "未找到目标 Provider: {}",
                                            member.provider_id
                                        )),
                                    ));
                                };

                                ctx.provider = Some(provider);
                                ctx.inbound_protocol = Some("openai_chat".to_string());
                                ctx.outbound_protocol = Some("openai_response".to_string());
                                ctx.adapter_registry = runtime.adapter_registry.clone();
                                return Ok(ctx);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. 降级：通过 RoutingRule 匹配（按 path + method）
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

    let provider_id = runtime
        .route_manager
        .resolve_provider_id(&route, &runtime.group_manager)
        .await
        .ok_or_else(|| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::NotFound("无法解析目标 Provider".to_string()),
            )
        })?;

    let provider = load_provider_with_cache(runtime, &provider_id, error_ctx.clone()).await?;

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
    if let Some(provider) = runtime.provider_cache.get(provider_id).await {
        return Ok(provider);
    }

    match ProviderRepo::find_by_id(&runtime.pool, provider_id).await {
        Ok(Some(provider)) => {
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
