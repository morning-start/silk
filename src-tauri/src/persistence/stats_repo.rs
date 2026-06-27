use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::Row;
use sqlx::SqlitePool;

use crate::models::RequestLog;

/// 仪表盘统计数据聚合仓库
pub struct StatsRepo;

impl StatsRepo {
    /// 获取今日请求总数
    pub async fn today_request_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM request_logs
            WHERE date(timestamp) = date('now')
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 获取今日成功请求数（2xx 状态码）
    pub async fn today_success_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM request_logs
            WHERE date(timestamp) = date('now')
              AND status_code >= 200 AND status_code < 300
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 获取今日平均响应时间（ms）
    pub async fn today_avg_duration_ms(pool: &SqlitePool) -> Result<Option<f64>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT AVG(duration_ms) as avg_duration FROM request_logs
            WHERE date(timestamp) = date('now')
              AND duration_ms IS NOT NULL
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<Option<f64>, _>("avg_duration"))
    }

    /// 获取今日 Token 消耗总量
    pub async fn today_total_tokens(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(tokens_input + tokens_output), 0) as total FROM request_logs
            WHERE date(timestamp) = date('now')
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("total"))
    }

    /// 获取活跃 Provider 数量（今日有请求的）
    pub async fn today_active_providers(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT provider_id) as count FROM request_logs
            WHERE date(timestamp) = date('now')
              AND provider_id IS NOT NULL
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 获取总日志数
    pub async fn total_request_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(r#"SELECT COUNT(*) as count FROM request_logs"#)
            .fetch_one(pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 获取昨日请求数（用于对比）
    pub async fn yesterday_request_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM request_logs
            WHERE date(timestamp) = date('now', '-1 day')
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    /// 获取最近 N 条请求（用于仪表盘最近请求列表）
    pub async fn recent_requests(
        pool: &SqlitePool,
        limit: i64,
    ) -> Result<Vec<RequestLog>, sqlx::Error> {
        sqlx::query_as::<_, RequestLog>(
            r#"
            SELECT * FROM request_logs
            ORDER BY timestamp DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 获取按 Provider 分组的请求统计（用于服务商负载分布）
    pub async fn stats_by_provider(
        pool: &SqlitePool,
        limit: i64,
    ) -> Result<Vec<ProviderStats>, sqlx::Error> {
        sqlx::query_as::<_, ProviderStats>(
            r#"
            SELECT
                p.name as provider_name,
                COUNT(*) as request_count,
                COALESCE(AVG(r.duration_ms), 0.0) as avg_duration_ms,
                COALESCE(SUM(r.tokens_input + r.tokens_output), 0) as total_tokens
            FROM request_logs r
            LEFT JOIN providers p ON r.provider_id = p.id
            WHERE r.timestamp >= datetime('now', '-1 day')
            GROUP BY r.provider_id, p.name
            ORDER BY request_count DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 获取按小时分组的时序统计（用于图表）
    pub async fn hourly_stats(
        pool: &SqlitePool,
        hours: i64,
    ) -> Result<Vec<HourlyStats>, sqlx::Error> {
        sqlx::query_as::<_, HourlyStats>(
            r#"
            SELECT
                strftime('%Y-%m-%d %H:00:00', timestamp) as hour,
                COUNT(*) as request_count,
                COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms,
                COALESCE(SUM(tokens_input + tokens_output), 0) as total_tokens
            FROM request_logs
            WHERE timestamp >= datetime('now', '-' || $1 || ' hours')
            GROUP BY strftime('%Y-%m-%d %H', timestamp)
            ORDER BY hour ASC
            "#,
        )
        .bind(hours)
        .fetch_all(pool)
        .await
    }
}

/// 按 Provider 分组的统计
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProviderStats {
    pub provider_name: Option<String>,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}

/// 按小时分组的统计
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct HourlyStats {
    pub hour: String,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}
