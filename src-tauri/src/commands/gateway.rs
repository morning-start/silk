use tauri::State;

use crate::application::gateway_service::{self, GatewayStartResponse, GatewayStatusResponse, GatewayStopResponse};
use crate::AppState;

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
