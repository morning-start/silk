pub mod crypto;
pub mod error;
pub mod gateway;
pub mod models;
pub mod persistence;
pub mod protocol;

use std::path::Path;
use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::Manager;
use tokio::sync::RwLock;

use crate::gateway::spawn_gateway_server;
use crate::gateway::{load_gateway_context, GatewayContext, GatewayServerHandle};

/// 数据库连接池（全局唯一）
static DB_POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();

/// 运行时网关状态
#[derive(Clone)]
pub struct AppState {
    pub gateway: GatewayContext,
    pub gateway_server: Arc<RwLock<Option<GatewayServerHandle>>>,
}

/// 初始化数据库连接池并运行迁移
pub async fn init_database(data_dir: &Path) -> Result<&'static SqlitePool, sqlx::Error> {
    DB_POOL
        .get_or_try_init(|| async move {
            std::fs::create_dir_all(data_dir)?;

            let db_path = data_dir.join("silk.db");
            let db_url = format!("sqlite://{}", db_path.display());

            let pool = SqlitePoolOptions::new()
                .max_connections(5)
                .min_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(&db_url)
                .await?;

            sqlx::query("PRAGMA journal_mode = WAL")
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

// Tauri 入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 日志 channel：容量 1000，背压时丢弃最旧日志
    let (log_sender, log_receiver) = tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法解析应用数据目录");

            if let Err(err) = tauri::async_runtime::block_on(async {
                let pool = init_database(&data_dir).await?;

                // 启动后台日志写入任务
                let log_writer_handle = crate::gateway::spawn_log_writer(pool.clone(), log_receiver);
                app.manage(log_writer_handle);

                // 加载网关上下文（含 log_sender）
                let gateway = load_gateway_context(pool.clone(), log_sender).await?;
                let gateway_server = spawn_gateway_server(gateway.clone()).await?;

                app.manage(AppState {
                    gateway,
                    gateway_server: Arc::new(RwLock::new(Some(gateway_server))),
                });
                Ok::<(), sqlx::Error>(())
            }) {
                panic!("数据库初始化失败: {err}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Phase 4 将在这里注册 IPC 命令
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
