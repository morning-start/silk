use sqlx::SqlitePool;

use crate::models::{GatewaySettings, UpdateGatewaySettings};

pub struct GatewaySettingsRepo;

impl GatewaySettingsRepo {
    /// 确保全局网关设置存在，并返回当前值
    pub async fn get_or_create_default(pool: &SqlitePool) -> Result<GatewaySettings, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();

        sqlx::query!(
            r#"
            INSERT OR IGNORE INTO gateway_settings (
                id, bind_host, bind_port, allow_remote, log_retention_days, created_at, updated_at
            )
            VALUES ('default', '127.0.0.1', 3000, 0, 30, $1, $1)
            "#,
            now,
        )
        .execute(pool)
        .await?;

        sqlx::query_as::<_, GatewaySettings>(
            r#"SELECT * FROM gateway_settings WHERE id = 'default'"#,
        )
        .fetch_one(pool)
        .await
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

        sqlx::query_as::<_, GatewaySettings>(
            r#"
            UPDATE gateway_settings
            SET bind_host = COALESCE($2, bind_host),
                bind_port = COALESCE($3, bind_port),
                allow_remote = COALESCE($4, allow_remote),
                auth_token_hash = COALESCE($5, auth_token_hash),
                log_retention_days = COALESCE($6, log_retention_days),
                default_provider_id = COALESCE($7, default_provider_id),
                default_route_id = COALESCE($8, default_route_id),
                updated_at = $9
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
        .bind(now)
        .fetch_one(pool)
        .await
    }
}
