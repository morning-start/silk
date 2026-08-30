use sqlx::{Row, SqlitePool};

use crate::LookupCache;

/// 从 DB 一次性加载所有字典表到 `LookupCache`。
///
/// 将原本散落在 `lib.rs` 组合根中的字典表查询集中到持久化层，
/// 保证「SQL 只出现在 persistence 层」这一架构约束。
pub struct LookupCacheRepo;

impl LookupCacheRepo {
    pub async fn load(pool: &SqlitePool) -> LookupCache {
        let provider_names = sqlx::query("SELECT id, name FROM providers")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.get::<String, _>("id"), r.get::<String, _>("name")))
            .collect();

        let model_mapping_names = sqlx::query("SELECT id, model_name FROM model_mappings")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.get::<String, _>("id"), r.get::<String, _>("model_name")))
            .collect();

        LookupCache {
            provider_names,
            model_mapping_names,
        }
    }
}
