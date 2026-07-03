mod types;
pub use types::{
    CleanupLogsPayload, ExportLogsPayload, ListLogsPayload,
};
pub use crate::application::provider_service::FetchModelsPayload;

pub use crate::application::gateway_key_service::{
    CreateGatewayKeyPayload, CreateGatewayKeyResponse, GatewayKeyResponse,
    UpdateGatewayKeyPayload,
};
pub use crate::application::log_service::{
    ExportLogsResponse, ListLogsResponse, LogResponse,
};
pub use crate::application::model_mapping_service::{
    CreateModelMappingPayload, ModelMappingResponse, UpdateModelMappingPayload,
};
pub use crate::application::stats_service::{
    DashboardStatsResponse, HourlyStatsResponse, ProviderStatsResponse,
};

use tauri::State;

use crate::application::gateway_key_service as gks;
use crate::application::gateway_service::{self, GatewayStartResponse, GatewayStatusResponse, GatewayStopResponse};
use crate::application::log_service;
use crate::application::model_mapping_service as mms;
use crate::application::provider_service::{
    self, CreateProviderPayload, ProviderModelInfo, ProviderResponse,
    ProviderTestResponse, UpdateProviderPayload,
};
use crate::application::routing_service::{
    self, CreateRoutingRulePayload, RoutingRuleResponse, UpdateRoutingRulePayload,
};
use crate::application::settings_service::{self, GatewaySettingsResponse, UpdateSettingsPayload};
use crate::application::stats_service;
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
    settings_service::get().map_err(|e| e.to_string())
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
// Log Commands
// ============================================================================

#[tauri::command]
pub async fn list_logs(
    state: State<'_, AppState>,
    payload: ListLogsPayload,
) -> Result<ListLogsResponse, String> {
    log_service::list(state.inner(), payload.limit, payload.offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_logs(
    _state: State<'_, AppState>,
    payload: CleanupLogsPayload,
) -> Result<u64, String> {
    log_service::cleanup(payload.before_days)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_all_logs(_state: State<'_, AppState>) -> Result<u64, String> {
    log_service::clear_all()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_logs_csv(
    _state: State<'_, AppState>,
    payload: ExportLogsPayload,
) -> Result<ExportLogsResponse, String> {
    log_service::export_csv(payload.provider_id, payload.limit, payload.offset, payload.file_path)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Stats Commands
// ============================================================================

#[tauri::command]
pub async fn dashboard_stats(
    _state: State<'_, AppState>,
) -> Result<DashboardStatsResponse, String> {
    stats_service::dashboard_stats()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recent_requests(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<LogResponse>, String> {
    log_service::recent_requests(limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stats_by_provider(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<ProviderStatsResponse>, String> {
    let limit = limit.unwrap_or(10);
    stats_service::stats_by_provider(limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hourly_stats(
    _state: State<'_, AppState>,
    hours: Option<i64>,
) -> Result<Vec<HourlyStatsResponse>, String> {
    let hours = hours.unwrap_or(24);
    stats_service::hourly_stats(hours)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Gateway Key Commands
// ============================================================================

#[tauri::command]
pub async fn list_gateway_keys(
    _state: State<'_, AppState>,
) -> Result<Vec<GatewayKeyResponse>, String> {
    gks::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_gateway_key(
    _state: State<'_, AppState>,
    id: String,
) -> Result<GatewayKeyResponse, String> {
    gks::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_gateway_key(
    _state: State<'_, AppState>,
    payload: CreateGatewayKeyPayload,
) -> Result<CreateGatewayKeyResponse, String> {
    gks::create(payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_gateway_key(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateGatewayKeyPayload,
) -> Result<GatewayKeyResponse, String> {
    gks::update(id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_gateway_key(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    gks::delete(id).await.map_err(|e| e.to_string())
}

// ============================================================================
// Model Mapping Commands
// ============================================================================

#[tauri::command]
pub async fn list_model_mappings(
    _state: State<'_, AppState>,
) -> Result<Vec<ModelMappingResponse>, String> {
    mms::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_model_mapping(
    _state: State<'_, AppState>,
    id: String,
) -> Result<ModelMappingResponse, String> {
    mms::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_model_mapping_by_name(
    _state: State<'_, AppState>,
    model_name: String,
) -> Result<Option<ModelMappingResponse>, String> {
    mms::find_by_name(model_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_model_mapping(
    _state: State<'_, AppState>,
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    mms::create(payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_model_mapping(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    mms::update(id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model_mapping(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    mms::delete(id).await.map_err(|e| e.to_string())
}