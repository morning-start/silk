use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::load_balancer::{LoadBalanceStrategy, LoadBalancedItem, LoadBalancer};
use crate::models::Provider;
use crate::persistence::{ModelMappingRepo, ProviderRepo};

use axum::response::IntoResponse;

/// Provider 协议 → 适配器名称映射
///
/// 同时支持短名（chat/response/message）和完整适配器名（openai_chat 等），
/// 完整名直接 identity 映射，避免前端传完整名时无法识别。
const PROTOCOL_ADAPTER_MAP: &[(&str, &str)] = &[
    ("chat", "openai_chat"),
    ("response", "openai_response"),
    ("message", "claude_messages"),
    ("openai_chat", "openai_chat"),
    ("openai_response", "openai_response"),
    ("claude_messages", "claude_messages"),
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
        return handle_models_listing(runtime, ctx).await;
    }

    // 1. 优先通过请求体中的 model 字段做模型映射路由
    let body_text = String::from_utf8_lossy(&ctx.request_body).into_owned();

    if body_text.trim().starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_text) {
            if let Some(result) = try_model_mapping_route(runtime, ctx, &json, error_ctx).await? {
                return Ok(result);
            }
        }
    }

    // 2. 降级：通过 RoutingRule 匹配 + 默认 Provider 兜底
    try_route_fallback(runtime, ctx).await
}

/// 处理 /v1/models 请求：返回本地模型池
async fn handle_models_listing(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let models = match ModelMappingRepo::find_enabled(&runtime.pool).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(%e, "查询模型列表失败");
            Vec::new()
        }
    };
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
    Ok(ctx)
}

/// 尝试通过请求体 model 字段匹配模型映射
///
/// 返回 Ok(Some(ctx)) 表示命中并路由成功，Ok(None) 表示未命中需要降级
async fn try_model_mapping_route(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
    json: &serde_json::Value,
    error_ctx: RequestContext,
) -> Result<Option<RequestContext>, StageError> {
    let request_model = match json.get("model").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => return Ok(None),
    };

    let mapping = match ModelMappingRepo::find_by_model_name(&runtime.pool, &request_model).await {
        Ok(Some(m)) if m.enabled != 0 => m,
        _ => return Ok(None),
    };

    let channels = match ModelMappingRepo::find_enabled_channels(&runtime.pool, &mapping.id).await {
        Ok(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };

    ctx.channels_available = channels.iter().map(|c| c.provider_id.clone()).collect();

    let strategy = LoadBalanceStrategy::from_str(&mapping.strategy);
    let items: Vec<ChannelItem> = channels.iter().map(|c| ChannelItem { provider_id: c.provider_id.clone() }).collect();
    let balancer = LoadBalancer::new(items, strategy);

    let selected = match balancer.select() {
        Some(s) => s,
        None => return Ok(None),
    };

    let selected_channel = channels.iter().find(|c| c.provider_id == selected.provider_id);
    let remote_model = selected_channel.and_then(|c| {
        let sm = c.selected_models_vec();
        if sm.contains(&request_model) { None } else { sm.first().cloned() }
    });

    let provider = load_provider_with_cache(runtime, &selected.provider_id, error_ctx).await?;

    if let Some(ref remote_model) = remote_model {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(
            &String::from_utf8_lossy(&ctx.request_body)
        ) {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("model".to_string(), serde_json::Value::String(remote_model.clone()));
                if let Ok(new_body) = serde_json::to_vec(&json) {
                    ctx.request_body = bytes::Bytes::from(new_body);
                }
            }
        }
        ctx.remote_model_override = Some(remote_model.clone());
    }

    ctx.provider = Some(provider);
    ctx.inbound_protocol = Some(detect_inbound_protocol(&ctx.path, json).to_string());
    ctx.outbound_protocol = Some(resolve_protocol_adapter(ctx.provider.as_ref().unwrap()));
    ctx.adapter_registry = runtime.adapter_registry.clone();
    Ok(Some(ctx))
}

