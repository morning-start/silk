use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{GatewayKey, MappingChannelInfo, ModelMapping, RequestLog};

// ---------------------------------------------------------------------------
// Log Response Types
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

// ---------------------------------------------------------------------------
// Stats Response Types
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
// Gateway Key Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GatewayKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub max_concurrent: i64,
    pub is_expired: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GatewayKey> for GatewayKeyResponse {
    fn from(k: GatewayKey) -> Self {
        let is_expired = k.is_expired();
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            enabled: k.enabled != 0,
            expires_at: k.expires_at.map(|d| d.to_string()),
            max_concurrent: k.max_concurrent,
            is_expired,
            created_at: k.created_at.to_string(),
            updated_at: k.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateGatewayKeyResponse {
    pub key: GatewayKeyResponse,
    pub plain_key: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGatewayKeyPayload {
    pub name: String,
    pub key_value: String,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGatewayKeyPayload {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

// ---------------------------------------------------------------------------
// Model Mapping Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ModelMappingResponse {
    pub id: String,
    pub model_name: String,
    pub strategy: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Vec<String>,
    pub description: String,
    pub enabled: bool,
    pub channels: Vec<MappingChannelInfo>,
    pub created_at: String,
    pub updated_at: String,
}

impl ModelMappingResponse {
    pub fn from_model(m: ModelMapping, channels: Vec<MappingChannelInfo>) -> Self {
        let capabilities = m.capabilities_vec();
        Self {
            id: m.id,
            model_name: m.model_name,
            strategy: m.strategy,
            max_input_tokens: m.max_input_tokens,
            max_context_tokens: m.max_context_tokens,
            max_output_tokens: m.max_output_tokens,
            input_price_per_1m: m.input_price_per_1m,
            output_price_per_1m: m.output_price_per_1m,
            capabilities,
            description: m.description,
            enabled: m.enabled != 0,
            channels,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateModelMappingPayload {
    pub model_name: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    pub channels: Option<Vec<crate::models::NewMappingChannel>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelMappingPayload {
    pub model_name: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    pub channels: Option<Vec<crate::models::NewMappingChannel>>,
}