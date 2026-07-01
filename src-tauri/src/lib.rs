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
use std::path::Path;
use std::sync::Arc;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::Manager;
use tokio::sync::RwLock;

use crate::application::gateway_service::{load_gateway_context, start_existing_gateway};
use crate::gateway::{GatewayContext, GatewayServerHandle};

/// 数据库连接池（全局唯一）
static DB_POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();

/// 运行时网关状态
#[derive(Clone)]
pub struct AppState {
    pub gateway: Arc<RwLock<GatewayContext>>,
    pub gateway_server: Arc<RwLock<Option<GatewayServerHandle>>>,
    /// 渠道 id → name 映射表，用于日志展示时解析渠道名称
    pub provider_name_cache: Arc<RwLock<HashMap<String, String>>>,
    /// 网关设置变更通知通道（用于解耦 settings_service → gateway_service 循环依赖）
    pub settings_change_tx: tokio::sync::broadcast::Sender<()>,
}

impl AppState {
    /// 使指定 Provider 的缓存失效
    pub async fn invalidate_cache(&self, id: &str) {
        self.gateway.read().await.provider_cache.invalidate(id).await;
    }

    /// 从 DB 重载路由规则到网关
    pub async fn reload_routes(&self, pool: &SqlitePool) {
        self.gateway.read().await.route_manager.reload(pool).await.ok();
    }

    /// 重载指定分组的成员到网关
    pub async fn reload_group(&self, pool: &SqlitePool, group_id: &str) {
        self.gateway.read().await.group_manager.reload_group(pool, group_id).await.ok();
    }

    /// 重载所有分组到网关
    pub async fn reload_all_groups(&self, pool: &SqlitePool) {
        self.gateway.read().await.group_manager.reload_all(pool).await.ok();
    }

    /// 刷新渠道名称缓存（从 providers 表重新加载）
    pub async fn refresh_name_cache(&self) {
        if let Some(pool) = crate::get_db_pool() {
            let cache = crate::load_provider_name_cache(pool).await;
            *self.provider_name_cache.write().await = cache;
        }
    }
}

/// 从 providers 表加载 id → name 映射
pub async fn load_provider_name_cache(pool: &SqlitePool) -> HashMap<String, String> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT id, name FROM providers")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let id: String = row.get("id");
            let name: String = row.get("name");
            Some((id, name))
        })
        .collect()
}

/// 初始化数据库连接池并运行迁移
pub async fn init_database(data_dir: &Path) -> Result<&'static SqlitePool, sqlx::Error> {
    let data_dir = data_dir.to_path_buf();
    DB_POOL
        .get_or_try_init(|| async move {
            std::fs::create_dir_all(&data_dir).map_err(sqlx::Error::Io)?;
            let db_path = data_dir.join("silk.db");

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

            // 初始化默认数据（仅首次运行）
            seed_default_data(&pool).await?;

            Ok(pool)
        })
        .await
}

pub fn get_db_pool() -> Option<&'static SqlitePool> {
    DB_POOL.get()
}

/// 初始化默认种子数据（仅首次运行时生效）
async fn seed_default_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // 默认网关设置
    sqlx::query(
        r#"INSERT OR IGNORE INTO gateway_settings (id, bind_host, bind_port, allow_remote, log_retention_days, created_at, updated_at)
        VALUES ('default', '127.0.0.1', 2013, 0, 30, datetime('now'), datetime('now'))"#
    )
    .execute(pool)
    .await?;

    tracing::info!("默认种子数据已就绪");
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
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("无法解析应用数据目录");

            eprintln!("[silk] 应用数据目录: {}", data_dir.display());

            if let Err(err) = tauri::async_runtime::block_on(async {
                let pool = init_database(&data_dir).await?;

                let db_path = data_dir.join("silk.db");
                eprintln!("[silk] 数据库文件: {}", db_path.display());

                // 启动后台日志写入任务
                let log_writer_handle =
                    crate::gateway::spawn_log_writer(pool.clone(), log_receiver);
                app.manage(log_writer_handle);

                // 加载网关上下文（不启动 HTTP 服务，由用户手动启动）
                let gateway = load_gateway_context(pool.clone(), log_sender).await?;

                // 加载渠道名称缓存
                let provider_name_cache =
                    Arc::new(RwLock::new(load_provider_name_cache(pool).await));

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
                    provider_name_cache,
                    settings_change_tx,
                });

                let state = app.state::<AppState>();
                start_existing_gateway(state.inner()).await.map_err(|err| {
                    sqlx::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err))
                })?;
                Ok::<(), sqlx::Error>(())
            }) {
                panic!("数据库初始化失败: {err}");
            }

            // 设置变更监听：配置变更时自动重启网关
            let app_handle = app.handle().clone();
            tokio::spawn(async move {
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
            commands::gateway_status,
            commands::gateway_start,
            commands::gateway_stop,
            commands::gateway_restart,
            // Provider 管理
            commands::list_providers,
            commands::get_provider,
            commands::create_provider,
            commands::update_provider,
            commands::test_provider,
            commands::delete_provider,
            commands::fetch_provider_models,
            // 路由规则管理
            commands::list_routing_rules,
            commands::get_routing_rule,
            commands::create_routing_rule,
            commands::update_routing_rule,
            commands::delete_routing_rule,
            // 日志管理
            commands::list_logs,
            commands::cleanup_logs,
            commands::clear_all_logs,
            commands::export_logs_csv,
            // 设置
            commands::get_gateway_settings,
            commands::update_gateway_settings,
            // 仪表盘统计
            commands::dashboard_stats,
            commands::recent_requests,
            commands::stats_by_provider,
            commands::hourly_stats,
            // 模型映射管理
            commands::list_model_mappings,
            commands::get_model_mapping,
            commands::find_model_mapping_by_name,
            commands::create_model_mapping,
            commands::update_model_mapping,
            commands::delete_model_mapping,
            commands::get_group_providers,
            // 网关 Key 管理
            commands::list_gateway_keys,
            commands::get_gateway_key,
            commands::create_gateway_key,
            commands::update_gateway_key,
            commands::delete_gateway_key,
            // 分组管理
            commands::list_groups,
            commands::find_groups_by_model,
            commands::get_group,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            commands::add_group_member,
            commands::update_group_member,
            commands::remove_group_member,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
