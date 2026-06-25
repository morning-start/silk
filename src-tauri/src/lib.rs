pub mod crypto;
pub mod error;
pub mod models;
pub mod persistence;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// 数据库连接池（全局唯一）
static DB_POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();

/// 初始化数据库连接池并运行迁移
pub async fn init_database(data_dir: &std::path::Path) -> Result<SqlitePool, sqlx::Error> {
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
}

/// 获取全局数据库连接池（需先调用 init_database）
pub fn get_db_pool() -> Option<&'static SqlitePool> {
    DB_POOL.get()
}

/// 设置全局数据库连接池
pub async fn set_db_pool(pool: SqlitePool) {
    DB_POOL.set(pool).ok();
}

// Tauri 入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Phase 4 将在这里注册 IPC 命令
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
