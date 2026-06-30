use sqlx::SqlitePool;

use crate::models::{NewProvider, Provider, UpdateProvider};
use crate::persistence::defaults;

pub struct ProviderRepo;

impl ProviderRepo {
    /// 创建新 Provider
    pub async fn create(pool: &SqlitePool, new: &NewProvider) -> Result<Provider, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();

        sqlx::query_as::<_, Provider>(
            r#"
            INSERT INTO providers (id, name, protocols, models, keys, key_strategy, api_base_url,
                                   proxy_url, timeout_seconds, max_retries, status, health_status,
                                   last_health_check_at, metadata_json, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new.name.as_str())
        .bind(serde_json::to_string(&new.protocols).unwrap_or_default())
        .bind(serde_json::to_string(&new.models).unwrap_or_default())
        .bind(serde_json::to_string(&new.keys).unwrap_or_default())
        .bind(new.key_strategy.as_deref().unwrap_or("round_robin"))
        .bind(new.api_base_url.as_str())
        .bind(new.proxy_url.as_deref())
        .bind(new.timeout_seconds.unwrap_or(defaults::DEFAULT_PROVIDER_TIMEOUT_SECONDS))
        .bind(new.max_retries.unwrap_or(defaults::DEFAULT_PROVIDER_MAX_RETRIES))
        .bind(new.status.as_deref().unwrap_or("enabled"))
        .bind(new.health_status.as_deref().unwrap_or("unknown"))
        .bind(new.last_health_check_at)
        .bind(new.metadata_json.as_deref())
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    /// 根据 ID 查询 Provider
    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Provider>, sqlx::Error> {
        sqlx::query_as::<_, Provider>(r#"SELECT * FROM providers WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// 查询所有 Provider
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Provider>, sqlx::Error> {
        sqlx::query_as::<_, Provider>(r#"SELECT * FROM providers ORDER BY created_at DESC"#)
            .fetch_all(pool)
            .await
    }

    /// 查询所有启用的 Provider
    pub async fn find_enabled(pool: &SqlitePool) -> Result<Vec<Provider>, sqlx::Error> {
        sqlx::query_as::<_, Provider>(
            r#"SELECT * FROM providers WHERE status = 'enabled' ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 更新 Provider
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateProvider,
    ) -> Result<Option<Provider>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();

        sqlx::query_as::<_, Provider>(
            r#"
            UPDATE providers
            SET name = COALESCE($2, name),
                protocols = COALESCE($3, protocols),
                models = COALESCE($4, models),
                keys = COALESCE($5, keys),
                key_strategy = COALESCE($6, key_strategy),
                api_base_url = COALESCE($7, api_base_url),
                proxy_url = COALESCE($8, proxy_url),
                timeout_seconds = COALESCE($9, timeout_seconds),
                max_retries = COALESCE($10, max_retries),
                status = COALESCE($11, status),
                health_status = COALESCE($12, health_status),
                last_health_check_at = COALESCE($13, last_health_check_at),
                metadata_json = COALESCE($14, metadata_json),
                updated_at = $15
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update.name.as_deref())
        .bind(
            update
                .protocols
                .as_ref()
                .map(|p| serde_json::to_string(p).unwrap_or_default()),
        )
        .bind(
            update
                .models
                .as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_default()),
        )
        .bind(
            update
                .keys
                .as_ref()
                .map(|k| serde_json::to_string(k).unwrap_or_default()),
        )
        .bind(update.key_strategy.as_deref())
        .bind(update.api_base_url.as_deref())
        .bind(update.proxy_url.as_deref())
        .bind(update.timeout_seconds)
        .bind(update.max_retries)
        .bind(update.status.as_deref())
        .bind(update.health_status.as_deref())
        .bind(update.last_health_check_at)
        .bind(update.metadata_json.as_deref())
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    /// 更新 Provider 健康状态（连通性测试后调用）
    pub async fn update_health_status(
        pool: &SqlitePool,
        id: &str,
        health_status: &str,
        checked_at: chrono::NaiveDateTime,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE providers
            SET health_status = $2, last_health_check_at = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(health_status)
        .bind(checked_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// 删除 Provider
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM providers WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
