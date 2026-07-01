use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// 子配置类型（逻辑分组）
// ---------------------------------------------------------------------------

/// 网络相关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_requests_per_minute: i64,
    pub max_tokens_per_minute: i64,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub retention_days: i64,
}

// ---------------------------------------------------------------------------
// 主设置结构（DB 映射）
// ---------------------------------------------------------------------------

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

impl GatewaySettings {
    /// 获取网络配置子集
    pub fn network_config(&self) -> NetworkConfig {
        NetworkConfig {
            bind_host: self.bind_host.clone(),
            bind_port: self.bind_port,
            allow_remote: self.allow_remote != 0,
        }
    }

    /// 获取速率限制配置子集
    pub fn rate_limit_config(&self) -> RateLimitConfig {
        RateLimitConfig {
            enabled: self.rate_limit_enabled != 0,
            max_requests_per_minute: self.rate_limit_max_requests_per_minute,
            max_tokens_per_minute: self.rate_limit_max_tokens_per_minute,
        }
    }

    /// 获取日志配置子集
    pub fn log_config(&self) -> LogConfig {
        LogConfig {
            retention_days: self.log_retention_days,
        }
    }
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
