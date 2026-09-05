use crate::application::preset_service::{PresetDefaults, PresetService, SwitchResult};
use crate::models::{AgentType, NewPreset, Preset, UpdatePreset};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_presets(
    _state: State<'_, AppState>,
    agent_type: String,
) -> Result<Vec<Preset>, String> {
    PresetService::list(agent_type).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_preset(
    _state: State<'_, AppState>,
    preset_id: String,
) -> Result<Preset, String> {
    PresetService::get(preset_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_preset(
    _state: State<'_, AppState>,
    payload: NewPreset,
) -> Result<Preset, String> {
    PresetService::create(payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_preset(
    _state: State<'_, AppState>,
    preset_id: String,
    payload: UpdatePreset,
) -> Result<Preset, String> {
    PresetService::update(preset_id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_preset(
    _state: State<'_, AppState>,
    preset_id: String,
) -> Result<bool, String> {
    PresetService::delete(preset_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn switch_preset(
    _state: State<'_, AppState>,
    agent_type: String,
    preset_id: String,
) -> Result<SwitchResult, String> {
    PresetService::switch(agent_type, preset_id).await.map_err(|e| e.to_string())
}

/// Agent 类型列表（前端 Tab 用）
#[tauri::command]
pub async fn list_agent_types() -> Vec<AgentType> {
    AgentType::all_typed()
}

/// 新建预设默认值（silk 网关 base_url/api_key，按 harness 映射表单字段）
#[tauri::command]
pub async fn get_preset_defaults(
    _state: State<'_, AppState>,
    agent_type: String,
) -> Result<PresetDefaults, String> {
    PresetService::get_defaults(agent_type).await.map_err(|e| e.to_string())
}

/// 预设重排序（拖拽后按序落 sort_index，对齐 cc-switch update_sort_order）
#[tauri::command]
pub async fn update_preset_order(
    _state: State<'_, AppState>,
    agent_type: String,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    PresetService::reorder(agent_type, ordered_ids).await.map_err(|e| e.to_string())
}
