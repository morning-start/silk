pub mod crypto;
pub mod error;
pub mod gateway;
pub mod models;
pub mod persistence;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::Manager;
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::gateway::{load_gateway_context, spawn_gateway_server, GatewayContext, GatewayServerHandle};

/// 数据库连接池（全局唯一）
static DB_POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();

/// 运行时网关状态
#[derive(Clone)]
pub struct AppState {
    pub gateway: GatewayContext,
    pub gateway_server: Arc<RwLock<Option<GatewayServerHandle>>>,
}

/// 初始化数据库连接池并运行迁移
pub async fn init_database(
    data_dir: &std::path::Path,
) -> Result<&'static SqlitePool, sqlx::Error> {
    DB_POOL
        .get_or_try_init(|| async move {
            // 确保数据目录存在
            std::fs::create_dir_all(data_dir)?;

            let db_path = data_dir.join("silk.db");
            let db_url = format!("sqlite://{}", db_path.display());

            let pool = SqlitePoolOptions::new()
                .max_connections(5)
                .min_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(&db_url)
                .await?;

            // 启用 WAL 模式（提升并发读写性能）
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&pool)
                .await?;

            // 运行迁移
            sqlx::migrate!("./migrations").run(&pool).await?;

            Ok(pool)
        })
        .await
}

/// 获取全局数据库连接池（需先调用 init_database）
pub fn get_db_pool() -> Option<&'static SqlitePool> {
    DB_POOL.get()
}

// Tauri 入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法解析应用数据目录");

            if let Err(err) = tauri::async_runtime::block_on(async {
                let pool = init_database(&data_dir).await?;
                let gateway = load_gateway_context(pool).await?;
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
