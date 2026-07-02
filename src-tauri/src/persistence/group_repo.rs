use sqlx::SqlitePool;

use crate::models::{
    GroupMember, NewGroupMember, NewProviderGroup, ProviderGroup, UpdateGroupMember,
    UpdateProviderGroup,
};

pub struct GroupRepo;

impl GroupRepo {
    // ---------------------------------------------------------------------------
    // ProviderGroup CRUD
    // ---------------------------------------------------------------------------

    pub async fn create_group(
        pool: &SqlitePool,
        new: &NewProviderGroup,
    ) -> Result<ProviderGroup, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let strategy = new.strategy.as_deref().unwrap_or("round_robin");
        let enabled = if new.enabled.unwrap_or(true) { 1 } else { 0 };

        sqlx::query_as::<_, ProviderGroup>(
            r#"
            INSERT INTO provider_groups (id, name, model_name, strategy, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING id, name, model_name, strategy, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(new.name.clone())
        .bind(new.model_name.clone())
        .bind(strategy)
        .bind(enabled)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    pub async fn find_group_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<ProviderGroup>, sqlx::Error> {
        sqlx::query_as::<_, ProviderGroup>(
            r#"SELECT id, name, model_name, strategy, enabled, created_at, updated_at FROM provider_groups WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_groups_by_model(
        pool: &SqlitePool,
        model_name: &str,
    ) -> Result<Vec<ProviderGroup>, sqlx::Error> {
        sqlx::query_as::<_, ProviderGroup>(
            r#"SELECT id, name, model_name, strategy, enabled, created_at, updated_at FROM provider_groups WHERE model_name = $1 AND enabled = 1 ORDER BY created_at DESC"#,
        )
        .bind(model_name)
        .fetch_all(pool)
        .await
    }

    pub async fn find_all_groups(pool: &SqlitePool) -> Result<Vec<ProviderGroup>, sqlx::Error> {
        sqlx::query_as::<_, ProviderGroup>(
            r#"SELECT id, name, model_name, strategy, enabled, created_at, updated_at FROM provider_groups ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn update_group(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateProviderGroup,
    ) -> Result<Option<ProviderGroup>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();

        sqlx::query_as::<_, ProviderGroup>(
            r#"
            UPDATE provider_groups
            SET name = COALESCE($2, name),
                model_name = COALESCE($3, model_name),
                strategy = COALESCE($4, strategy),
                enabled = COALESCE($5, enabled),
                updated_at = $6
            WHERE id = $1
            RETURNING id, name, model_name, strategy, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(update.name.as_ref())
        .bind(update.model_name.as_ref())
        .bind(update.strategy.as_ref())
        .bind(update.enabled.map(|v| if v { 1 } else { 0 }))
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete_group(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM provider_groups WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ---------------------------------------------------------------------------
    // GroupMember CRUD
    // ---------------------------------------------------------------------------

    pub async fn add_member(
        pool: &SqlitePool,
        new: &NewGroupMember,
    ) -> Result<GroupMember, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let weight = new.weight.unwrap_or(1);

        sqlx::query_as::<_, GroupMember>(
            r#"
            INSERT INTO group_members (id, group_id, provider_id, weight, enabled, created_at)
            VALUES ($1, $2, $3, $4, 1, $5)
            RETURNING id, group_id, provider_id, weight, enabled, created_at
            "#,
        )
        .bind(id)
        .bind(&new.group_id)
        .bind(&new.provider_id)
        .bind(weight)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    pub async fn find_members_by_group(
        pool: &SqlitePool,
        group_id: &str,
    ) -> Result<Vec<GroupMember>, sqlx::Error> {
        sqlx::query_as::<_, GroupMember>(
            r#"SELECT id, group_id, provider_id, weight, enabled, created_at FROM group_members WHERE group_id = $1 ORDER BY created_at ASC"#,
        )
        .bind(group_id)
        .fetch_all(pool)
        .await
    }

    /// 根据 member ID 查询单个成员
    pub async fn find_member_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<GroupMember>, sqlx::Error> {
        sqlx::query_as::<_, GroupMember>(
            r#"SELECT id, group_id, provider_id, weight, enabled, created_at FROM group_members WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn update_member(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateGroupMember,
    ) -> Result<Option<GroupMember>, sqlx::Error> {
        sqlx::query_as::<_, GroupMember>(
            r#"
            UPDATE group_members
            SET weight = COALESCE($2, weight),
                enabled = COALESCE($3, enabled)
            WHERE id = $1
            RETURNING id, group_id, provider_id, weight, enabled, created_at
            "#,
        )
        .bind(id)
        .bind(update.weight)
        .bind(update.enabled.map(|v| if v { 1 } else { 0 }))
        .fetch_optional(pool)
        .await
    }

    pub async fn remove_member(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM group_members WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
