use sqlx::Row;
use sqlx::SqlitePool;

use crate::models::{
    MappingChannelInfo, ModelMapping, ModelMappingChannel, NewMappingChannel, NewModelMapping,
    UpdateModelMapping,
};

pub struct ModelMappingRepo;

impl ModelMappingRepo {
    /// 创建新模型映射（含关联渠道，事务保证原子性）
    pub async fn create(
        pool: &SqlitePool,
        new: &NewModelMapping,
    ) -> Result<ModelMapping, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let enabled = if new.enabled.unwrap_or(true) { 1 } else { 0 };
        let capabilities = serde_json::to_string(&new.capabilities.as_deref().unwrap_or(&[]))
            .unwrap_or_else(|_| "[]".to_string());
        let strategy = new.strategy.as_deref().unwrap_or("round_robin");

        let mapping = sqlx::query_as::<_, ModelMapping>(
            r#"
            INSERT INTO model_mappings (
                id, model_name,
                max_input_tokens, max_context_tokens, max_output_tokens,
                input_price_per_1m, output_price_per_1m,
                capabilities, description,
                strategy, enabled, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(new.model_name.as_str())
        .bind(new.max_input_tokens)
        .bind(new.max_context_tokens)
        .bind(new.max_output_tokens)
        .bind(new.input_price_per_1m)
        .bind(new.output_price_per_1m)
        .bind(capabilities)
        .bind(new.description.as_deref().unwrap_or(""))
        .bind(strategy)
        .bind(enabled)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        // 事务内插入关联渠道
        if let Some(ref channels) = new.channels {
            Self::replace_channels_internal(&mut *tx, &id, channels).await?;
        }

        tx.commit().await?;
        Ok(mapping)
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
        sqlx::query_as::<_, ModelMapping>(r#"SELECT * FROM model_mappings WHERE model_name = $1"#)
            .bind(model_name)
            .fetch_optional(pool)
            .await
    }

    /// 根据多个模型名称批量查询（用于日志 cost 计算）
    pub async fn find_by_model_names(
        pool: &SqlitePool,
        model_names: &[String],
    ) -> Result<Vec<ModelMapping>, sqlx::Error> {
        if model_names.is_empty() {
            return Ok(Vec::new());
        }
        // 使用 json_each 将 Rust 数组传递为 JSON 数组，避免动态拼接占位符
        let names_json = serde_json::to_string(model_names).unwrap_or_default();
        sqlx::query_as::<_, ModelMapping>(
            r#"SELECT * FROM model_mappings WHERE model_name IN (SELECT value FROM json_each($1))"#,
        )
        .bind(names_json)
        .fetch_all(pool)
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

    /// 更新模型映射（含关联渠道替换，事务保证原子性）
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateModelMapping,
    ) -> Result<Option<ModelMapping>, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let now = chrono::Utc::now().naive_utc();
        let enabled = update.enabled.map(|v| if v { 1 } else { 0 });

        let capabilities = update
            .capabilities
            .as_ref()
            .map(|caps| serde_json::to_string(caps).unwrap_or_else(|_| "[]".to_string()));

        let Some(mapping) = sqlx::query_as::<_, ModelMapping>(
            r#"
            UPDATE model_mappings
            SET model_name = COALESCE($2, model_name),
                max_input_tokens = COALESCE($3, max_input_tokens),
                max_context_tokens = COALESCE($4, max_context_tokens),
                max_output_tokens = COALESCE($5, max_output_tokens),
                input_price_per_1m = COALESCE($6, input_price_per_1m),
                output_price_per_1m = COALESCE($7, output_price_per_1m),
                capabilities = COALESCE($8, capabilities),
                description = COALESCE($9, description),
                strategy = COALESCE($10, strategy),
                enabled = COALESCE($11, enabled),
                updated_at = $12
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update.model_name.as_deref())
        .bind(update.max_input_tokens)
        .bind(update.max_context_tokens)
        .bind(update.max_output_tokens)
        .bind(update.input_price_per_1m)
        .bind(update.output_price_per_1m)
        .bind(capabilities.as_deref())
        .bind(update.description.as_deref())
        .bind(update.strategy.as_deref())
        .bind(enabled)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };

        // 在事务内替换关联渠道
        if let Some(ref channels) = update.channels {
            Self::replace_channels_internal(&mut *tx, id, channels).await?;
        }

        tx.commit().await?;
        Ok(Some(mapping))
    }

