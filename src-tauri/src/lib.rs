pub mod application;
pub mod commands;
pub mod crypto;
pub mod error;
pub mod gateway;
pub mod load_balancer;
pub mod models;
pub mod persistence;
pub mod protocol;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::menu::MenuItem;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tokio::sync::RwLock;

use crate::application::gateway_service::{load_gateway_context, start_existing_gateway};
use crate::gateway::{GatewayContext, GatewayServerHandle};

/// 数据库连接池（全局唯一）
static DB_POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();
static DB_PATH: tokio::sync::OnceCell<PathBuf> = tokio::sync::OnceCell::const_new();

/// 网关设置文件路径（全局唯一）
static SETTINGS_PATH: tokio::sync::OnceCell<PathBuf> = tokio::sync::OnceCell::const_new();

/// 存放所有非敏感、小型、读频繁的字典表数据。
/// 启动时一次性加载，写时按分区失效刷新。
#[derive(Debug, Clone, Default)]
pub struct LookupCache {
    /// provider id → name
    pub provider_names: HashMap<String, String>,
    /// model mapping id → model_name
    pub model_mapping_names: HashMap<String, String>,
    /// gateway key id → name
    pub gateway_key_names: HashMap<String, String>,
}

/// 运行时网关状态
#[derive(Clone)]
pub struct AppState {
    pub gateway: Arc<RwLock<GatewayContext>>,
    pub gateway_server: Arc<RwLock<Option<GatewayServerHandle>>>,
    /// 通用字典表缓存，启动时一次性加载，写操作后刷新
    pub lookup_cache: Arc<RwLock<LookupCache>>,
    /// 网关设置变更通知通道（用于解耦 settings_service → gateway_service 循环依赖）
    pub settings_change_tx: tokio::sync::broadcast::Sender<()>,
}

impl AppState {
    /// 使指定 Provider 的缓存失效
    pub async fn invalidate_cache(&self, id: &str) {
        self.gateway.read().await.provider_cache.invalidate(id).await;
    }

    pub async fn refresh_lookup(&self) {
        if let Some(pool) = crate::get_db_pool() {
            let cache = crate::load_lookup_cache(pool).await;
            *self.lookup_cache.write().await = cache;
        }
    }
}

/// 从 DB 一次性加载所有字典表到 LookupCache
pub async fn load_lookup_cache(pool: &SqlitePool) -> LookupCache {
    use sqlx::Row;

    let provider_names = sqlx::query("SELECT id, name FROM providers")
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            Some((r.get::<String, _>("id"), r.get::<String, _>("name")))
        })
        .collect();

    let model_mapping_names = sqlx::query("SELECT id, model_name FROM model_mappings")
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            Some((r.get::<String, _>("id"), r.get::<String, _>("model_name")))
        })
        .collect();

    let gateway_key_names = sqlx::query("SELECT id, name FROM gateway_keys")
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            Some((r.get::<String, _>("id"), r.get::<String, _>("name")))
        })
        .collect();

    LookupCache {
        provider_names,
        model_mapping_names,
        gateway_key_names,
    }
}

/// 初始化数据库连接池并运行迁移
pub async fn init_database(data_dir: &Path) -> Result<&'static SqlitePool, sqlx::Error> {
    let data_dir = data_dir.to_path_buf();
    DB_POOL
        .get_or_try_init(|| async move {
            std::fs::create_dir_all(&data_dir).map_err(sqlx::Error::Io)?;
            let db_path = data_dir.join("silk.db");
            let _ = DB_PATH.set(db_path.clone());

            eprintln!("[silk] 数据库路径: {}", db_path.display());

            let conn_opts = SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(5)
                .min_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect_with(conn_opts)
                .await?;

            // SQLite 运行时 PRAGMA 优化
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&pool)
                .await
                .map_err(|e| {
                    eprintln!("[silk] 启用 WAL 模式失败: {e}");
                    e
                })?;
            sqlx::query("PRAGMA synchronous = NORMAL")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA temp_store = MEMORY")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA cache_size = -8000")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA busy_timeout = 5000")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await?;

            sqlx::migrate!("./migrations").run(&pool).await?;

            Ok(pool)
        })
        .await
}

pub fn get_db_pool() -> Option<&'static SqlitePool> {
    DB_POOL.get()
}

pub fn get_db_path() -> Option<&'static Path> {
    DB_PATH.get().map(|p| p.as_path())
}

/// 获取网关设置文件路径
pub fn get_settings_path() -> Option<&'static Path> {
    SETTINGS_PATH.get().map(|p| p.as_path())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_main_window<R: Runtime>(window: &tauri::Window<R>) {
    let _ = window.hide();
}

