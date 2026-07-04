use tauri::State;

use crate::application::log_service;
use crate::application::log_service::{ExportLogsResponse, ListLogsResponse};
use crate::commands::{CleanupLogsPayload, ExportLogsPayload, ListLogsPayload};
use crate::AppState;

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
