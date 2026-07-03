use sqlx::SqlitePool;

use crate::models::{NewRequestLogExtraToken, RequestLogExtraToken};

/// 请求日志扩展信息仓库（cache_hit, request_size_bytes, response_size_bytes, tokens_input, tokens_output, cost）
pub struct LogExtraTokenRepo;

impl LogExtraTokenRepo {
    /// 批量插入扩展日志
    pub async fn insert_batch(
        pool: &SqlitePool,
        extras: &[NewRequestLogExtraToken],
    ) -> Result<u64, sqlx::Error> {
        if extras.is_empty() {
            return Ok(0);
        }

        let mut tx = pool.begin().await?;
        let mut count = 0u64;

        for extra in extras {
            let id = uuid::Uuid::new_v4().to_string();
            let cache_hit = if extra.cache_hit.unwrap_or(false) { 1 } else { 0 };

            let result = sqlx::query(
                r#"
                INSERT INTO request_log_extra_token (id, request_id, cache_hit, request_size_bytes,
                                                response_size_bytes, tokens_input, tokens_output, cost)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(id)
            .bind(extra.request_id.as_str())
            .bind(cache_hit)
            .bind(extra.request_size_bytes)
            .bind(extra.response_size_bytes)
            .bind(extra.tokens_input)
            .bind(extra.tokens_output)
            .bind(extra.cost)
            .execute(&mut *tx)
            .await?;
            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// 按 request_id 列表批量查询扩展信息
    pub async fn find_by_request_ids(
        pool: &SqlitePool,
        request_ids: &[String],
    ) -> Result<Vec<RequestLogExtraToken>, sqlx::Error> {
        if request_ids.is_empty() {
            return Ok(Vec::new());
        }

        // SQLite 不支持数组绑定，用 IN 配合参数占位
        let placeholders: Vec<String> = request_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();

        let query = format!(
            "SELECT * FROM request_log_extra_token WHERE request_id IN ({})",
            placeholders.join(", ")
        );

        let mut q = sqlx::query_as::<_, RequestLogExtraToken>(&query);
        for id in request_ids {
            q = q.bind(id);
        }
        q.fetch_all(pool).await
    }
}
