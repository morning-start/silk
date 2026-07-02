use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{require_db, ServiceError};
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

// ---------------------------------------------------------------------------
// Dashboard Stats Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct DashboardStatsResponse {
    pub today_requests: i64,
    pub today_success: i64,
    pub today_avg_duration_ms: f64,
    pub today_tokens: i64,
    pub active_providers: i64,
    pub total_requests: i64,
    pub yesterday_requests: i64,
}

impl DashboardStatsResponse {
    pub fn success_rate(&self) -> f64 {
        if self.today_requests == 0 {
            0.0
        } else {
            (self.today_success as f64 / self.today_requests as f64) * 100.0
        }
    }

    pub fn growth_rate(&self) -> f64 {
        if self.yesterday_requests == 0 {
            0.0
        } else {
            ((self.today_requests - self.yesterday_requests) as f64
                / self.yesterday_requests as f64)
                * 100.0
        }
    }
}

// ---------------------------------------------------------------------------
// Dashboard Service Functions
// ---------------------------------------------------------------------------

/// 获取仪表盘聚合统计数据
pub async fn dashboard_stats() -> Result<DashboardStatsResponse, ServiceError> {
    let pool = require_db()?;
    let agg = StatsRepo::dashboard_aggregate(pool).await?;

    Ok(DashboardStatsResponse {
        today_requests: agg.today_requests,
        today_success: agg.today_success,
        today_avg_duration_ms: agg.today_avg_duration_ms.unwrap_or(0.0),
        today_tokens: agg.today_tokens,
        active_providers: agg.active_providers,
        total_requests: agg.total_requests,
        yesterday_requests: agg.yesterday_requests,
    })
}
