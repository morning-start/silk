use tauri::AppHandle;

use crate::application::kernel_service::{self, KernelInfo, KernelInstallResult, KernelStatus};
use crate::application::settings_service;

/// 读取网关设置里的全局代理（可选配置，读取失败不应阻断内核更新）
fn proxy_url() -> Option<String> {
    settings_service::get().ok().and_then(|s| s.proxy_url)
}

/// 当前内核状态：版本、ABI、来源路径与是否可更新
#[tauri::command]
pub async fn get_kernel_status() -> Result<KernelStatus, String> {
    Ok(kernel_service::status())
}

/// 检查协议内核（prism.wasm）更新
///
/// 与 `check_app_update` 一样走 GitHub Releases REST API 并沿用全局代理，
/// 但比对的是内核版本（来自本地构建清单）而非应用版本。
#[tauri::command]
pub async fn check_kernel_update() -> Result<KernelInfo, String> {
    kernel_service::check(proxy_url().as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// 下载并安装最新协议内核
///
/// 下载后先做 sha256 校验，再用 wasmtime 实例化探测 ABI；两道关都通过才原子替换
/// 应用数据目录下的 prism.wasm。内核是进程级单例，安装后需重启应用才生效。
#[tauri::command]
pub async fn install_kernel_update() -> Result<KernelInstallResult, String> {
    kernel_service::install(proxy_url().as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// 回滚到上一次替换前的内核备份
///
/// 内核更新后若应用起不来或转换异常，这是唯一的自救出口 —— 不必重装应用。
/// 回滚前会用 wasmtime 探测备份内核，坏的备份会被拒绝（换上去比不换更糟）。
#[tauri::command]
pub async fn rollback_kernel_update() -> Result<KernelInstallResult, String> {
    kernel_service::rollback_to_backup().map_err(|e| e.to_string())
}

/// 重启应用（内核更新后生效用）
///
/// 直接使用 Tauri 核心的 `AppHandle::restart()`，无需额外插件或权限。
#[tauri::command]
pub async fn restart_app(app_handle: AppHandle) -> Result<(), String> {
    app_handle.restart();
}
