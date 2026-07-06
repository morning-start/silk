use sqlx::Row;
use sqlx::SqlitePool;

use crate::models::{NewRequestLog, RequestLog};
use crate::persistence::defaults;

pub struct LogRepo;

impl LogRepo {
    /// 插入单条日志
    pub async fn insert(pool: &SqlitePool, log: &NewRequestLog) -> Result<RequestLog, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let retry_count = log.retry_count.unwrap_or(0);
        let stream_enabled = defaults::bool_to_i64(log.stream_enabled, false);

        sqlx::query_as::<_, RequestLog>(
            r#"
            INSERT INTO request_logs (id, request_id, method, path, inbound_protocol, outbound_protocol,
                                      status_code, resp_ms, total_duration_ms, provider_id,
                                      error_message, error_code, model_id, model_name, retry_count, stream_enabled,
                                      auth_key_name, channel_key_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15, $16, $17, $18)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(log.request_id.as_str())
        .bind(log.method.as_str())
        .bind(log.path.as_str())
        .bind(log.inbound_protocol.as_deref())
        .bind(log.outbound_protocol.as_deref())
        .bind(log.status_code)
        .bind(log.resp_ms)
        .bind(log.total_duration_ms)
        .bind(log.provider_id.as_deref())
        .bind(log.error_message.as_deref())
        .bind(log.error_code.as_deref())
        .bind(log.model_id.as_deref())
        .bind(log.model_name.as_deref())
        .bind(retry_count)
        .bind(stream_enabled)
        .bind(log.auth_key_name.as_deref())
        .bind(log.channel_key_name.as_deref())
        .fetch_one(pool)
        .await
    }

    /// 批量插入日志（用于异步批量写入场景）
    pub async fn insert_batch(
        pool: &SqlitePool,
        logs: &[NewRequestLog],
    ) -> Result<u64, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let mut count = 0u64;

        for log in logs {
            let id = uuid::Uuid::new_v4().to_string();
            let retry_count = log.retry_count.unwrap_or(0);
            let stream_enabled = defaults::bool_to_i64(log.stream_enabled, false);
            let result = sqlx::query(
                r#"
                INSERT INTO request_logs (id, request_id, method, path, inbound_protocol, outbound_protocol,
                                          status_code, resp_ms, total_duration_ms, provider_id,
                                          error_message, error_code, model_id, model_name, retry_count, stream_enabled,
                                          auth_key_name, channel_key_name)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                            $12, $13, $14, $15, $16, $17, $18)
                "#,
            )
            .bind(id)
            .bind(log.request_id.as_str())
            .bind(log.method.as_str())
            .bind(log.path.as_str())
            .bind(log.inbound_protocol.as_deref())
            .bind(log.outbound_protocol.as_deref())
            .bind(log.status_code)
            .bind(log.resp_ms)
            .bind(log.total_duration_ms)
            .bind(log.provider_id.as_deref())
            .bind(log.error_message.as_deref())
            .bind(log.error_code.as_deref())
            .bind(log.model_id.as_deref())
            .bind(log.model_name.as_deref())
            .bind(retry_count)
            .bind(stream_enabled)
            .bind(log.auth_key_name.as_deref())
            .bind(log.channel_key_name.as_deref())
            .execute(&mut *tx)
            .await?;
            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// 分页查询日志（按时间倒序）
    pub async fn find_paginated(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RequestLog>, sqlx::Error> {
        let limit = limit.clamp(1, defaults::PAGINATION_MAX_LIMIT);
        sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT * FROM request_logs
            ORDER BY timestamp DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// 按 Provider ID 查询日志
    pub async fn find_by_provider(
        pool: &SqlitePool,
        provider_id: &str,
        limit: i64,
    ) -> Result<Vec<RequestLog>, sqlx::Error> {
        let limit = limit.clamp(1, defaults::PAGINATION_MAX_LIMIT);
        sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT * FROM request_logs
            WHERE provider_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(provider_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 按 request_id 查询（关联请求和响应日志）
    pub async fn find_by_request_id(
        pool: &SqlitePool,
        request_id: &str,
    ) -> Result<Vec<RequestLog>, sqlx::Error> {
        sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT * FROM request_logs
            WHERE request_id = $1
            ORDER BY timestamp ASC
            "#,
        )
        .bind(request_id)
        .fetch_all(pool)
        .await
    }

    /// 查询日志总数
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(r#"SELECT COUNT(*) as count FROM request_logs"#)
            .fetch_one(pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 删除指定时间之前的日志（用于定期清理）
    pub async fn delete_before(
        pool: &SqlitePool,
        before: chrono::NaiveDateTime,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM request_logs WHERE timestamp < $1"#)
            .bind(before)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// 删除所有日志
    pub async fn delete_all(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM request_logs"#)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }
}