/// 降级路由：RoutingRule 匹配 → default_route_id → default_provider_id
async fn try_route_fallback(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();
        .route_manager
        .resolve(
            ctx.host.as_deref(),
            ctx.method.as_str(),
            &ctx.path,
            ctx.content_type.as_deref(),
        )
        .await;

    let route = match route_result {
        Some(r) => r,
        None => {
            // 2.1 路由未命中时，尝试使用 settings 中的默认路由/Provider 兜底
            let settings = runtime.settings.read().await;

            // 优先尝试 default_route_id
            if let Some(ref default_route_id) = settings.default_route_id {
                if !default_route_id.is_empty() {
                    if let Ok(Some(fallback_route)) =
                        crate::persistence::RoutingRuleRepo::find_by_id(&runtime.pool, default_route_id).await
                    {
                        let provider_id = runtime
                            .route_manager
                            .resolve_provider_id(&fallback_route, &runtime.group_manager)
                            .await;

                        if let Some(pid) = provider_id {
                            let provider = load_provider_with_cache(runtime, &pid, error_ctx.clone()).await?;
                            ctx.route = Some(fallback_route.clone());
                            ctx.provider = Some(provider);
                            ctx.inbound_protocol = fallback_route.inbound_protocol.clone();
                            ctx.outbound_protocol = fallback_route.outbound_protocol.clone();
                            ctx.adapter_registry = runtime.adapter_registry.clone();
                            return Ok(ctx);
                        }
                    }
                }
            }

            // 再尝试 default_provider_id
            if let Some(ref default_provider_id) = settings.default_provider_id {
                if !default_provider_id.is_empty() {
                    let provider = load_provider_with_cache(runtime, default_provider_id, error_ctx.clone()).await?;
                    // 使用 Provider 的第一个协议做 outbound
                    let outbound = resolve_protocol_adapter(&provider);
                    // 用真实请求体推断入站协议（避免误判为 openai_chat）
                    let body_json = serde_json::from_slice::<serde_json::Value>(&ctx.request_body).unwrap_or(serde_json::Value::Null);
                    let inbound = detect_inbound_protocol(&ctx.path, &body_json);
                    ctx.provider = Some(provider);
                    ctx.inbound_protocol = Some(inbound.to_string());
                    ctx.outbound_protocol = Some(outbound);
                    ctx.adapter_registry = runtime.adapter_registry.clone();
                    return Ok(ctx);
                }
            }

            return Err(StageError::new(
                error_ctx.clone(),
                GatewayError::NotFound(format!("未找到匹配路由: {} {}", ctx.method, ctx.path)),
            ));
        }
    };

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

    // 重新计算远程模型覆盖（每个渠道可能有不同的 selected_models）
    let body_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&ctx.client_body)).unwrap_or_default();
    if let Some(original_model) = body_json
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        if let Ok(Some(mapping)) =
            ModelMappingRepo::find_by_model_name(&runtime.pool, &original_model).await
        {
            if mapping.enabled != 0 {
                if let Ok(channels) =
                    ModelMappingRepo::find_enabled_channels(&runtime.pool, &mapping.id).await
                {
                    if let Some(channel) = channels.iter().find(|c| c.provider_id == next_provider_id)
                    {
                        let sm = channel.selected_models_vec();
                        if !sm.contains(&original_model) {
                            if let Some(remote_model) = sm.first().cloned() {
                                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(
                                    &String::from_utf8_lossy(&ctx.request_body),
                                ) {
                                    if let Some(obj) = json.as_object_mut() {
                                        obj.insert(
                                            "model".to_string(),
                                            serde_json::Value::String(remote_model.clone()),
                                        );
                                        if let Ok(new_body) = serde_json::to_vec(&json) {
                                            ctx.request_body = bytes::Bytes::from(new_body);
                                        }
                                    }
                                }
                                ctx.remote_model_override = Some(remote_model);
                            }
                        }
                    }
                }
            }
        }
    }

    // 协议推断（使用 client_body 避免被覆盖干扰）
    let inbound_adapter = detect_inbound_protocol(&ctx.path, &body_json);
    let outbound_adapter = resolve_protocol_adapter(&provider);
    ctx.provider = Some(provider);
    ctx.inbound_protocol = Some(inbound_adapter.to_string());
    ctx.outbound_protocol = Some(outbound_adapter);
    // 重置 Key 相关的失败记录（新渠道从头开始试 Key）
    ctx.failed_keys.clear();
    ctx.selected_api_key = None;

    Some(ctx)
}

/// 从请求路径 + 请求体中检测客户端使用的入站协议
///
/// 检测优先级：
/// 1. 路径匹配（优先）：/v1/chat/completions → openai_chat,
///    /v1/responses → openai_response, /v1/messages → claude_messages
/// 2. 请求体 JSON 顶层键（兜底）：有 "input" → openai_response,
///    有 "messages" → openai_chat, 其他 → openai_chat
fn detect_inbound_protocol(path: &str, body: &serde_json::Value) -> &'static str {
    match path {
        "/v1/chat/completions" => return "openai_chat",
        "/v1/responses" => return "openai_response",
        "/v1/messages" => return "claude_messages",
        _ => {}
    }
    if body.get("input").is_some() {
        "openai_response"
    } else if body.get("messages").is_some() {
        // messages 字段可能来自 OpenAI Chat 或 Claude Messages
        // 通过 model 前缀区分：claude- 开头 → claude_messages，否则 → openai_chat
        if let Some(model) = body.get("model").and_then(|v| v.as_str()) {
            if model.starts_with("claude-") {
                return "claude_messages";
            }
        }
        "openai_chat"
    } else {
        "openai_chat"
    }
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
