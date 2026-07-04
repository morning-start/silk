use tauri::State;

use crate::application::provider_service::{
    self, CreateProviderPayload, ProviderModelInfo, ProviderResponse,
    ProviderTestResponse, UpdateProviderPayload,
};
use crate::AppState;

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
    payload: crate::application::provider_service::FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, String> {
    provider_service::fetch_models(payload).await.map_err(|e| e.to_string())
}
