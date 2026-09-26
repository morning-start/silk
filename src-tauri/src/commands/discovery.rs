//! 发现类命令：已安装 AI 应用检测、渠道模板目录。
//!
//! 薄命令层，仅做透传与错误包装，业务逻辑委托给 `application` 层。

use crate::application::auto_detect::{AiAppDetector, InstalledAiApp};
use crate::application::channel_templates::{ChannelTemplate, ChannelTemplateService};

#[tauri::command]
pub async fn detect_installed_ai_apps() -> Result<Vec<InstalledAiApp>, String> {
    Ok(AiAppDetector::detect_all())
}

#[tauri::command]
pub async fn get_channel_templates() -> Result<Vec<ChannelTemplate>, String> {
    Ok(ChannelTemplateService::get_all())
}

#[tauri::command]
pub async fn get_channel_template_by_id(id: String) -> Result<Option<ChannelTemplate>, String> {
    Ok(ChannelTemplateService::get_by_id(&id))
}
