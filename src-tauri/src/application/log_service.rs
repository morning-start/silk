use std::collections::HashMap;

use serde::Serialize;

use crate::error::{require_db, ServiceError};
use crate::models::RequestLog;
use crate::persistence::{LogRepo, StatsRepo};
use crate::AppState;

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct LogResponse {
    pub id: String,
    pub request_id: String,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub route_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub response_status: Option<i64>,
    pub duration_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub model_used: Option<String>,
    pub retry_count: i64,
    pub stream_enabled: bool,
    pub cache_hit: bool,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost: Option<f64>,
    pub auth_key_name: Option<String>,
}

impl LogResponse {
    pub fn from_log(log: RequestLog, cache: &HashMap<String, String>) -> Self {
        let provider_name = log
            .provider_id
            .as_ref()
            .and_then(|id| cache.get(id))
            .cloned();
        let mut resp = Self::from(log);
        resp.provider_name = provider_name;
        resp
    }
}

impl From<RequestLog> for LogResponse {
    fn from(l: RequestLog) -> Self {
        Self {
            id: l.id,
            request_id: l.request_id,
            timestamp: l.timestamp.to_string(),
            method: l.method,
            path: l.path,
            route_id: l.route_id,
            inbound_protocol: l.inbound_protocol,
            outbound_protocol: l.outbound_protocol,
            response_status: l.status_code,
            duration_ms: l.duration_ms,
            provider_id: l.provider_id,
            provider_name: None,
            error_message: l.error_message,
            error_code: l.error_code,
            model_used: l.model_used,
            retry_count: l.retry_count,
            stream_enabled: l.stream_enabled != 0,
            cache_hit: l.cache_hit != 0,
            request_size_bytes: l.request_size_bytes,
            response_size_bytes: l.response_size_bytes,
            tokens_input: l.tokens_input,
            tokens_output: l.tokens_output,
            cost: l.cost,
            auth_key_name: l.auth_key_name,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ListLogsResponse {
    pub logs: Vec<LogResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExportLogsResponse {
    pub file_path: String,
    pub exported_count: u64,
}

// ---------------------------------------------------------------------------
// Service Functions
// ---------------------------------------------------------------------------

/// 分页查询日志
pub async fn list(
    state: &AppState,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<ListLogsResponse, ServiceError> {
    let pool = require_db()?;
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let offset = offset.unwrap_or(0);
    let cache = state.provider_name_cache.read().await;

    let logs = LogRepo::find_paginated(pool, limit, offset).await?;
    let total = LogRepo::count(pool).await?;

    Ok(ListLogsResponse {
        logs: logs.into_iter().map(|l| LogResponse::from_log(l, &cache)).collect(),
        total,
        limit,
        offset,
    })
}

/// 清理指定天数之前的日志
pub async fn cleanup(before_days: i64) -> Result<u64, ServiceError> {
    let pool = require_db()?;
    let before = chrono::Utc::now().naive_utc() - chrono::Duration::days(before_days);
    LogRepo::delete_before(pool, before).await.map_err(ServiceError::from)
}

/// 清空所有日志
pub async fn clear_all() -> Result<u64, ServiceError> {
    let pool = require_db()?;
    LogRepo::delete_all(pool).await.map_err(ServiceError::from)
}

/// CSV 导出日志
pub async fn export_csv(
    provider_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    file_path: Option<String>,
) -> Result<ExportLogsResponse, ServiceError> {
    let pool = require_db()?;
    let limit = limit.unwrap_or(10000).clamp(1, 50000);
    let offset = offset.unwrap_or(0);

    let logs = if let Some(ref provider_id) = provider_id {
        LogRepo::find_by_provider(pool, provider_id, limit).await?
    } else {
        LogRepo::find_paginated(pool, limit, offset).await?
    };

    let mut csv_content = String::new();
    csv_content.push_str("id,request_id,timestamp,method,path,status_code,duration_ms,provider_id,model_used,tokens_input,tokens_output,error_message\n");

    // CSV 字段转义：含逗号/换行/引号的字段用双引号包裹
    fn csv_escape(field: &str) -> String {
        if field.contains(',') || field.contains('\n') || field.contains('"') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }

    for log in &logs {
        csv_content.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&log.id),
            csv_escape(&log.request_id),
            csv_escape(&log.timestamp.to_string()),
            csv_escape(&log.method),
            csv_escape(&log.path),
            log.status_code.unwrap_or(0),
            log.duration_ms.unwrap_or(0),
            csv_escape(log.provider_id.as_deref().unwrap_or("")),
            csv_escape(log.model_used.as_deref().unwrap_or("")),
            log.tokens_input.unwrap_or(0),
            log.tokens_output.unwrap_or(0),
            csv_escape(log.error_message.as_deref().unwrap_or("")),
        ));
    }

    let file_path = file_path.unwrap_or_else(|| {
        format!("silk_logs_{}.csv", chrono::Utc::now().format("%Y%m%d_%H%M%S"))
    });

    // 路径安全校验：使用跨平台 is_absolute() 防止路径遍历
    if file_path.contains("..") || std::path::Path::new(&file_path).is_absolute() {
        return Err(ServiceError::BadRequest {
            message: "文件路径不安全: 不允许包含 .. 或使用绝对路径".to_string(),
            code: None,
        });
    }

    tokio::fs::write(&file_path, &csv_content)
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("写入 CSV 文件失败: {e}"),
            detail: None,
        })?;

    Ok(ExportLogsResponse {
        file_path,
        exported_count: logs.len() as u64,
    })
}

/// 获取最近 N 条请求（用于仪表盘）
pub async fn recent_requests(limit: Option<i64>) -> Result<Vec<LogResponse>, ServiceError> {
    let pool = require_db()?;
    let limit = limit.unwrap_or(20);
    let logs = StatsRepo::recent_requests(pool, limit).await?;
    Ok(logs.into_iter().map(LogResponse::from).collect())
}