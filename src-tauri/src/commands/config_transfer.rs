use tauri::State;

use crate::application::config_transfer_service::{
    self, BackupDatabasePayload, ExportConfigPayload, FileOperationResponse, ImportConfigPayload,
    RestoreDatabasePayload,
};
use crate::AppState;

#[tauri::command]
pub async fn export_app_config(
    payload: ExportConfigPayload,
) -> Result<FileOperationResponse, String> {
    config_transfer_service::export_config(payload)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_app_config(
    state: State<'_, AppState>,
    payload: ImportConfigPayload,
) -> Result<FileOperationResponse, String> {
    config_transfer_service::import_config(state.inner(), payload)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backup_database(
    payload: BackupDatabasePayload,
) -> Result<FileOperationResponse, String> {
    config_transfer_service::backup_database(payload)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_database(
    state: State<'_, AppState>,
    payload: RestoreDatabasePayload,
) -> Result<FileOperationResponse, String> {
    config_transfer_service::restore_database(state.inner(), payload)
        .await
        .map_err(|e| e.to_string())
}
