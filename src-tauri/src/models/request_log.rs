use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub request_id: String,
    pub timestamp: chrono::NaiveDateTime,
    pub method: String,
    pub path: String,
    pub route_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub request_headers: Option<String>,
    pub request_body: Option<String>,
    pub response_status: Option<i64>,
    pub status_code: Option<i64>,
    pub response_headers: Option<String>,
    pub response_body: Option<String>,
    pub duration_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub model_used: Option<String>,
    pub retry_count: i64,
    pub stream_enabled: i64,
    pub cache_hit: i64,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
}

/// 用于写入日志的输入结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRequestLog {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub route_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub request_headers: Option<String>,
    pub request_body: Option<String>,
    pub response_status: Option<i64>,
    pub status_code: Option<i64>,
    pub response_headers: Option<String>,
    pub response_body: Option<String>,
    pub duration_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub model_used: Option<String>,
    pub retry_count: Option<i64>,
    pub stream_enabled: Option<bool>,
    pub cache_hit: Option<bool>,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
}

impl RequestLog {
    /// 请求是否成功（2xx 状态码）
    pub fn is_success(&self) -> bool {
        self.status_code
            .or(self.response_status)
            .map(|s| s >= 200 && s < 300)
            .unwrap_or(false)
    }

    /// 获取耗时（毫秒）
    pub fn duration_ms(&self) -> Option<i64> {
        self.duration_ms
    }

    /// 是否为流式请求
    pub fn is_streaming(&self) -> bool {
        self.stream_enabled != 0
    }

    /// 是否命中缓存
    pub fn is_cache_hit(&self) -> bool {
        self.cache_hit != 0
    }
}
