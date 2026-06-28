use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::load_balancer::{LoadBalanceStrategy, LoadBalancedItem, LoadBalancer};
use crate::models::Provider;
use crate::persistence::{ModelMappingRepo, ProviderRepo};

use axum::response::IntoResponse;

/// Provider 协议 → 适配器名称映射
const PROTOCOL_ADAPTER_MAP: &[(&str, &str)] = &[
    ("chat", "openai_chat"),
    ("response", "openai_response"),
    ("message", "claude_messages"),
];

/// 用于负载均衡选渠道的轻量条目
#[derive(Clone)]
struct ChannelItem {
    provider_id: String,
}

impl LoadBalancedItem for ChannelItem {
    fn weight(&self) -> i64 {
        1
    }
    fn enabled(&self) -> bool {
        true
    }
}

pub async fn run(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 0. /v1/models：直接返回模型列表，不路由到上游
    if ctx.path == "/v1/models" {
        let models = ModelMappingRepo::find_enabled(&runtime.pool).await.unwrap_or_default();
        let data: Vec<serde_json::Value> = models
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.model_name,
                    "object": "model",
                    "created": m.created_at.and_utc().timestamp(),
                    "owned_by": if m.vendor.is_empty() { "silk" } else { &m.vendor },
                })
            })
            .collect();
        let resp = axum::Json(serde_json::json!({ "object": "list", "data": data }));
        ctx.response = Some(resp.into_response());
        return Ok(ctx);
    }

    // 1. 优先通过请求体中的 model 字段做模型映射路由
    let body_cloned = ctx.body.clone();
    let body_text = String::from_utf8_lossy(&body_cloned).into_owned();

    // 简单 JSON 提取 model
    if body_text.trim().starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_text) {
            if let Some(request_model) = json
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                if let Ok(Some(mapping)) =
                    ModelMappingRepo::find_by_model_name(&runtime.pool, &request_model).await
                {
                    if mapping.enabled != 0 {
                        // 从关联渠道表获取可用渠道，按策略选一个
                        if let Ok(channels) =
                            ModelMappingRepo::find_enabled_channels(&runtime.pool, &mapping.id)
                                .await
                        {
                            if !channels.is_empty() {
                                // 保存可用渠道列表供失败回退使用
                                ctx.channels_available = channels
                                    .iter()
                                    .map(|c| c.provider_id.clone())
                                    .collect::<Vec<_>>();

                                let strategy = LoadBalanceStrategy::from_str(&mapping.strategy);
                                let items: Vec<ChannelItem> = channels
                                    .iter()
                                    .map(|c| ChannelItem {
                                        provider_id: c.provider_id.clone(),
                                    })
                                    .collect();
                                let balancer = LoadBalancer::new(items, strategy);

                                if let Some(selected) = balancer.select() {
                                    // 查找该渠道的 selected_models，确定远程模型名
                                    let selected_channel = channels
                                        .iter()
                                        .find(|c| c.provider_id == selected.provider_id);
                                    let remote_model = selected_channel.and_then(|c| {
                                        let sm = c.selected_models_vec();
                                        if sm.contains(&request_model) {
                                            None // 请求模型已在选中列表中，无需覆盖
                                        } else {
                                            sm.first().cloned() // 使用列表第一个模型名覆盖
                                        }
                                    });

                                    let provider = if let Some(p) =
                                        runtime.provider_cache.get(&selected.provider_id).await
                                    {
                                        p
                                    } else if let Ok(Some(p)) = ProviderRepo::find_by_id(
                                        &runtime.pool,
                                        &selected.provider_id,
                                    )
                                    .await
                                    {
                                        runtime.provider_cache.put(p.clone()).await;
                                        p
                                    } else {
                                        return Err(StageError::new(
                                            error_ctx.clone(),
                                            GatewayError::NotFound(format!(
                                                "未找到目标 Provider: {}",
                                                selected.provider_id
                                            )),
                                        ));
                                    };

                                    // 如果需要覆盖模型名，修改请求体 JSON
                                    if let Some(ref remote_model) = remote_model {
                                        if let Ok(mut json) =
                                            serde_json::from_str::<serde_json::Value>(&body_text)
                                        {
                                            if let Some(obj) = json.as_object_mut() {
                                                obj.insert(
                                                    "model".to_string(),
                                                    serde_json::Value::String(remote_model.clone()),
                                                );
                                                if let Ok(new_body) = serde_json::to_vec(&json) {
                                                    ctx.body = bytes::Bytes::from(new_body);
                                                }
                                            }
                                        }
                                        ctx.remote_model_override = Some(remote_model.clone());
                                    }

                                    // 根据 Provider 支持的协议设置 inbound/outbound 适配器
                                    let adapter = resolve_protocol_adapter(&provider);
                                    ctx.provider = Some(provider);
                                    ctx.inbound_protocol = Some(adapter.clone());
                                    ctx.outbound_protocol = Some(adapter);
                                    ctx.adapter_registry = runtime.adapter_registry.clone();
                                    return Ok(ctx);
                                }
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

/// 失败回退：从 channels_available 中选择下一个未失败的渠道
///
/// 在 dispatch_upstream 全部重试耗尽后调用。返回新的 provider，或 None（所有渠道已尝试过）。
pub async fn try_next_channel(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Option<RequestContext> {
    let _body_text = String::from_utf8_lossy(&ctx.body).into_owned();

    // 从 channels_available 中找第一个未失败的 provider
    let next_provider_id = ctx
        .channels_available
        .iter()
        .find(|pid| !ctx.failed_providers.contains(pid))?
        .clone();

    let provider = if let Some(p) = runtime.provider_cache.get(&next_provider_id).await {
        p
    } else if let Ok(Some(p)) = ProviderRepo::find_by_id(&runtime.pool, &next_provider_id).await {
        runtime.provider_cache.put(p.clone()).await;
        p
    } else {
        return None;
    };

    // 根据 Provider 支持的协议设置 inbound/outbound 适配器
    let adapter = resolve_protocol_adapter(&provider);
    ctx.provider = Some(provider);
    ctx.inbound_protocol = Some(adapter.clone());
    ctx.outbound_protocol = Some(adapter);
    // 重置 Key 相关的失败记录（新渠道从头开始试 Key）
    ctx.failed_keys.clear();
    ctx.selected_api_key = None;

    // 如果渠道的 selected_models 有远程模型覆盖，再次应用
    // （这里简化处理：不重新计算 remote_model_override，使用已经覆盖过的 body 即可）

    Some(ctx)
}

/// 根据 Provider 的 protocols 字段解析对应的适配器名称
///
/// 取第一个支持的协议映射到 adapter，例如：
/// - ["chat"] → "openai_chat"
/// - ["response"] → "openai_response"
/// - ["message"] → "claude_messages"
/// - 不支持任何协议或未知协议 → 默认 "openai_chat"
fn resolve_protocol_adapter(provider: &Provider) -> String {
    let protocols = provider.protocols_vec();
    for protocol in &protocols {
        for &(key, adapter) in PROTOCOL_ADAPTER_MAP {
            if protocol == key {
                return adapter.to_string();
            }
        }
    }
    // 默认使用 openai_chat
    "openai_chat".to_string()
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
