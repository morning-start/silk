use sqlx::SqlitePool;

/// 通用配置片段（仅用于 DB 读写）
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct CommonConfigSnippet {
    pub id: String,
    pub agent_type: String,
    pub content: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

pub struct CommonConfigSnippetRepo;

impl CommonConfigSnippetRepo {
    /// 按 agent_type 查询
    pub async fn find_by_agent(
        pool: &SqlitePool,
        agent_type: &str,
    ) -> Result<Option<CommonConfigSnippet>, sqlx::Error> {
        sqlx::query_as::<_, CommonConfigSnippet>(
            r#"SELECT * FROM common_config_snippets WHERE agent_type = $1"#,
        )
        .bind(agent_type)
        .fetch_optional(pool)
        .await
    }

    /// 创建或更新（upsert by agent_type）
    pub async fn upsert(
        pool: &SqlitePool,
        agent_type: &str,
        content: &str,
    ) -> Result<CommonConfigSnippet, sqlx::Error> {
        let now = chrono::Utc::now().naive_utc();

        // 尝试先更新
        let existing = sqlx::query_as::<_, CommonConfigSnippet>(
            r#"UPDATE common_config_snippets SET content = $2, updated_at = $3 WHERE agent_type = $1 RETURNING *"#,
        )
        .bind(agent_type)
        .bind(content)
        .bind(now)
        .fetch_optional(pool)
        .await?;

        if let Some(snippet) = existing {
            return Ok(snippet);
        }

        // 不存在则创建
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query_as::<_, CommonConfigSnippet>(
            r#"INSERT INTO common_config_snippets (id, agent_type, content, created_at, updated_at) VALUES ($1, $2, $3, $4, $5) RETURNING *"#,
        )
        .bind(&id)
        .bind(agent_type)
        .bind(content)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
    }

    /// 删除
    pub async fn delete(pool: &SqlitePool, agent_type: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM common_config_snippets WHERE agent_type = $1"#)
            .bind(agent_type)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}