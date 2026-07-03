use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 请求日志 — 主表（基础信息）
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
    pub status_code: Option<i64>,
    /// 响应时间（毫秒）
    pub resp_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    /// 实际使用的模型 ID（如 gpt-4o）
    pub model_id: Option<String>,
    /// 模型池名称（用户自定义）
    pub model_name: Option<String>,
    pub retry_count: i64,
    pub stream_enabled: i64,
    /// 认证使用的 Gateway Key 名称
    pub auth_key_name: Option<String>,
    /// 使用的渠道 Key 名称（Provider 下选中的 Key）
    pub channel_key_name: Option<String>,
}

/// 用于写入日志的输入结构（内部传输，包含主表和扩展表所有字段）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewRequestLog {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub route_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub status_code: Option<i64>,
    /// 响应时间（毫秒）
    pub resp_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    /// 实际使用的模型 ID
    pub model_id: Option<String>,
    /// 模型池名称
    pub model_name: Option<String>,
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
    /// 使用的渠道 Key 名称（Provider 下选中的 Key）
    pub channel_key_name: Option<String>,
}

/// 请求日志 Token 扩展信息（迁出字段：缓存、大小、Token、费用）
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RequestLogExtraToken {
    pub id: String,
    pub request_id: String,
    pub cache_hit: i64,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost: Option<f64>,
}

/// 用于写入 Token 扩展日志的输入结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewRequestLogExtraToken {
    pub request_id: String,
    pub cache_hit: Option<bool>,
    pub request_size_bytes: Option<i64>,
    pub response_size_bytes: Option<i64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost: Option<f64>,
}
