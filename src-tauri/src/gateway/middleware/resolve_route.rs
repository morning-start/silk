use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::schedulers::load_balancer::{LoadBalanceStrategy, LoadBalancedItem, LoadBalancer};
use crate::models::{ModelMappingChannel, Provider};
use crate::persistence::{ModelMappingRepo, ProviderRepo};

use axum::response::IntoResponse;

/// Provider 协议 → 适配器名称映射
///
/// 只支持 4 个核心协议，短名映射到标准名。
const PROTOCOL_ADAPTER_MAP: &[(&str, &str)] = &[
    ("chat", "openai"),
    ("response", "responses"),
    ("message", "messages"),
    ("openai", "openai"),
    ("responses", "responses"),
    ("messages", "messages"),
    ("gemini", "gemini"),
];

/// 用于负载均衡的候选条目（渠道 × 选中模型 展开，模型级权重）
#[derive(Clone)]
struct RouteItem {
    provider_id: String,
    /// 选中的远程模型名（空 = 使用请求原模型名）
    model: String,
    weight: i64,
}

impl LoadBalancedItem for RouteItem {
    fn weight(&self) -> i64 {
        self.weight.max(1)
    }
    fn enabled(&self) -> bool {
        true
    }
}

/// 将渠道列表展开为模型级候选条目（渠道 × 选中模型）
fn expand_route_items(channels: &[ModelMappingChannel]) -> Vec<RouteItem> {
    channels
        .iter()
        .flat_map(|c| {
            let models = c.selected_models_vec();
            if models.is_empty() {
                // 空模型列表 = 使用 mapping 的 model_name（请求原模型名），权重 1
                vec![RouteItem {
                    provider_id: c.provider_id.clone(),
                    model: String::new(),
                    weight: 1,
                }]
            } else {
                models
                    .into_iter()
                    .map(|m| RouteItem {
                        provider_id: c.provider_id.clone(),
                        model: m.name,
                        weight: m.weight,
                    })
                    .collect()
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 公共入口
// ---------------------------------------------------------------------------

pub async fn run(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 0. /v1/models：直接返回模型列表，不路由到上游
    if ctx.path == "/v1/models" {
        return handle_models_listing(ctx).await;
    }

    // 1. 优先通过请求体中的 model 字段做模型映射路由
    if let Some(json) = ctx.get_parsed_body().cloned() {
        if let Some(result) = try_model_mapping_route(runtime, ctx.clone(), &json, error_ctx).await? {
            return Ok(result);
        }
    }

    // 2. 降级：通过默认 Provider 兜底
    match try_settings_default(runtime, ctx.clone()).await {
        Ok(ctx) => Ok(ctx),
        Err(_) => {
            // 3. 最后兜底：根据路径自动匹配协议，选择任意可用 Provider
            //    利用已有的协议转换能力（transform_request/transform_response）处理协议不匹配
            try_path_based_default(runtime, ctx).await
        }
    }
}

// ===== 阶段1：路由匹配（确定候选集和协议） =====

/// 处理 /v1/models 请求：返回本地模型池（短路，不走上游）
async fn handle_models_listing(
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let items = match crate::application::models_listing::list_all_models().await {
        Ok(items) => items,
        Err(e) => {
            tracing::warn!(%e, "查询全量模型列表失败");
            Vec::new()
        }
    };

    let data: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "id": item.id,
                "object": item.object,
                "created": item.created,
                "owned_by": item.owned_by,
            })
        })
        .collect();

    let resp = axum::Json(serde_json::json!({ "object": "list", "data": data }));
    ctx.response = Some(resp.into_response());
    Ok(ctx)
}

/// 路由匹配（1）：通过请求体 model 字段匹配模型映射，用负载均衡选 provider
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

    // 路由匹配：查模型映射表
    let mapping = match ModelMappingRepo::find_by_model_name(&runtime.pool, &request_model).await {
        Ok(Some(m)) if m.enabled != 0 => m,
        _ => return Ok(None),
    };

    let channels = match ModelMappingRepo::find_enabled_channels(&runtime.pool, &mapping.id).await {
        Ok(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };

    // 候选选择：负载均衡从可用渠道中选 provider
    ctx.channels_available = channels.iter().map(|c| c.provider_id.clone()).collect();

    // 候选选择：负载均衡
    let (selected_id, selected_model) =
        match select_via_load_balancer(runtime, &mapping.id, &channels, &mapping.strategy) {
            Some(s) => s,
            None => return Ok(None),
        };

    // 应用选中渠道的模型覆盖
    apply_model_override(&mut ctx, &request_model, &selected_model);

    // 上下文填充：加载 provider，推断协议
    let provider = load_provider_with_cache(runtime, &selected_id, error_ctx).await?;
    let inbound = detect_inbound_protocol(&ctx.path, json).to_string();
    let outbound = resolve_protocol_adapter(&provider);
    fill_routing_context(&mut ctx, provider, inbound, outbound);

    Ok(Some(ctx))
}

