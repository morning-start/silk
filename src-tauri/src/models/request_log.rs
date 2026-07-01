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
    /// 本次请求费用（美元），非流式响应时计算
    pub cost: Option<f64>,
    /// 认证使用的 Gateway Key 名称
    pub auth_key_name: Option<String>,
}

/// 用于写入日志的输入结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewRequestLog {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub route_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub request_headers: Option<String>,
    pub request_body: Option<String>,
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
    /// 本次请求费用（美元），非流式响应时计算
    pub cost: Option<f64>,
    /// 认证使用的 Gateway Key 名称
    pub auth_key_name: Option<String>,
}


