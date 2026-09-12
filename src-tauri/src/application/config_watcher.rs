//! 外部配置文件变更监听（外部监督）
//!
//! 监听应用数据目录下 `gateway.json` 与 `model-catalog.json` 的外部修改：
//! - `gateway.json` 外部编辑 → 重载设置并在网关运行中自动重启（复用 settings_change_tx 广播）
//! - `model-catalog.json` 外部编辑 → 重载模型目录缓存，模型池响应即时反映
//!
//! 应用自身写入（原子写会记录 self-write 标记）不会触发循环重启。

use std::path::PathBuf;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use crate::AppState;

/// 去抖窗口：notify 对一次编辑可能产生多个事件，统一合并
const DEBOUNCE: Duration = Duration::from_millis(300);

/// 启动配置监听任务（在 lib.rs setup 中调用，后台运行不阻塞启动）
pub fn spawn_config_watcher(app_handle: AppHandle, data_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_watcher(app_handle, data_dir).await {
            tracing::warn!(error = %e, "配置文件监听已停止");
        }
    });
}

async fn run_watcher(app_handle: AppHandle, data_dir: PathBuf) -> Result<(), String> {
    // 事件经 mpsc 从 notify 回调线程桥接到异步任务
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(64);

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                let _ = tx.blocking_send(event);
            }
            Err(e) => tracing::debug!(error = %e, "notify 事件错误"),
        })
        .map_err(|e| format!("创建 watcher 失败: {e}"))?;

    watcher
        .watch(&data_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("监听目录失败: {e}"))?;
    tracing::info!("配置文件监听已启动: {}", data_dir.display());

    while let Some(event) = rx.recv().await {
        handle_event(&app_handle, event).await;
    }
    Ok(())
}

/// 处理单个 notify 事件（带 300ms 去抖，合并同文件连续事件）
async fn handle_event(app_handle: &AppHandle, event: Event) {
    // 只关心 Modify / Create / Rename 事件
    let relevant = matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Any
    );
    if !relevant {
        return;
    }

    let names: Vec<String> = event
        .paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    if names.is_empty() {
        return;
    }

    // 去抖：等待窗口结束，期间丢弃同一文件的后续事件
    tokio::time::sleep(DEBOUNCE).await;

    for name in names {
        match name.as_str() {
            "gateway.json" => handle_gateway_change(app_handle).await,
            "model-catalog.json" => handle_catalog_change(app_handle, &event.paths).await,
            _ => {}
        }
    }
}

/// gateway.json 外部变更：重载设置；运行中的网关自动重启
async fn handle_gateway_change(app_handle: &AppHandle) {
    let Some(path) = crate::get_settings_path() else {
        return;
    };

    // 读取新内容
    let Ok(content) = std::fs::read_to_string(path) else {
        tracing::warn!("读取 gateway.json 失败，忽略外部变更");
        return;
    };

    // 应用自身写入（原子写已记录 self-write 标记）→ 跳过，避免循环
    if crate::application::config_writer::is_self_write(path, &content) {
        return;
    }

    // 解析新设置
    let Ok(new_settings) = serde_json::from_str::<crate::models::GatewaySettings>(&content) else {
        tracing::warn!("gateway.json 外部变更解析失败（可能编辑到一半），忽略");
        return;
    };

    let state = app_handle.state::<AppState>();
    let runtime_changed = {
        let gateway_guard = state.gateway.read().await;
        let mut current = gateway_guard.settings.write().await;
        let changed = current.bind_host != new_settings.bind_host
            || current.bind_port != new_settings.bind_port;
        *current = new_settings.clone();

        // 热更新限流配置
        gateway_guard
            .rate_limit_state
            .update_config(
                new_settings.rate_limit_enabled,
                new_settings.rate_limit_max_requests_per_minute as u64,
                new_settings.rate_limit_max_tokens_per_minute as u64,
            )
            .await;
        changed
    };

    tracing::info!("检测到 gateway.json 外部变更，已重载设置");
    if runtime_changed {
        let _ = state.settings_change_tx.send(());
    }
}

/// model-catalog.json 外部变更：重载模型目录缓存
async fn handle_catalog_change(app_handle: &AppHandle, paths: &[PathBuf]) {
    let data_dir = paths
        .iter()
        .find(|p| p.file_name().map(|n| n == "model-catalog.json").unwrap_or(false))
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let catalog_path = data_dir.map(|d| d.join("model-catalog.json")).unwrap_or_else(|| {
        app_handle
            .path()
            .app_data_dir()
            .map(|d| d.join("model-catalog.json"))
            .unwrap_or_else(|_| PathBuf::from("model-catalog.json"))
    });

    crate::application::model_catalog::reload(&catalog_path);
}
