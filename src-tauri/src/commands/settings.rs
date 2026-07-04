use tauri::{AppHandle, State};

use crate::application::settings_service::{self, GatewaySettingsResponse, UpdateSettingsPayload};
use crate::AppState;

#[tauri::command]
pub async fn get_gateway_settings() -> Result<GatewaySettingsResponse, String> {
    settings_service::get().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_gateway_settings(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, String> {
    settings_service::update(&app_handle, state.inner(), payload)
        .await
        .map_err(|e| e.to_string())
}
