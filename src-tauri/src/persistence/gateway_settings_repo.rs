use sqlx::SqlitePool;

use crate::models::{GatewaySettings, UpdateGatewaySettings};

pub struct GatewaySettingsRepo;

impl GatewaySettingsRepo {
    /// 运行时默认网关设置（数据库不存在配置时使用）
    pub fn runtime_default(now: chrono::NaiveDateTime) -> GatewaySettings {
        GatewaySettings {
            id: "default".to_string(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 2013,
            allow_remote: 0,
            auth_token_hash: None,
            log_retention_days: 30,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: 0,
            rate_limit_max_requests_per_minute: 1000,
            rate_limit_max_tokens_per_minute: 500000,
            created_at: now,
            updated_at: now,
        }
    }

    /// 读取当前全局网关设置；如果数据库中还没有记录，则返回运行时默认值
    pub async fn load_effective(pool: &SqlitePool) -> Result<GatewaySettings, sqlx::Error> {
        if let Some(settings) = Self::find(pool).await? {
            return Ok(settings);
        }

        Ok(Self::runtime_default(chrono::Utc::now().naive_utc()))
    }

    /// 查询当前全局网关设置
    pub async fn find(pool: &SqlitePool) -> Result<Option<GatewaySettings>, sqlx::Error> {
        sqlx::query_as::<_, GatewaySettings>(
            r#"SELECT * FROM gateway_settings WHERE id = 'default'"#,
        )
        .fetch_optional(pool)
        .await
    }

    /// 更新全局网关设置
    pub async fn update(
        pool: &SqlitePool,
        update: &UpdateGatewaySettings,
    ) -> Result<GatewaySettings, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let allow_remote = update.allow_remote.map(|v| if v { 1 } else { 0 });

        let mut tx = pool.begin().await?;

        let rate_limit_enabled = update.rate_limit_enabled.map(|v| if v { 1 } else { 0 });

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO gateway_settings (
                id, bind_host, bind_port, allow_remote, log_retention_days,
                rate_limit_enabled, rate_limit_max_requests_per_minute, rate_limit_max_tokens_per_minute,
                created_at, updated_at
            )
            VALUES ('default', '127.0.0.1', 2013, 0, 30, 0, 1000, 500000, $1, $1)
            "#,
        )
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query_as::<_, GatewaySettings>(
            r#"
            UPDATE gateway_settings
            SET bind_host = COALESCE($2, bind_host),
                bind_port = COALESCE($3, bind_port),
                allow_remote = COALESCE($4, allow_remote),
                auth_token_hash = COALESCE($5, auth_token_hash),
                log_retention_days = COALESCE($6, log_retention_days),
                default_provider_id = COALESCE($7, default_provider_id),
                default_route_id = COALESCE($8, default_route_id),
                rate_limit_enabled = COALESCE($9, rate_limit_enabled),
                rate_limit_max_requests_per_minute = COALESCE($10, rate_limit_max_requests_per_minute),
                rate_limit_max_tokens_per_minute = COALESCE($11, rate_limit_max_tokens_per_minute),
                updated_at = $12
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind("default")
        .bind(update.bind_host.as_deref())
        .bind(update.bind_port)
        .bind(allow_remote)
        .bind(update.auth_token_hash.as_deref())
        .bind(update.log_retention_days)
        .bind(update.default_provider_id.as_deref())
        .bind(update.default_route_id.as_deref())
        .bind(rate_limit_enabled)
        .bind(update.rate_limit_max_requests_per_minute)
        .bind(update.rate_limit_max_tokens_per_minute)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result)
    }
}
