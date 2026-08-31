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

/// 用户家目录（可覆盖，用于测试）
static HOME_DIR: tokio::sync::OnceCell<PathBuf> = tokio::sync::OnceCell::const_new();

/// 存放所有非敏感、小型、读频繁的字典表数据。
/// 启动时一次性加载，写时按分区失效刷新。
#[derive(Debug, Clone, Default)]
pub struct LookupCache {
    /// provider id → name
    pub provider_names: HashMap<String, String>,
    /// model mapping id → model_name
    pub model_mapping_names: HashMap<String, String>,
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
    crate::persistence::LookupCacheRepo::load(pool).await
}

/// 初始化数据库连接池并运行迁移
pub async fn init_database(data_dir: &Path) -> Result<&'static SqlitePool, sqlx::Error> {
    let data_dir = data_dir.to_path_buf();
    DB_POOL
        .get_or_try_init(|| async move {
            std::fs::create_dir_all(&data_dir).map_err(sqlx::Error::Io)?;
            let db_path = data_dir.join("silk.db");
            let _ = DB_PATH.set(db_path.clone());

            tracing::info!(db_path = %db_path.display(), "数据库路径");

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
                    tracing::warn!("启用 WAL 模式失败: {e}");
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

            // 确保 API Key 文件存在
            let _ = crate::application::api_key_service::ensure_api_key(&data_dir);

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

/// 获取用户家目录
pub fn get_home_dir() -> &'static Path {
    HOME_DIR.get().map(|p| p.as_path()).unwrap_or_else(|| {
        // 未初始化时从环境变量获取
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        // 尝试初始化，失败则用已存在的值
        let _ = HOME_DIR.set(home);
        HOME_DIR.get().map(|p| p.as_path()).unwrap_or(Path::new("."))
    })
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
            } => show_main_window(tray.app_handle()),
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
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
    // tracing 在 setup 中初始化（需要 data_dir 路径写日志文件），
    // setup 之前的输出走 stderr。

    // 日志 channel：容量 1000，背压时丢弃最旧日志
    let (log_sender, log_receiver) =
        tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
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
            let settings_path = data_dir.join("gateway.json");
            let settings = crate::models::GatewaySettings::load(&settings_path)
                .unwrap_or_default();

            // 必须早于任何日志输出：setup 之前的日志只能走 stderr
            init_logging_system(&data_dir, &settings);
            tracing::info!(data_dir = %data_dir.display(), "应用数据目录");

            if let Err(err) = tauri::async_runtime::block_on(bootstrap(
                app,
                &data_dir,
                log_sender,
                log_receiver,
            )) {
                panic!("数据库初始化失败: {err}");
            }

            if let Err(err) = setup_tray(app.handle()) {
                panic!("托盘初始化失败: {err}");
            }

            spawn_settings_watchdog(app.handle().clone());

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
            commands::gateway_keys::get_builtin_gateway_key,
            commands::gateway_keys::reset_builtin_gateway_key,
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
            commands::profiles::import_live_config,
            commands::profiles::get_agent_live_status,
            // 自动检测
            commands::discovery::detect_installed_ai_apps,
            // 预置配置
            commands::discovery::get_preset_providers,
            commands::discovery::get_preset_provider_by_id,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 初始化分层级日志系统
///
/// 日志 guard 必须存活到进程结束，因此泄漏到全局。
fn init_logging_system(data_dir: &Path, settings: &crate::models::GatewaySettings) {
    let log_dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_config = crate::gateway::logging::LoggingConfig::from_settings(settings, log_dir);
    let guard = crate::gateway::logging::init_logging(log_config);
    Box::leak(Box::new(guard));
}

/// 装配应用运行时：数据库、后台任务、AppState，必要时拉起网关
///
/// 网关 HTTP 服务默认不启动（由用户手动启动），仅当设置开启自动启动时才拉起。
async fn bootstrap(
    app: &mut tauri::App,
    data_dir: &Path,
    log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    log_receiver: tokio::sync::mpsc::Receiver<crate::models::NewRequestLog>,
) -> Result<(), sqlx::Error> {
    let pool = init_database(data_dir).await?;
    tracing::info!(db_path = %data_dir.join("silk.db").display(), "数据库文件");

    init_gateway_settings(data_dir)
        .await
        .map_err(|e| sqlx::Error::Io(std::io::Error::other(e)))?;

    init_home_dir(data_dir);

    // 启动后台日志写入任务
    let log_writer_handle = crate::gateway::spawn_log_writer(pool.clone(), log_receiver);
    app.manage(log_writer_handle);

    // 加载网关上下文（不启动 HTTP 服务，由用户手动启动）
    let gateway = load_gateway_context(pool.clone(), log_sender).await?;
    init_trace_manager(&gateway).await;

    // 加载通用字典表缓存
    let lookup_cache = Arc::new(RwLock::new(load_lookup_cache(pool).await));

    // 启动后台日志清理任务
    let cleanup_handle = crate::gateway::log_cleanup::spawn_log_cleanup_task(
        pool.clone(),
        gateway.settings.clone(),
    );
    app.manage(cleanup_handle);

    // 设置变更广播通道（容量 16，避免背压阻塞）
    let (settings_change_tx, _settings_change_rx) = tokio::sync::broadcast::channel::<()>(16);
    app.manage(AppState {
        gateway: Arc::new(RwLock::new(gateway)),
        gateway_server: Arc::new(RwLock::new(None)),
        lookup_cache,
        settings_change_tx,
    });

    auto_start_gateway_if_enabled(app).await?;

    Ok(())
}

/// 初始化用户家目录（AI 工具配置文件写入位置），失败时回退到应用数据目录
fn init_home_dir(data_dir: &Path) {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| data_dir.to_path_buf());
    let _ = HOME_DIR.set(home);
}

/// 初始化追踪管理器，失败仅告警不阻断启动
async fn init_trace_manager(gateway: &GatewayContext) {
    let trace_enabled = gateway.settings.read().await.trace_enabled;
    if let Err(e) = crate::gateway::trace_manager::init(trace_enabled) {
        tracing::warn!(error = %e, "追踪管理器初始化失败");
    }
}

/// 同步开机自启设置，并在开启了「启动时自动运行网关」时拉起网关
async fn auto_start_gateway_if_enabled(app: &mut tauri::App) -> Result<(), sqlx::Error> {
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
        start_existing_gateway(state.inner())
            .await
            .map_err(|err| sqlx::Error::Io(std::io::Error::other(err)))?;
    }

    Ok(())
}

/// 监听设置变更，网关运行期间自动重启以应用新配置
fn spawn_settings_watchdog(app_handle: AppHandle) {
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
                        let _ = crate::application::gateway_service::restart(state.inner()).await;
                    }
                }
                // 广播积压：丢弃跳过的通知，继续监听
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                // channel 关闭：退出监听循环
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
