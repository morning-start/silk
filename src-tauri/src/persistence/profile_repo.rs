use sqlx::SqlitePool;

use crate::models::{NewProfile, Profile, UpdateProfile};
use crate::persistence::defaults;

pub struct ProfileRepo;

impl ProfileRepo {
    /// 创建新 Profile
    pub async fn create(pool: &SqlitePool, new: &NewProfile) -> Result<Profile, sqlx::Error> {
        let (id, now) = defaults::new_id_and_now();

        sqlx::query_as::<_, Profile>(
            r#"
            INSERT INTO profiles (id, name, agent_type, config_json, is_active, sort_index, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.agent_type)
        .bind(&new.config_json)
        .bind(new.is_active.unwrap_or(false))
        .bind(new.sort_index)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    /// 根据 ID 查询
    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Profile>, sqlx::Error> {
        sqlx::query_as::<_, Profile>(r#"SELECT * FROM profiles WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// 查询所有 Profile
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Profile>, sqlx::Error> {
        sqlx::query_as::<_, Profile>(
            r#"SELECT * FROM profiles ORDER BY sort_index ASC, created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }

    /// 按 agent_type 查询
    pub async fn find_by_agent_type(
        pool: &SqlitePool,
        agent_type: &str,
    ) -> Result<Vec<Profile>, sqlx::Error> {
        sqlx::query_as::<_, Profile>(
            r#"SELECT * FROM profiles WHERE agent_type = $1 ORDER BY sort_index ASC, created_at DESC"#,
        )
        .bind(agent_type)
        .fetch_all(pool)
        .await
    }

    /// 查询当前激活的 Profile（某 Agent 类型）
    pub async fn find_active(
        pool: &SqlitePool,
        agent_type: &str,
    ) -> Result<Option<Profile>, sqlx::Error> {
        sqlx::query_as::<_, Profile>(
            r#"SELECT * FROM profiles WHERE agent_type = $1 AND is_active = 1 LIMIT 1"#,
        )
        .bind(agent_type)
        .fetch_optional(pool)
        .await
    }

    /// 更新 Profile
    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateProfile,
    ) -> Result<Option<Profile>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();

        sqlx::query_as::<_, Profile>(
            r#"
            UPDATE profiles
            SET name = COALESCE($2, name),
                config_json = COALESCE($3, config_json),
                is_active = COALESCE($4, is_active),
                sort_index = COALESCE($5, sort_index),
                updated_at = $6
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&update.name)
        .bind(&update.config_json)
        .bind(update.is_active)
        .bind(update.sort_index)
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    /// 删除 Profile
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM profiles WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 将某 Agent 类型的所有 Profile 置为未激活
    pub async fn deactivate_all(pool: &SqlitePool, agent_type: &str) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE profiles SET is_active = 0, updated_at = $2 WHERE agent_type = $1"#)
            .bind(agent_type)
            .bind(chrono::Utc::now().naive_utc())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 激活指定 Profile
    pub async fn activate(pool: &SqlitePool, id: &str) -> Result<Option<Profile>, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_as::<_, Profile>(
            r#"UPDATE profiles SET is_active = 1, updated_at = $2 WHERE id = $1 RETURNING *"#,
        )
        .bind(id)
        .bind(now)
        .fetch_optional(pool)
        .await
    }
}