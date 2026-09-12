//! 发现类命令：已安装 AI 应用检测、预置 Provider 配置。
//!
//! 薄命令层，仅做透传与错误包装，业务逻辑委托给 `application` 层。

use crate::application::auto_detect::{AiAppDetector, InstalledAiApp};
use crate::application::preset_providers::{PresetProvider, PresetProviderService};

#[tauri::command]
pub async fn detect_installed_ai_apps() -> Result<Vec<InstalledAiApp>, String> {
    Ok(AiAppDetector::detect_all())
}

#[tauri::command]
pub async fn get_preset_providers() -> Result<Vec<PresetProvider>, String> {
    Ok(PresetProviderService::get_all())
}

#[tauri::command]
pub async fn get_preset_provider_by_id(id: String) -> Result<Option<PresetProvider>, String> {
    Ok(PresetProviderService::get_by_id(&id))
}

/// 探测本机 OMP 的模型列表（`omp --list-models`，3s 超时；未安装降级为空）。
#[tauri::command]
pub async fn list_omp_models() -> Result<Vec<serde_json::Value>, String> {
    Ok(crate::application::harness::omp::probe_omp_models())
}