/// 路由匹配（2）：settings 中的默认 Provider 兜底
async fn try_settings_default(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();
    let settings = runtime.settings.read().await;

    if let Some(ref default_provider_id) = settings.default_provider_id {
        if !default_provider_id.is_empty() {
            let provider = load_provider_with_cache(runtime, default_provider_id, error_ctx.clone()).await?;
            let outbound = resolve_protocol_adapter(&provider);
            let body_json = ctx.get_parsed_body().cloned().unwrap_or(serde_json::Value::Null);
            let inbound = detect_inbound_protocol(&ctx.path, &body_json).to_string();
            ctx.channels_available = all_enabled_provider_ids(runtime).await;
            fill_routing_context(&mut ctx, provider, inbound, outbound);
            return Ok(ctx);
        }
    }

    Err(StageError::new(
        error_ctx.clone(),
        GatewayError::NotFound(format!("未找到匹配路由: {} {}", ctx.method, ctx.path)),
    ))
}

/// 路由匹配（3）：路径兜底，选择任意启用 Provider
///
/// 当没有显式路由规则或模型映射匹配时使用。
/// 检测入站协议 → 选择任意启用 Provider → 利用现有协议转换能力处理协议不匹配。
async fn try_path_based_default(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 路由匹配：路径推断协议
    let body_json = ctx.get_parsed_body().cloned().unwrap_or(serde_json::Value::Null);
    let inbound = detect_inbound_protocol(&ctx.path, &body_json).to_string();

    // 候选选择：取第一个可用 Provider
    let providers = match ProviderRepo::find_enabled(&runtime.pool).await {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => {
            return Err(StageError::new(
                error_ctx,
                GatewayError::NotFound(
                    "未找到可用的渠道（Provider），请先添加并启用至少一个渠道".to_string(),
                ),
            ));
        }
        Err(e) => {
            return Err(StageError::new(error_ctx, GatewayError::Database(e)));
        }
    };

    // 填充渠道列表以支持 3 级回退
    ctx.channels_available = providers.iter().map(|p| p.id.clone()).collect();

    let provider = providers.into_iter().next().unwrap();
    let outbound = resolve_protocol_adapter(&provider);

    tracing::info!(
        "[path_based_default] inbound={inbound}, outbound={outbound}, provider={}",
        provider.name
    );

    // 上下文填充
    fill_routing_context(&mut ctx, provider, inbound, outbound);

    Ok(ctx)
}

// ===== 阶段2：候选选择（从候选中选一个 Provider） =====

/// 从模型映射渠道中通过负载均衡选择一个渠道和模型
fn select_via_load_balancer(
    runtime: &GatewayContext,
    mapping_id: &str,
    channels: &[ModelMappingChannel],
    strategy: &str,
) -> Option<(String, String)> {
    let strategy = LoadBalanceStrategy::parse(strategy);
    let items = expand_route_items(channels);
    let state = runtime.load_balancer_state(&format!("mapping:{mapping_id}"));
    let balancer = LoadBalancer::with_shared_state(items, strategy, state);
    let selected = balancer.select()?;

    Some((selected.provider_id.clone(), selected.model.clone()))
}

// ---------------------------------------------------------------------------
// 候选选择辅助：失败回退换渠道
// ---------------------------------------------------------------------------

/// 失败回退：从 channels_available 中选择下一个未失败的渠道
///
/// 在 dispatch_upstream 全部重试耗尽后调用。返回新的 provider，或 None（所有渠道已尝试过）。
pub async fn try_next_channel(
    runtime: &GatewayContext,
    mut ctx: RequestContext,
) -> Option<RequestContext> {
    // 候选选择：从 channels_available 中找第一个未失败的 provider
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

    // 重新计算远程模型覆盖（每个渠道可能有不同的 selected_models，回退时按权重选模型）
    let original_model = ctx
        .get_parsed_body()
        .and_then(|json| json.get("model")?.as_str().map(|s| s.to_string()));
    if let Some(ref original_model) = original_model {
        if let Ok(Some(mapping)) =
            ModelMappingRepo::find_by_model_name(&runtime.pool, original_model).await
        {
            if mapping.enabled != 0 {
                if let Ok(channels) =
                    ModelMappingRepo::find_enabled_channels(&runtime.pool, &mapping.id).await
                {
                    if let Some(channel) = channels.iter().find(|c| c.provider_id == next_provider_id) {
                        // 在该渠道内按策略（含权重）重新选模型
                        let strategy = LoadBalanceStrategy::parse(&mapping.strategy);
                        let items = expand_route_items(std::slice::from_ref(channel));
                        let state = runtime.load_balancer_state(&format!(
                            "mapping:{}:channel:{}",
                            mapping.id, next_provider_id
                        ));
                        let balancer = LoadBalancer::with_shared_state(items, strategy, state);
                        let selected_model = balancer
                            .select()
                            .map(|item| item.model.clone())
                            .unwrap_or_default();
                        apply_model_override(&mut ctx, original_model, &selected_model);
                    }
                }
            }
        }
    }

    // 上下文填充（使用缓存的请求体避免被覆盖干扰）
    let inbound_body = ctx.get_parsed_body().cloned().unwrap_or_default();
    let inbound = detect_inbound_protocol(&ctx.path, &inbound_body).to_string();
    let outbound = resolve_protocol_adapter(&provider);
    fill_routing_context(&mut ctx, provider, inbound, outbound);
    // 重置 Key 相关的失败记录（新渠道从头开始试 Key）
    ctx.failed_keys.clear();
    ctx.selected_api_key = None;
    ctx.selected_key_encrypted = None;

    Some(ctx)
}

