use tauri::AppHandle;

use crate::application::settings_service;
use crate::application::update_service::{self, UpdateInfo};

/// 检查软件更新
///
/// 走 GitHub Releases REST API，返回最新版本、发布说明与当前平台的安装包地址；
/// 仓库配置了全局代理时一并使用（国内直连 api.github.com 通常不可用）。
#[tauri::command]
pub async fn check_app_update(app_handle: AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app_handle.package_info().version.to_string();
    // 代理是可选配置，读取失败不应阻断更新检查
    let proxy_url = settings_service::get().ok().and_then(|s| s.proxy_url);

    update_service::check(&current_version, proxy_url.as_deref())
        .await
        .map_err(|e| e.to_string())
}
