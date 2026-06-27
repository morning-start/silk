use serde::Serialize;
use tauri::State;

use crate::persistence::StatsRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands - 仪表盘统计
// ---------------------------------------------------------------------------

/// 获取仪表盘概览统计
#[tauri::command]
pub async fn dashboard_stats(
    _state: State<'_, AppState>,
) -> Result<DashboardStatsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let today_requests = StatsRepo::today_request_count(pool)
        .await
        .map_err(|e| format!("查询今日请求数失败: {e}"))?;
    let today_success = StatsRepo::today_success_count(pool)
        .await
        .map_err(|e| format!("查询今日成功数失败: {e}"))?;
    let today_avg_duration = StatsRepo::today_avg_duration_ms(pool)
        .await
        .map_err(|e| format!("查询今日平均响应时间失败: {e}"))?;
    let today_tokens = StatsRepo::today_total_tokens(pool)
        .await
        .map_err(|e| format!("查询今日 Token 消耗失败: {e}"))?;
    let active_providers = StatsRepo::today_active_providers(pool)
        .await
        .map_err(|e| format!("查询活跃服务商数失败: {e}"))?;
    let total_requests = StatsRepo::total_request_count(pool)
        .await
        .map_err(|e| format!("查询总请求数失败: {e}"))?;
    let yesterday_requests = StatsRepo::yesterday_request_count(pool)
        .await
        .map_err(|e| format!("查询昨日请求数失败: {e}"))?;

    Ok(DashboardStatsResponse {
        today_requests,
        today_success,
        today_avg_duration_ms: today_avg_duration.unwrap_or(0.0),
        today_tokens,
        active_providers,
        total_requests,
        yesterday_requests,
    })
}

/// 获取最近请求列表（仪表盘用）
#[tauri::command]
pub async fn recent_requests(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<crate::models::RequestLog>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(20);

    StatsRepo::recent_requests(pool, limit)
        .await
        .map_err(|e| format!("查询最近请求失败: {e}"))
}

/// 获取按 Provider 分组的统计
#[tauri::command]
pub async fn stats_by_provider(
    _state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<ProviderStatsResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(10);

    let stats = StatsRepo::stats_by_provider(pool, limit)
        .await
        .map_err(|e| format!("查询服务商统计失败: {e}"))?;

    Ok(stats.into_iter().map(ProviderStatsResponse::from).collect())
}

/// 获取按小时分组的时序统计
#[tauri::command]
pub async fn hourly_stats(
    _state: State<'_, AppState>,
    hours: Option<i64>,
) -> Result<Vec<HourlyStatsResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let hours = hours.unwrap_or(24);

    let stats = StatsRepo::hourly_stats(pool, hours)
        .await
        .map_err(|e| format!("查询时序统计失败: {e}"))?;

    Ok(stats.into_iter().map(HourlyStatsResponse::from).collect())
}

// ---------------------------------------------------------------------------
// Types
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
    /// 今日成功率（百分比）
    pub fn success_rate(&self) -> f64 {
        if self.today_requests == 0 {
            0.0
        } else {
            (self.today_success as f64 / self.today_requests as f64) * 100.0
        }
    }

    /// 较昨日增长百分比
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
