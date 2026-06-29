use tauri::State;

use crate::application::settings_service::{self, GatewaySettingsResponse, UpdateSettingsPayload};
use crate::AppState;

#[tauri::command]
pub async fn get_gateway_settings() -> Result<GatewaySettingsResponse, String> {
    settings_service::get().await
}

#[tauri::command]
pub async fn update_gateway_settings(
    state: State<'_, AppState>,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, String> {
    settings_service::update(state.inner(), payload).await
}