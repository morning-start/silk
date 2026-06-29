use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct GatewaySettings {
    pub id: String,
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: i64,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: i64,
    pub rate_limit_max_requests_per_minute: i64,
    pub rate_limit_max_tokens_per_minute: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGatewaySettings {
    pub bind_host: Option<String>,
    pub bind_port: Option<i64>,
    pub allow_remote: Option<bool>,
    pub log_retention_days: Option<i64>,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: Option<bool>,
    pub rate_limit_max_requests_per_minute: Option<i64>,
    pub rate_limit_max_tokens_per_minute: Option<i64>,
}

impl Default for UpdateGatewaySettings {
    fn default() -> Self {
        Self {
            bind_host: None,
            bind_port: None,
            allow_remote: None,
            log_retention_days: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }
    }
}
