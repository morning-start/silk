use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::RequestLog;
use crate::persistence::LogRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 分页获取日志
#[tauri::command]
pub async fn list_logs(
    _state: State<'_, AppState>,
    payload: ListLogsPayload,
) -> Result<ListLogsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let limit = payload.limit.unwrap_or(50).clamp(1, 500);
    let offset = payload.offset.unwrap_or(0);

    let logs = LogRepo::find_paginated(pool, limit, offset)
        .await
        .map_err(|e| format!("查询日志失败: {e}"))?;

    let total = LogRepo::count(pool)
        .await
        .map_err(|e| format!("查询日志总数失败: {e}"))?;

    Ok(ListLogsResponse {
        logs: logs.into_iter().map(LogResponse::from).collect(),
        total,
        limit,
        offset,
    })
}

/// 按 Provider ID 查询日志
#[tauri::command]
pub async fn logs_by_provider(
    _state: State<'_, AppState>,
    provider_id: String,
    limit: Option<i64>,
) -> Result<Vec<LogResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(50).clamp(1, 500);

    let logs = LogRepo::find_by_provider(pool, &provider_id, limit)
        .await
        .map_err(|e| format!("查询日志失败: {e}"))?;

    Ok(logs.into_iter().map(LogResponse::from).collect())
}

/// 按 request_id 查询日志（关联请求和响应）
#[tauri::command]
pub async fn logs_by_request_id(
    _state: State<'_, AppState>,
    request_id: String,
) -> Result<Vec<LogResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let logs = LogRepo::find_by_request_id(pool, &request_id)
        .await
        .map_err(|e| format!("查询日志失败: {e}"))?;

    Ok(logs.into_iter().map(LogResponse::from).collect())
}

/// 获取日志总数
#[tauri::command]
pub async fn count_logs(_state: State<'_, AppState>) -> Result<i64, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    LogRepo::count(pool)
        .await
        .map_err(|e| format!("查询日志总数失败: {e}"))
}

/// 清理指定时间之前的日志
#[tauri::command]
pub async fn cleanup_logs(
    _state: State<'_, AppState>,
    payload: CleanupLogsPayload,
) -> Result<u64, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let before = chrono::Utc::now().naive_utc()
        - chrono::Duration::days(payload.before_days);

    let deleted = LogRepo::delete_before(pool, before)
        .await
        .map_err(|e| format!("清理日志失败: {e}"))?;

    Ok(deleted)
}

/// 删除所有日志
#[tauri::command]
pub async fn clear_all_logs(_state: State<'_, AppState>) -> Result<u64, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let deleted = LogRepo::delete_all(pool)
        .await
        .map_err(|e| format!("清空日志失败: {e}"))?;

    Ok(deleted)
}

/// 导出日志为 CSV 文件
#[tauri::command]
pub async fn export_logs_csv(
    _state: State<'_, AppState>,
    payload: ExportLogsPayload,
) -> Result<ExportLogsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let limit = payload.limit.unwrap_or(10000);
    let offset = payload.offset.unwrap_or(0);

    let logs = if let Some(provider_id) = &payload.provider_id {
        LogRepo::find_by_provider(pool, provider_id, limit)
            .await
            .map_err(|e| format!("查询日志失败: {e}"))?
    } else {
        LogRepo::find_paginated(pool, limit, offset)
            .await
            .map_err(|e| format!("查询日志失败: {e}"))?
    };

    // 生成 CSV 内容
    let mut csv_content = String::new();
    csv_content.push_str("id,request_id,timestamp,method,path,status_code,duration_ms,provider_id,model_used,tokens_input,tokens_output,error_message\n");

    for log in &logs {
        let provider_id = log.provider_id.as_deref().unwrap_or("");
        let model_used = log.model_used.as_deref().unwrap_or("");
        let error_message = log.error_message.as_deref().unwrap_or("");

        csv_content.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            log.id,
            log.request_id,
            log.timestamp,
            log.method,
            log.path,
            log.response_status.unwrap_or(0),
            log.duration_ms.unwrap_or(0),
            provider_id,
            model_used,
            log.tokens_input.unwrap_or(0),
            log.tokens_output.unwrap_or(0),
            error_message,
        ));
    }

    // 写入文件
    let file_path = payload
        .file_path
        .unwrap_or_else(|| format!("silk_logs_{}.csv", chrono::Utc::now().format("%Y%m%d_%H%M%S")));

    tokio::fs::write(&file_path, &csv_content)
        .await
        .map_err(|e| format!("写入 CSV 文件失败: {e}"))?;

    Ok(ExportLogsResponse {
        file_path,
        exported_count: logs.len() as u64,
    })
}

// ---------------------------------------------------------------------------
// Types
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
    pub status_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub model_used: Option<String>,
    pub retry_count: i64,
    pub stream_enabled: bool,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
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
            response_status: l.response_status,
            status_code: l.status_code,
            duration_ms: l.duration_ms,
            provider_id: l.provider_id,
            error_message: l.error_message,
            model_used: l.model_used,
            retry_count: l.retry_count,
            stream_enabled: l.stream_enabled != 0,
            request_size_bytes: l.request_size_bytes,
            response_size_bytes: l.response_size_bytes,
            tokens_input: l.tokens_input,
            tokens_output: l.tokens_output,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListLogsPayload {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ListLogsResponse {
    pub logs: Vec<LogResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct CleanupLogsPayload {
    pub before_days: i64,
}

#[derive(Debug, Deserialize)]
pub struct ExportLogsPayload {
    pub provider_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExportLogsResponse {
    pub file_path: String,
    pub exported_count: u64,
}
