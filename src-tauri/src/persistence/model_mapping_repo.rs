use sqlx::Row;
use sqlx::SqlitePool;

use crate::models::{ModelMapping, NewModelMapping, UpdateModelMapping};

pub struct ModelMappingRepo;

impl ModelMappingRepo {
    /// 创建新模型映射
    pub async fn create(
        pool: &SqlitePool,
        new: &NewModelMapping,
    ) -> Result<ModelMapping, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let enabled = if new.enabled.unwrap_or(true) { 1 } else { 0 };
        let capabilities = serde_json::to_string(&new.capabilities.as_deref().unwrap_or(&[]))
            .unwrap_or_else(|_| "[]".to_string());

        sqlx::query_as::<_, ModelMapping>(
            r#"
            INSERT INTO model_mappings (
                id, model_name, provider_group_id,
                max_input_tokens, max_context_tokens, max_output_tokens,
                input_price_per_1m, output_price_per_1m,
                capabilities, enabled, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new.model_name.as_str())
        .bind(new.provider_group_id.as_deref())
        .bind(new.max_input_tokens)
        .bind(new.max_context_tokens)
        .bind(new.max_output_tokens)
        .bind(new.input_price_per_1m)
        .bind(new.output_price_per_1m)
        .bind(capabilities)
        .bind(enabled)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    /// 根据 ID 查询
    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<ModelMapping>, sqlx::Error> {
        sqlx::query_as::<_, ModelMapping>(r#"SELECT * FROM model_mappings WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// 根据模型名称查询
    pub async fn find_by_model_name(
        pool: &SqlitePool,
        model_name: &str,
    ) -> Result<Option<ModelMapping>, sqlx::Error> {
        sqlx::query_as::<_, ModelMapping>(
            r#"SELECT * FROM model_mappings WHERE model_name = $1"#,
        )
        .bind(model_name)
        .fetch_optional(pool)
        .await
    }

    /// 查询所有模型映射
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<ModelMapping>, sqlx::Error> {
        sqlx::query_as::<_, ModelMapping>(
            r#"SELECT * FROM model_mappings ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 查询所有启用的模型映射
    pub async fn find_enabled(pool: &SqlitePool) -> Result<Vec<ModelMapping>, sqlx::Error> {
        sqlx::query_as::<_, ModelMapping>(
            r#"SELECT * FROM model_mappings WHERE enabled = 1 ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 更新模型映射
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateModelMapping,
    ) -> Result<Option<ModelMapping>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let enabled = update.enabled.map(|v| if v { 1 } else { 0 });

        let capabilities = update.capabilities.as_ref().map(|caps| {
            serde_json::to_string(caps).unwrap_or_else(|_| "[]".to_string())
        });

        let Some(_) =
            sqlx::query_as::<_, ModelMapping>(r#"SELECT * FROM model_mappings WHERE id = $1"#)
                .bind(id)
                .fetch_optional(pool)
                .await?
        else {
            return Ok(None);
        };

        sqlx::query_as::<_, ModelMapping>(
            r#"
            UPDATE model_mappings
            SET model_name = COALESCE($2, model_name),
                provider_group_id = COALESCE($3, provider_group_id),
                max_input_tokens = COALESCE($4, max_input_tokens),
                max_context_tokens = COALESCE($5, max_context_tokens),
                max_output_tokens = COALESCE($6, max_output_tokens),
                input_price_per_1m = COALESCE($7, input_price_per_1m),
                output_price_per_1m = COALESCE($8, output_price_per_1m),
                capabilities = COALESCE($9, capabilities),
                enabled = COALESCE($10, enabled),
                updated_at = $11
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update.model_name.as_deref())
        .bind(update.provider_group_id.as_deref())
        .bind(update.max_input_tokens)
        .bind(update.max_context_tokens)
        .bind(update.max_output_tokens)
        .bind(update.input_price_per_1m)
        .bind(update.output_price_per_1m)
        .bind(capabilities.as_deref())
        .bind(enabled)
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    /// 删除模型映射
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM model_mappings WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 统计模型映射数量
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(r#"SELECT COUNT(*) as count FROM model_mappings"#)
            .fetch_one(pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}
