use crate::application::models_listing::ModelListingItem;
use crate::application::profile_service as ps;
use crate::application::profile_service::{
    CreateProfilePayload, ProfileResponse, SwitchResult, UpdateProfilePayload,
};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_all_models() -> Result<Vec<ModelListingItem>, String> {
    crate::application::models_listing::list_all_models()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_profiles(
    _state: State<'_, AppState>,
    agent_type: String,
) -> Result<Vec<ProfileResponse>, String> {
    ps::list(agent_type).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_profile(
    _state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfileResponse, String> {
    ps::get(profile_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_profile(
    _state: State<'_, AppState>,
    payload: CreateProfilePayload,
) -> Result<ProfileResponse, String> {
    ps::create(payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_profile(
    _state: State<'_, AppState>,
    profile_id: String,
    payload: UpdateProfilePayload,
) -> Result<ProfileResponse, String> {
    ps::update(profile_id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(
    _state: State<'_, AppState>,
    profile_id: String,
) -> Result<bool, String> {
    ps::delete(profile_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn switch_profile(
    _state: State<'_, AppState>,
    agent_type: String,
    profile_id: String,
) -> Result<SwitchResult, String> {
    ps::switch(agent_type, profile_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_common_snippet(
    _state: State<'_, AppState>,
    agent_type: String,
) -> Result<Option<String>, String> {
    ps::get_common_snippet(agent_type).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_common_snippet(
    _state: State<'_, AppState>,
    agent_type: String,
    content: String,
) -> Result<(), String> {
    ps::set_common_snippet(agent_type, content).await.map_err(|e| e.to_string())
}