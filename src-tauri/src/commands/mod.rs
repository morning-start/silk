mod types;
pub use types::*;

use tauri::State;

use crate::application::gateway_service::{self, GatewayStartResponse, GatewayStatusResponse, GatewayStopResponse};
use crate::application::group_service::{
    self as gs, AddMemberPayload, CreateGroupPayload, GroupWithMembersResponse, UpdateGroupPayload,
    UpdateMemberPayload,
};
use crate::application::provider_service::{
    self, CreateProviderPayload, FetchModelsPayload, ProviderModelInfo, ProviderResponse,
    ProviderTestResponse, UpdateProviderPayload,
};
use crate::application::routing_service::{
    self, CreateRoutingRulePayload, RoutingRuleResponse, UpdateRoutingRulePayload,
};
use crate::application::settings_service::{self, GatewaySettingsResponse, UpdateSettingsPayload};
use crate::models::{GroupMember, NewGatewayKey, NewModelMapping, ProviderGroup, UpdateGatewayKey, UpdateModelMapping};
use crate::persistence::{GatewayKeyRepo, LogRepo, ModelMappingRepo, StatsRepo};
use crate::AppState;

// ============================================================================
// Gateway Commands
// ============================================================================

#[tauri::command]
pub async fn gateway_status(state: State<'_, AppState>) -> Result<GatewayStatusResponse, String> {
    gateway_service::status(state.inner()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gateway_start(state: State<'_, AppState>) -> Result<GatewayStartResponse, String> {
    gateway_service::start(state.inner()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gateway_stop(state: State<'_, AppState>) -> Result<GatewayStopResponse, String> {
    gateway_service::stop(state.inner()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gateway_restart(state: State<'_, AppState>) -> Result<GatewayStartResponse, String> {
    gateway_service::restart(state.inner()).await.map_err(|e| e.to_string())
}

// ============================================================================
// Settings Commands
// ============================================================================

#[tauri::command]
pub async fn get_gateway_settings() -> Result<GatewaySettingsResponse, String> {
    settings_service::get().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_gateway_settings(
    state: State<'_, AppState>,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, String> {
    settings_service::update(state.inner(), payload).await.map_err(|e| e.to_string())
}

// ============================================================================
// Provider Commands
// ============================================================================

#[tauri::command]
pub async fn list_providers() -> Result<Vec<ProviderResponse>, String> {
    provider_service::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_provider(id: String) -> Result<ProviderResponse, String> {
    provider_service::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_provider(
    state: State<'_, AppState>,
    payload: CreateProviderPayload,
) -> Result<ProviderResponse, String> {
    provider_service::create(state.inner(), payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateProviderPayload,
) -> Result<ProviderResponse, String> {
    provider_service::update(state.inner(), id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProviderTestResponse, String> {
    provider_service::test(state.inner(), id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    provider_service::delete(state.inner(), id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_provider_models(
    payload: FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, String> {
    provider_service::fetch_models(payload).await.map_err(|e| e.to_string())
}

// ============================================================================
// Routing Rule Commands
// ============================================================================

#[tauri::command]
pub async fn list_routing_rules() -> Result<Vec<RoutingRuleResponse>, String> {
    routing_service::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_routing_rule(id: String) -> Result<RoutingRuleResponse, String> {
    routing_service::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_routing_rule(
    state: State<'_, AppState>,
    payload: CreateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    routing_service::create(state.inner(), payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_routing_rule(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    routing_service::update(state.inner(), id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_routing_rule(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    routing_service::delete(state.inner(), id).await.map_err(|e| e.to_string())
}

// ============================================================================
// Group Commands
// ============================================================================

#[tauri::command]
pub async fn list_groups() -> Result<Vec<ProviderGroup>, String> {
    gs::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_groups_by_model(model_name: String) -> Result<Vec<ProviderGroup>, String> {
    gs::find_by_model(model_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_group(id: String) -> Result<GroupWithMembersResponse, String> {
    gs::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    payload: CreateGroupPayload,
) -> Result<ProviderGroup, String> {
    gs::create(state.inner(), payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_group(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateGroupPayload,
) -> Result<ProviderGroup, String> {
    gs::update(state.inner(), id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    gs::delete(state.inner(), id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: String,
    payload: AddMemberPayload,
) -> Result<GroupMember, String> {
    gs::add_member(state.inner(), group_id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_group_member(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateMemberPayload,
) -> Result<GroupMember, String> {
    gs::update_member(state.inner(), id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_group_member(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    gs::remove_member(state.inner(), id).await.map_err(|e| e.to_string())
}

// ============================================================================
// Log Commands
// ============================================================================

#[tauri::command]
pub async fn list_logs(
    state: State<'_, AppState>,
    payload: ListLogsPayload,
) -> Result<ListLogsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = payload.limit.unwrap_or(50).clamp(1, 500);
    let offset = payload.offset.unwrap_or(0);
    let cache = state.provider_name_cache.read().await;

    let logs = LogRepo::find_paginated(pool, limit, offset)
        .await
        .map_err(|e| format!("查询日志失败: {e}"))?;
    let total = LogRepo::count(pool)
        .await
        .map_err(|e| format!("查询日志总数失败: {e}"))?;

    Ok(ListLogsResponse {
        logs: logs.into_iter().map(|l| LogResponse::from_log(l, &cache)).collect(),
        total,
        limit,
        offset,
    })
}

#[tauri::command]
pub async fn cleanup_logs(
    _state: State<'_, AppState>,
    payload: CleanupLogsPayload,
) -> Result<u64, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let before = chrono::Utc::now().naive_utc() - chrono::Duration::days(payload.before_days);
    LogRepo::delete_before(pool, before)
        .await
        .map_err(|e| format!("清理日志失败: {e}"))
}

#[tauri::command]
pub async fn clear_all_logs(_state: State<'_, AppState>) -> Result<u64, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    LogRepo::delete_all(pool)
        .await
        .map_err(|e| format!("清空日志失败: {e}"))
}

#[tauri::command]
pub async fn export_logs_csv(
    _state: State<'_, AppState>,
    payload: ExportLogsPayload,
) -> Result<ExportLogsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = payload.limit.unwrap_or(10000);
    let offset = payload.offset.unwrap_or(0);

    let logs = if let Some(provider_id) = &payload.provider_id {
        LogRepo::find_by_provider(pool, provider_id, limit)
            .await
            .map_err(|e| format!("查询日志失败: {e}"))?
    } else {
        LogRepo::find_paginated(pool, limit, offset)
            .await
            .map_err(|e| format!("查询日志失败: {e}"))?
    };

    let mut csv_content = String::new();
    csv_content.push_str("id,request_id,timestamp,method,path,status_code,duration_ms,provider_id,model_used,tokens_input,tokens_output,error_message\n");

    for log in &logs {
        csv_content.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            log.id,
            log.request_id,
            log.timestamp,
            log.method,
            log.path,
            log.status_code.unwrap_or(0),
            log.duration_ms.unwrap_or(0),
            log.provider_id.as_deref().unwrap_or(""),
            log.model_used.as_deref().unwrap_or(""),
            log.tokens_input.unwrap_or(0),
            log.tokens_output.unwrap_or(0),
            log.error_message.as_deref().unwrap_or(""),
        ));
    }

    let file_path = payload.file_path.unwrap_or_else(|| {
        format!("silk_logs_{}.csv", chrono::Utc::now().format("%Y%m%d_%H%M%S"))
    });
    tokio::fs::write(&file_path, &csv_content)
        .await
        .map_err(|e| format!("写入 CSV 文件失败: {e}"))?;

    Ok(ExportLogsResponse { file_path, exported_count: logs.len() as u64 })
}

// ============================================================================
// Stats Commands
// ============================================================================

#[tauri::command]
pub async fn dashboard_stats(
    _state: State<'_, AppState>,
) -> Result<DashboardStatsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let today_requests = StatsRepo::today_request_count(pool)
        .await.map_err(|e| format!("查询今日请求数失败: {e}"))?;
    let today_success = StatsRepo::today_success_count(pool)
        .await.map_err(|e| format!("查询今日成功数失败: {e}"))?;
    let today_avg_duration = StatsRepo::today_avg_duration_ms(pool)
        .await.map_err(|e| format!("查询今日平均响应时间失败: {e}"))?;
    let today_tokens = StatsRepo::today_total_tokens(pool)
        .await.map_err(|e| format!("查询今日 Token 消耗失败: {e}"))?;
    let active_providers = StatsRepo::today_active_providers(pool)
        .await.map_err(|e| format!("查询活跃服务商数失败: {e}"))?;
    let total_requests = StatsRepo::total_request_count(pool)
        .await.map_err(|e| format!("查询总请求数失败: {e}"))?;
    let yesterday_requests = StatsRepo::yesterday_request_count(pool)
        .await.map_err(|e| format!("查询昨日请求数失败: {e}"))?;

    Ok(DashboardStatsResponse {
        today_requests,
        today_success,
        today_avg_duration_ms: today_avg_duration.unwrap_or(0.0),
        today_tokens,
        active_providers,
        total_requests,
        yesterday_requests,
    })
}

#[tauri::command]
pub async fn recent_requests(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<LogResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(20);
    let logs = StatsRepo::recent_requests(pool, limit)
        .await.map_err(|e| format!("查询最近请求失败: {e}"))?;
    Ok(logs.into_iter().map(LogResponse::from).collect())
}

#[tauri::command]
pub async fn stats_by_provider(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<ProviderStatsResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(10);
    let stats = StatsRepo::stats_by_provider(pool, limit)
        .await.map_err(|e| format!("查询服务商统计失败: {e}"))?;
    Ok(stats.into_iter().map(ProviderStatsResponse::from).collect())
}

#[tauri::command]
pub async fn hourly_stats(
    _state: State<'_, AppState>,
    hours: Option<i64>,
) -> Result<Vec<HourlyStatsResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let hours = hours.unwrap_or(24);
    let stats = StatsRepo::hourly_stats(pool, hours)
        .await.map_err(|e| format!("查询时序统计失败: {e}"))?;
    Ok(stats.into_iter().map(HourlyStatsResponse::from).collect())
}

// ============================================================================
// Gateway Key Commands
// ============================================================================

#[tauri::command]
pub async fn list_gateway_keys(
    _state: State<'_, AppState>,
) -> Result<Vec<GatewayKeyResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let keys = GatewayKeyRepo::find_all(pool)
        .await.map_err(|e| format!("查询 Key 失败: {e}"))?;
    Ok(keys.into_iter().map(GatewayKeyResponse::from).collect())
}

#[tauri::command]
pub async fn get_gateway_key(
    _state: State<'_, AppState>,
    id: String,
) -> Result<GatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let key = GatewayKeyRepo::find_by_id(pool, &id)
        .await.map_err(|e| format!("查询 Key 失败: {e}"))?
        .ok_or("Key 不存在")?;
    Ok(GatewayKeyResponse::from(key))
}

#[tauri::command]
pub async fn create_gateway_key(
    _state: State<'_, AppState>,
    payload: CreateGatewayKeyPayload,
) -> Result<CreateGatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let key_value = if payload.key_value.is_empty() {
        format!("sk-gw-{}", uuid::Uuid::new_v4().to_string().replace("-", ""))
    } else {
        payload.key_value
    };
    let new = NewGatewayKey {
        name: payload.name,
        key_value,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };
    let (key, plain_key) = GatewayKeyRepo::create(pool, &new)
        .await.map_err(|e| format!("创建 Key 失败: {e}"))?;
    Ok(CreateGatewayKeyResponse { key: GatewayKeyResponse::from(key), plain_key })
}

#[tauri::command]
pub async fn update_gateway_key(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateGatewayKeyPayload,
) -> Result<GatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let update = UpdateGatewayKey {
        name: payload.name,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };
    let key = GatewayKeyRepo::update(pool, &id, &update)
        .await.map_err(|e| format!("更新 Key 失败: {e}"))?
        .ok_or("Key 不存在")?;
    Ok(GatewayKeyResponse::from(key))
}

#[tauri::command]
pub async fn delete_gateway_key(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    GatewayKeyRepo::delete(pool, &id)
        .await.map_err(|e| format!("删除 Key 失败: {e}"))
}

// ============================================================================
// Model Mapping Commands
// ============================================================================

#[tauri::command]
pub async fn list_model_mappings(
    _state: State<'_, AppState>,
) -> Result<Vec<ModelMappingResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mappings = ModelMappingRepo::find_all(pool)
        .await.map_err(|e| format!("查询模型映射失败: {e}"))?;
    let mut result = Vec::with_capacity(mappings.len());
    for m in mappings {
        let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &m.id)
            .await.unwrap_or_default();
        result.push(ModelMappingResponse::from_model(m, channels));
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_model_mapping(
    _state: State<'_, AppState>,
    id: String,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mapping = ModelMappingRepo::find_by_id(pool, &id)
        .await.map_err(|e| format!("查询模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await.unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

#[tauri::command]
pub async fn find_model_mapping_by_name(
    _state: State<'_, AppState>,
    model_name: String,
) -> Result<Option<ModelMappingResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mapping = match ModelMappingRepo::find_by_model_name(pool, &model_name)
        .await.map_err(|e| format!("查询模型映射失败: {e}"))?
    {
        Some(m) => m,
        None => return Ok(None),
    };
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await.unwrap_or_default();
    Ok(Some(ModelMappingResponse::from_model(mapping, channels)))
}

#[tauri::command]
pub async fn create_model_mapping(
    _state: State<'_, AppState>,
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let new = NewModelMapping {
        model_name: payload.model_name,
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
        capabilities: payload.capabilities,
        description: payload.description,
        strategy: payload.strategy,
        enabled: payload.enabled,
        channels: payload.channels,
    };
    let mapping = ModelMappingRepo::create(pool, &new)
        .await.map_err(|e| format!("创建模型映射失败: {e}"))?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await.unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

#[tauri::command]
pub async fn update_model_mapping(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let update = UpdateModelMapping {
        model_name: payload.model_name,
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
        capabilities: payload.capabilities,
        description: payload.description,
        strategy: payload.strategy,
        enabled: payload.enabled,
        channels: payload.channels,
    };
    let mapping = ModelMappingRepo::update(pool, &id, &update)
        .await.map_err(|e| format!("更新模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await.unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

#[tauri::command]
pub async fn delete_model_mapping(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    ModelMappingRepo::delete(pool, &id)
        .await.map_err(|e| format!("删除模型映射失败: {e}"))
}

#[tauri::command]
pub async fn get_group_providers(
    _state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<crate::models::GroupProviderInfo>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    ModelMappingRepo::find_group_providers(pool, &group_id)
        .await.map_err(|e| format!("查询分组渠道失败: {e}"))
}