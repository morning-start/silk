use serde::Deserialize;

// ---------------------------------------------------------------------------
// Log Payload Types (input-only, consumed by commands)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListLogsPayload {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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

