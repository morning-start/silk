use tauri::State;

use crate::application::provider_service::{
    self, CreateProviderPayload, FetchModelsPayload, ProviderModelInfo, ProviderResponse,
    ProviderTestResponse, UpdateProviderPayload,
};
use crate::AppState;

#[tauri::command]
pub async fn list_providers(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderResponse>, String> {
    provider_service::list(state).await
}

#[tauri::command]
pub async fn get_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProviderResponse, String> {
    provider_service::get(state, id).await
}

#[tauri::command]
pub async fn create_provider(
    state: State<'_, AppState>,
    payload: CreateProviderPayload,
) -> Result<ProviderResponse, String> {
    provider_service::create(state, payload).await
}

#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateProviderPayload,
) -> Result<ProviderResponse, String> {
    provider_service::update(state, id, payload).await
}

#[tauri::command]
pub async fn test_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProviderTestResponse, String> {
    provider_service::test(state, id).await
}

#[tauri::command]
pub async fn delete_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    provider_service::delete(state, id).await
}

#[tauri::command]
pub async fn fetch_provider_models(
    state: State<'_, AppState>,
    payload: FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, String> {
    provider_service::fetch_models(state, payload).await
}