fn handle_tray_action<R: Runtime>(app: &AppHandle<R>, action: &str) {
    match action {
        "show" => show_main_window(app),
        "start_gateway" => {
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::application::gateway_service::start(&state).await;
            });
        }
        "stop_gateway" => {
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::application::gateway_service::stop(&state).await;
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start_gateway", "启动网关", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop_gateway", "停止网关", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 Silk", true, None::<&str>)?;
    let menu = tauri::menu::Menu::with_items(app, &[&show_item, &start_item, &stop_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("Silk")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_action(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => show_main_window(&tray.app_handle()),
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(&tray.app_handle()),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _ = builder.build(app)?;
    Ok(())
}

/// 初始化网关设置文件（首次运行时创建默认配置）
pub(crate) async fn init_gateway_settings(data_dir: &Path) -> Result<(), String> {
    let settings_path = data_dir.join("gateway.json");
    SETTINGS_PATH.set(settings_path.clone()).map_err(|_| "网关设置路径已初始化".to_string())?;
    let _ = crate::models::GatewaySettings::load(&settings_path)?;
    tracing::info!("网关设置文件已就绪: {}", settings_path.display());
    Ok(())
}

// Tauri 入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化 tracing 日志（输出到终端，开发时通过 `cargo tauri dev` 查看）
    tracing_subscriber::fmt::init();

    // 日志 channel：容量 1000，背压时丢弃最旧日志
    let (log_sender, log_receiver) =
        tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let Some(path) = crate::get_settings_path() {
                    let settings = crate::persistence::GatewaySettingsRepo::load_effective(path);
                    if settings.close_to_tray {
                        api.prevent_close();
                        hide_main_window(window);
                    }
                }
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("无法解析应用数据目录");

            eprintln!("[silk] 应用数据目录: {}", data_dir.display());

            if let Err(err) = tauri::async_runtime::block_on(async {
                // 初始化数据库
                let pool = init_database(&data_dir).await?;

                let db_path = data_dir.join("silk.db");
                eprintln!("[silk] 数据库文件: {}", db_path.display());

                // 初始化网关设置文件
                init_gateway_settings(&data_dir).await
                    .map_err(|e| sqlx::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

                // 启动后台日志写入任务
                let log_writer_handle =
                    crate::gateway::spawn_log_writer(pool.clone(), log_receiver);
                app.manage(log_writer_handle);

                // 加载网关上下文（不启动 HTTP 服务，由用户手动启动）
                let gateway = load_gateway_context(pool.clone(), log_sender).await?;

                // 加载通用字典表缓存
                let lookup_cache =
                    Arc::new(RwLock::new(load_lookup_cache(pool).await));

                // 启动后台日志清理任务
                let cleanup_handle = crate::gateway::log_cleanup::spawn_log_cleanup_task(
                    pool.clone(),
                    gateway.settings.clone(),
                );
                app.manage(cleanup_handle);

                // 创建设置变更广播通道（容量 16，避免背压阻塞）
                let (settings_change_tx, _settings_change_rx) =
                    tokio::sync::broadcast::channel::<()>(16);

                app.manage(AppState {
                    gateway: Arc::new(RwLock::new(gateway)),
                    gateway_server: Arc::new(RwLock::new(None)),
                    lookup_cache,
                    settings_change_tx,
                });

                let state = app.state::<AppState>();
                let should_auto_start = {
                    let gateway_guard = state.gateway.read().await;
                    let settings_guard = gateway_guard.settings.read().await;
                    let _ = crate::application::settings_service::sync_autostart(
                        &app.handle().clone(),
                        settings_guard.launch_at_startup,
                    );
                    settings_guard.auto_start_gateway
                };
                if should_auto_start {
                    start_existing_gateway(state.inner()).await.map_err(|err| {
                        sqlx::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err))
                    })?;
                }
                Ok::<(), sqlx::Error>(())
            }) {
                panic!("数据库初始化失败: {err}");
            }

            if let Err(err) = setup_tray(&app.handle()) {
                panic!("托盘初始化失败: {err}");
            }

            // 设置变更监听：配置变更时自动重启网关
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = {
                    let state = app_handle.state::<AppState>();
                    state.settings_change_tx.subscribe()
                };
                loop {
                    match rx.recv().await {
                        Ok(()) => {
                            let state = app_handle.state::<AppState>();
                            if state.gateway_server.read().await.is_some() {
                                tracing::info!("设置变更，自动重启网关");
                                let _ = crate::application::gateway_service::restart(
                                    state.inner(),
                                )
                                .await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            continue; // 丢弃，继续监听
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break; // channel 关闭，退出
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Gateway 控制
            commands::gateway::gateway_status,
            commands::gateway::gateway_start,
            commands::gateway::gateway_stop,
            commands::gateway::gateway_restart,
            // Provider 管理
            commands::providers::list_providers,
            commands::providers::get_provider,
            commands::providers::create_provider,
            commands::providers::update_provider,
            commands::providers::test_provider,
            commands::providers::delete_provider,
            commands::providers::fetch_provider_models,
            // 日志管理
            commands::logs::list_logs,
            commands::logs::cleanup_logs,
            commands::logs::clear_all_logs,
            commands::logs::export_logs_csv,
            // 设置
            commands::settings::get_gateway_settings,
            commands::settings::update_gateway_settings,
            commands::config_transfer::export_app_config,
            commands::config_transfer::import_app_config,
            commands::config_transfer::backup_database,
            commands::config_transfer::restore_database,
            // 仪表盘统计
            commands::stats::dashboard_stats,
            commands::stats::recent_requests,
            commands::stats::stats_by_provider,
            commands::stats::hourly_stats,
            // 模型映射管理
            commands::model_mappings::list_model_mappings,
            commands::model_mappings::get_model_mapping,
            commands::model_mappings::find_model_mapping_by_name,
            commands::model_mappings::create_model_mapping,
            commands::model_mappings::update_model_mapping,
            commands::model_mappings::delete_model_mapping,
            // 网关 Key 管理
            commands::gateway_keys::list_gateway_keys,
            commands::gateway_keys::get_gateway_key,
            commands::gateway_keys::create_gateway_key,
            commands::gateway_keys::update_gateway_key,
            commands::gateway_keys::delete_gateway_key,
            // Profile 管理
            commands::profiles::list_profiles,
            commands::profiles::get_profile,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile,
            commands::profiles::get_common_snippet,
            commands::profiles::set_common_snippet,
            commands::profiles::list_all_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
