use sqlx::SqlitePool;

use crate::models::{NewRoutingRule, RoutingRule, UpdateRoutingRule};

pub struct RoutingRuleRepo;

impl RoutingRuleRepo {
    /// 创建新路由规则
    pub async fn create(
        pool: &SqlitePool,
        new: &NewRoutingRule,
    ) -> Result<RoutingRule, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let protocol_conversion = if new.protocol_conversion.unwrap_or(true) {
            1
        } else {
            0
        };
        let enabled = if new.enabled.unwrap_or(true) { 1 } else { 0 };

        sqlx::query_as::<_, RoutingRule>(
            r#"
            INSERT INTO routing_rules (id, name, match_host, match_path, match_method, match_content_type,
                                       inbound_protocol, outbound_protocol, target_provider_id, target_group_id,
                                       failover_provider_id, protocol_conversion, model_name_override, metadata_json,
                                       priority, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new.name.as_str())
        .bind(new.match_host.as_deref())
        .bind(new.match_path.as_str())
        .bind(new.match_method.as_deref().unwrap_or("*"))
        .bind(new.match_content_type.as_deref())
        .bind(new.inbound_protocol.as_deref().unwrap_or("any"))
        .bind(new.outbound_protocol.as_deref().unwrap_or("openai_response"))
        .bind(new.target_provider_id.as_str())
        .bind(new.target_group_id.as_deref())
        .bind(new.failover_provider_id.as_deref())
        .bind(protocol_conversion)
        .bind(new.model_name_override.as_deref())
        .bind(new.metadata_json.as_deref())
        .bind(new.priority.unwrap_or(100))
        .bind(enabled)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    /// 根据 ID 查询规则
    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<RoutingRule>, sqlx::Error> {
        sqlx::query_as::<_, RoutingRule>(r#"SELECT * FROM routing_rules WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// 查询所有规则（按优先级排序）
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<RoutingRule>, sqlx::Error> {
        sqlx::query_as::<_, RoutingRule>(
            r#"SELECT * FROM routing_rules ORDER BY priority ASC, created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 查询所有启用的规则（按优先级排序，供 Route Manager 构建路由表使用）
    pub async fn find_enabled_ordered(pool: &SqlitePool) -> Result<Vec<RoutingRule>, sqlx::Error> {
        sqlx::query_as::<_, RoutingRule>(
            r#"
            SELECT * FROM routing_rules
            WHERE enabled = 1
            ORDER BY priority ASC, created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    /// 更新路由规则
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateRoutingRule,
    ) -> Result<Option<RoutingRule>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        let protocol_conversion = update.protocol_conversion.map(|v| if v { 1 } else { 0 });
        let enabled = update.enabled.map(|v| if v { 1 } else { 0 });

        let Some(_) =
            sqlx::query_as::<_, RoutingRule>(r#"SELECT * FROM routing_rules WHERE id = $1"#)
                .bind(id)
                .fetch_optional(pool)
                .await?
        else {
            return Ok(None);
        };

        sqlx::query_as::<_, RoutingRule>(
            r#"
            UPDATE routing_rules
            SET name = COALESCE($2, name),
                match_host = COALESCE($3, match_host),
                match_path = COALESCE($4, match_path),
                match_method = COALESCE($5, match_method),
                match_content_type = COALESCE($6, match_content_type),
                inbound_protocol = COALESCE($7, inbound_protocol),
                outbound_protocol = COALESCE($8, outbound_protocol),
                target_provider_id = COALESCE($9, target_provider_id),
                target_group_id = COALESCE($10, target_group_id),
                failover_provider_id = COALESCE($11, failover_provider_id),
                protocol_conversion = COALESCE($12, protocol_conversion),
                model_name_override = COALESCE($13, model_name_override),
                metadata_json = COALESCE($14, metadata_json),
                priority = COALESCE($15, priority),
                enabled = COALESCE($16, enabled),
                updated_at = $17
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(update.name.as_deref())
        .bind(update.match_host.as_deref())
        .bind(update.match_path.as_deref())
        .bind(update.match_method.as_deref())
        .bind(update.match_content_type.as_deref())
        .bind(update.inbound_protocol.as_deref())
        .bind(update.outbound_protocol.as_deref())
        .bind(update.target_provider_id.as_deref())
        .bind(update.target_group_id.as_deref())
        .bind(update.failover_provider_id.as_deref())
        .bind(protocol_conversion)
        .bind(update.model_name_override.as_deref())
        .bind(update.metadata_json.as_deref())
        .bind(update.priority)
        .bind(enabled)
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    /// 删除路由规则
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(r#"DELETE FROM routing_rules WHERE id = $1"#, id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
