use sqlx::Row;
use sqlx::SqlitePool;

use crate::crypto::{encrypt, hash_api_key};
use crate::models::{GatewayKey, NewGatewayKey, UpdateGatewayKey};
use crate::persistence::defaults;

pub struct GatewayKeyRepo;

impl GatewayKeyRepo {
    /// 创建新 Key
    pub async fn create(
        pool: &SqlitePool,
        new: &NewGatewayKey,
    ) -> Result<(GatewayKey, String), sqlx::Error> {
        let (id, now) = defaults::new_id_and_now();
        let enabled = defaults::bool_to_i64(new.enabled, true);
        let key_hash = hash_api_key(&new.key_value);
        let encrypted_key_value =
            encrypt(&new.key_value).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let max_concurrent = new
            .max_concurrent
            .unwrap_or(defaults::DEFAULT_KEY_MAX_CONCURRENT);

        let row = sqlx::query(
            r#"
            INSERT INTO gateway_keys (
                id, name, key_hash, encrypted_key_value, enabled,
                expires_at, max_concurrent, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new.name.as_str())
        .bind(key_hash)
        .bind(encrypted_key_value)
        .bind(enabled)
        .bind(new.expires_at)
        .bind(max_concurrent)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;

        let key = GatewayKey {
            id: row.get("id"),
            name: row.get("name"),
            key_hash: row.get("key_hash"),
            encrypted_key_value: row.get("encrypted_key_value"),
            enabled: row.get("enabled"),
            expires_at: row.get("expires_at"),
            max_concurrent: row.get("max_concurrent"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        // 返回 key 和明文 value（仅创建时返回一次）
        Ok((key, new.key_value.clone()))
    }

    /// 根据 ID 查询
    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<GatewayKey>, sqlx::Error> {
        sqlx::query_as::<_, GatewayKey>(r#"SELECT * FROM gateway_keys WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// 查询所有 Key
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<GatewayKey>, sqlx::Error> {
        sqlx::query_as::<_, GatewayKey>(r#"SELECT * FROM gateway_keys ORDER BY created_at DESC"#)
            .fetch_all(pool)
            .await
    }

    /// 根据 key 哈希查询（用于验证）
    pub async fn find_by_hash(
        pool: &SqlitePool,
        key_hash: &str,
    ) -> Result<Option<GatewayKey>, sqlx::Error> {
        sqlx::query_as::<_, GatewayKey>(
            r#"SELECT * FROM gateway_keys WHERE key_hash = $1 AND enabled = 1"#,
        )
        .bind(key_hash)
        .fetch_optional(pool)
        .await
    }

    /// 更新 Key
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateGatewayKey,
    ) -> Result<Option<GatewayKey>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let enabled = update.enabled.map(|v| if v { 1 } else { 0 });

        sqlx::query_as::<_, GatewayKey>(
            r#"
            UPDATE gateway_keys
            SET name = COALESCE($2, name),
                enabled = COALESCE($3, enabled),
                expires_at = COALESCE($4, expires_at),
                max_concurrent = COALESCE($5, max_concurrent),
                updated_at = $6
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update.name.as_deref())
        .bind(enabled)
        .bind(update.expires_at)
        .bind(update.max_concurrent)
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    /// 删除 Key
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM gateway_keys WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