// ===== 阶段3：上下文填充：统一写入路由决策结果 =====

/// 将路由决策结果写入请求上下文（provider + 协议）
///
/// 所有 try_* 函数在确定 provider 和协议后，统一通过此函数填充上下文，
/// 避免在各个路由分支中重复散落相同的字段赋值。
fn fill_routing_context(
    ctx: &mut RequestContext,
    provider: Provider,
    inbound: String,
    outbound: String,
) {
    ctx.provider = Some(provider);
    ctx.inbound_protocol = Some(inbound);
    ctx.outbound_protocol = Some(outbound);
}

// ===== 工具函数：协议推断 =====

/// 从请求路径 + 请求体中检测客户端使用的入站协议
///
/// 检测优先级：
/// 1. 路径匹配（优先）：/v1/chat/completions → openai,
///    /v1/responses → responses, /v1/messages → messages
/// 2. 请求体 JSON 顶层键（兜底）：有 "input" → responses,
///    有 "messages" → openai, 其他 → openai
fn detect_inbound_protocol(path: &str, body: &serde_json::Value) -> &'static str {
    match path {
        "/v1/chat/completions" => return "openai",
        "/v1/responses" => return "responses",
        "/v1/messages" | "/v1/anthropic" => return "messages",
        _ => {}
    }
    if body.get("input").is_some() {
        "responses"
    } else if body.get("messages").is_some() {
        // messages 字段可能来自 OpenAI Chat 或 Claude Messages
        // 通过 model 前缀区分：claude- 开头 → messages，否则 → openai
        if let Some(model) = body.get("model").and_then(|v| v.as_str()) {
            if model.starts_with("claude-") {
                return "messages";
            }
        }
        "openai"
    } else {
        "openai"
    }
}

/// 根据 Provider 的 protocols 字段解析对应的适配器名称
///
/// 取第一个支持的协议映射到 adapter，例如：
/// - ["chat"] → "openai"
/// - ["response"] → "responses"
/// - ["message"] → "messages"
/// - 不支持任何协议或未知协议 → 默认 "openai"
fn resolve_protocol_adapter(provider: &Provider) -> String {
    let protocols = provider.protocols_vec();
    for protocol in &protocols {
        for &(key, adapter) in PROTOCOL_ADAPTER_MAP {
            if protocol == key {
                return adapter.to_string();
            }
        }
    }
    // 默认使用 openai
    "openai".to_string()
}

// ===== 工具函数：缓存加载 =====

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

// ===== 工具函数：模型覆盖 =====

/// 应用模型覆盖逻辑
fn apply_model_override(
    ctx: &mut RequestContext,
    original_model: &str,
    selected_model: &str,
) {
    // 恢复为原始客户端请求体，清除可能在上一次尝试中设置的覆盖
    ctx.request_body = ctx.client_body.clone();
    ctx.parsed_body = None;
    ctx.remote_model_override = None;

    // 选中模型为空（渠道未勾选模型）= 使用请求原模型名，无需覆盖
    if !selected_model.is_empty() && selected_model != original_model {
        if let Some(mut json) = ctx.get_parsed_body().cloned() {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("model".to_string(), serde_json::Value::String(selected_model.to_string()));
                let _ = ctx.update_body(json);
            }
        }
        ctx.remote_model_override = Some(selected_model.to_string());
    }
}

/// 查询所有已启用的 Provider ID 列表（用于 3 级回退的渠道填充）
async fn all_enabled_provider_ids(runtime: &GatewayContext) -> Vec<String> {
    match ProviderRepo::find_enabled(&runtime.pool).await {
        Ok(providers) => providers.into_iter().map(|p| p.id).collect(),
        Err(_) => Vec::new(),
    }
}