    /// 删除模型映射（关联渠道由 ON DELETE CASCADE 自动清理）
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

    /// 查询某映射的所有关联渠道（带渠道详情）
    pub async fn find_channels_by_mapping_id(
        pool: &SqlitePool,
        mapping_id: &str,
    ) -> Result<Vec<MappingChannelInfo>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT mmc.id, mmc.mapping_id, mmc.provider_id,
                   p.name as provider_name, p.protocols, p.models, p.health_status,
                   mmc.selected_models, mmc.enabled
            FROM model_mapping_channels mmc
            JOIN providers p ON p.id = mmc.provider_id
            WHERE mmc.mapping_id = ?1
            ORDER BY mmc.created_at
            "#,
        )
        .bind(mapping_id)
        .fetch_all(pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let protocols: Vec<String> =
                serde_json::from_str(row.get::<&str, _>("protocols")).unwrap_or_default();
            let models: Vec<String> =
                serde_json::from_str(row.get::<&str, _>("models")).unwrap_or_default();
            let selected_models: Vec<String> =
                serde_json::from_str(row.get::<&str, _>("selected_models")).unwrap_or_default();
            result.push(MappingChannelInfo {
                id: row.get("id"),
                mapping_id: row.get("mapping_id"),
                provider_id: row.get("provider_id"),
                provider_name: row.get("provider_name"),
                provider_protocols: protocols,
                provider_models: models.clone(),
                provider_models_count: models.len() as i64,
                provider_health: row.get("health_status"),
                selected_models,
                enabled: row.get::<i64, _>("enabled") != 0,
            });
        }
        Ok(result)
    }

    /// 查询某映射的启用关联渠道（网关路由用，轻量）
    pub async fn find_enabled_channels(
        pool: &SqlitePool,
        mapping_id: &str,
    ) -> Result<Vec<ModelMappingChannel>, sqlx::Error> {
        sqlx::query_as::<_, ModelMappingChannel>(
            r#"
            SELECT * FROM model_mapping_channels
            WHERE mapping_id = ?1 AND enabled = 1
            ORDER BY created_at
            "#,
        )
        .bind(mapping_id)
        .fetch_all(pool)
        .await
    }

    /// 批量替换关联渠道（使用连接，可在事务外调用）
    async fn replace_channels_internal(
        conn: &mut sqlx::SqliteConnection,
        mapping_id: &str,
        channels: &[NewMappingChannel],
    ) -> Result<(), sqlx::Error> {
        // 先删除旧的
        sqlx::query(r#"DELETE FROM model_mapping_channels WHERE mapping_id = ?1"#)
            .bind(mapping_id)
            .execute(&mut *conn)
            .await?;

        // 再批量插入
        let now = chrono::Utc::now().naive_utc();
        for channel in channels {
            let id = uuid::Uuid::new_v4().to_string();
            let enabled = if channel.enabled.unwrap_or(true) {
                1
            } else {
                0
            };
            let selected_models =
                serde_json::to_string(&channel.selected_models.as_deref().unwrap_or(&[]))
                    .unwrap_or_else(|_| "[]".to_string());
            sqlx::query(
                r#"
                INSERT INTO model_mapping_channels (id, mapping_id, provider_id, selected_models, enabled, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
            )
            .bind(id)
            .bind(mapping_id)
            .bind(&channel.provider_id)
            .bind(selected_models)
            .bind(enabled)
            .bind(now)
            .execute(&mut *conn)
            .await?;
        }

        Ok(())
    }

    /// 查询分组内的渠道信息（废弃，保留兼容）
    pub async fn find_group_providers(
        pool: &SqlitePool,
        group_id: &str,
    ) -> Result<Vec<crate::models::GroupProviderInfo>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT p.id, p.name, p.protocols, p.models, p.health_status
            FROM group_members gm
            JOIN providers p ON p.id = gm.provider_id
            WHERE gm.group_id = ?1 AND gm.enabled = 1
            "#,
        )
        .bind(group_id)
        .fetch_all(pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let protocols: Vec<String> =
                serde_json::from_str(row.get::<&str, _>("protocols")).unwrap_or_default();
            let models: Vec<String> =
                serde_json::from_str(row.get::<&str, _>("models")).unwrap_or_default();

            result.push(crate::models::GroupProviderInfo {
                id: row.get("id"),
                name: row.get("name"),
                protocols,
                models_count: models.len() as i64,
                health_status: row.get("health_status"),
            });
        }
        Ok(result)
    }
}
