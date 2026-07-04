use tauri::State;

use crate::application::model_mapping_service as mms;
use crate::application::model_mapping_service::{
    CreateModelMappingPayload, ModelMappingResponse, UpdateModelMappingPayload,
};
use crate::AppState;

#[tauri::command]
pub async fn list_model_mappings(
    _state: State<'_, AppState>,
) -> Result<Vec<ModelMappingResponse>, String> {
    mms::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_model_mapping(
    _state: State<'_, AppState>,
    id: String,
) -> Result<ModelMappingResponse, String> {
    mms::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_model_mapping_by_name(
    _state: State<'_, AppState>,
    model_name: String,
) -> Result<Option<ModelMappingResponse>, String> {
    mms::find_by_name(model_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_model_mapping(
    _state: State<'_, AppState>,
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    mms::create(payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_model_mapping(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    mms::update(id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model_mapping(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    mms::delete(id).await.map_err(|e| e.to_string())
}
