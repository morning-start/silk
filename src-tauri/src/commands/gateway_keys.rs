use tauri::State;

use crate::application::gateway_key_service as gks;
use crate::application::gateway_key_service::{
    CreateGatewayKeyPayload, CreateGatewayKeyResponse, GatewayKeyResponse,
    UpdateGatewayKeyPayload,
};
use crate::AppState;

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
