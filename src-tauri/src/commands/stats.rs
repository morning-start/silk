use tauri::State;

use crate::application::log_service::{self, LogResponse};
use crate::application::stats_service::{self, DashboardStatsResponse, HourlyStatsResponse, ProviderStatsResponse};
use crate::AppState;

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
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<LogResponse>, String> {
    log_service::recent_requests(state.inner(), limit)
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
