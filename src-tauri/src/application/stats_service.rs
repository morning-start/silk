use serde::Serialize;
use sqlx::SqlitePool;

use crate::persistence::StatsRepo;

// ---------------------------------------------------------------------------
// 统计响应类型
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ProviderStatsResponse {
    pub provider_name: Option<String>,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}

impl From<crate::persistence::stats_repo::ProviderStats> for ProviderStatsResponse {
    fn from(s: crate::persistence::stats_repo::ProviderStats) -> Self {
        Self {
            provider_name: s.provider_name,
            request_count: s.request_count,
            avg_duration_ms: s.avg_duration_ms,
            total_tokens: s.total_tokens,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct HourlyStatsResponse {
    pub hour: String,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}

impl From<crate::persistence::stats_repo::HourlyStats> for HourlyStatsResponse {
    fn from(s: crate::persistence::stats_repo::HourlyStats) -> Self {
        Self {
            hour: s.hour,
            request_count: s.request_count,
            avg_duration_ms: s.avg_duration_ms,
            total_tokens: s.total_tokens,
        }
    }
}

// ---------------------------------------------------------------------------
// 统计查询函数
// ---------------------------------------------------------------------------

/// 获取按 Provider 分组的请求统计
pub async fn stats_by_provider(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ProviderStatsResponse>, sqlx::Error> {
    let stats = StatsRepo::stats_by_provider(pool, limit).await?;
    Ok(stats.into_iter().map(ProviderStatsResponse::from).collect())
}

/// 获取按小时分组的时序统计
pub async fn hourly_stats(
    pool: &SqlitePool,
    hours: i64,
) -> Result<Vec<HourlyStatsResponse>, sqlx::Error> {
    let stats = StatsRepo::hourly_stats(pool, hours).await?;
    Ok(stats.into_iter().map(HourlyStatsResponse::from).collect())
}
