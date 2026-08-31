use tauri::{AppHandle, State};

use crate::application::change_events::emit_data_changed;
use crate::application::model_mapping_service as mms;
use crate::application::model_mapping_service::{
    CreateModelMappingPayload, ModelMappingResponse, UpdateModelMappingPayload,
};
use crate::application::models_listing::ModelListingItem;
use crate::AppState;

/// 全量模型列表（模型池 + 渠道模型，供前端下拉使用）
#[tauri::command]
pub async fn list_all_models() -> Result<Vec<ModelListingItem>, String> {
    crate::application::models_listing::list_all_models()
        .await
        .map_err(|e| e.to_string())
}

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
    app_handle: AppHandle,
    _state: State<'_, AppState>,
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let result = mms::create(payload).await.map_err(|e| e.to_string())?;
    emit_data_changed(&app_handle, "groups");
    Ok(result)
}

#[tauri::command]
pub async fn update_model_mapping(
    app_handle: AppHandle,
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let result = mms::update(id, payload).await.map_err(|e| e.to_string())?;
    emit_data_changed(&app_handle, "groups");
    Ok(result)
}

#[tauri::command]
pub async fn delete_model_mapping(app_handle: AppHandle, _state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let result = mms::delete(id).await.map_err(|e| e.to_string())?;
    emit_data_changed(&app_handle, "groups");
    Ok(result)
}
