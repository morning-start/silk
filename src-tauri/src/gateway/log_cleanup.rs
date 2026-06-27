use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::models::GatewaySettings;
use crate::persistence::LogRepo;

/// 启动后台日志清理任务
///
/// 每小时检查一次，删除超过 `log_retention_days` 的日志
pub fn spawn_log_cleanup_task(
    pool: SqlitePool,
    settings: Arc<RwLock<GatewaySettings>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 每小时

        loop {
            interval.tick().await;

            let retention_days = {
                let s = settings.read().await;
                s.log_retention_days
            };

            if retention_days <= 0 {
                continue; // 禁用清理
            }

            let before = chrono::Utc::now().naive_utc() - chrono::Duration::days(retention_days);

            match LogRepo::delete_before(&pool, before).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(deleted, "自动清理过期日志");
                    }
                }
                Err(err) => {
                    tracing::warn!(%err, "自动清理日志失败");
                }
            }
        }
    })
}
