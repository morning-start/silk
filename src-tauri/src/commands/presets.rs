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

/// OpenCode 累加模式独立启停：active=true 激活（加入 opencode.json 的 provider 段，
/// 不清除其他已激活 preset）；active=false 取消激活（移出配置）。其余应用请用 switch_preset。
#[tauri::command]
pub async fn set_preset_active(
    _state: State<'_, AppState>,
    agent_type: String,
    preset_id: String,
    active: bool,
) -> Result<SwitchResult, String> {
    PresetService::set_active(agent_type, preset_id, active).await.map_err(|e| e.to_string())
}

/// 官方直连行一键恢复默认（category=official）：settings 重置回官方锚定形态；
/// 激活中则同步剥离 live 管理键。
#[tauri::command]
pub async fn reset_official_preset(
    _state: State<'_, AppState>,
    preset_id: String,
) -> Result<Preset, String> {
    PresetService::reset_official_default(preset_id).await.map_err(|e| e.to_string())
}

/// 协议能力项：name 为 silk 规范协议名（openai/responses/messages/gemini/bedrock 等），
/// convert = 是否可由 silk 网关 prism 转换（否则仅直连第三方端点）
#[derive(serde::Serialize)]
pub struct AgentProtocolInfo {
    pub name: &'static str,
    pub convert: bool,
}

/// Agent 类型信息（前端 Tab + 协议能力提示用）。协议能力单一事实来源在
/// AgentType::HARNESS_NATIVE_PROTOCOLS（Rust），前端不自建副本。
#[derive(serde::Serialize)]
pub struct AgentTypeInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub protocols: Vec<AgentProtocolInfo>,
}

/// Agent 类型列表（前端 Tab 用），附带各 harness 原生协议能力：
/// 单协议 harness（claude_code/codex/gemini_cli）协议由 CLI 固定 → 表单无协议字段；
/// 多协议 harness（opencode/hermes）→ 表单保留 npm/api_mode 选择器。
#[tauri::command]
pub async fn list_agent_types() -> Vec<AgentTypeInfo> {
    AgentType::all()
        .iter()
        .map(|(id, name)| AgentTypeInfo {
            id,
            name,
            protocols: AgentType::native_protocols(id)
                .iter()
                .map(|protocol| AgentProtocolInfo {
                    name: protocol,
                    convert: AgentType::is_gateway_convertible(protocol),
                })
                .collect(),
        })
        .collect()
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
