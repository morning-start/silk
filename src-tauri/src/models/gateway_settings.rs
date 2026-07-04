use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::persistence::defaults;

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

/// 桌面行为配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopBehaviorConfig {
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub auto_start_gateway: bool,
}

// ---------------------------------------------------------------------------
// 主设置结构（JSON 配置映射）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySettings {
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub log_retention_days: i64,
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub auto_start_gateway: bool,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_max_requests_per_minute: i64,
    pub rate_limit_max_tokens_per_minute: i64,
}

impl Default for GatewaySettings {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            bind_port: defaults::DEFAULT_BIND_PORT,
            allow_remote: false,
            log_retention_days: defaults::DEFAULT_LOG_RETENTION_DAYS,
            launch_at_startup: false,
            minimize_to_tray: true,
            close_to_tray: true,
            auto_start_gateway: false,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: false,
            rate_limit_max_requests_per_minute: defaults::DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            rate_limit_max_tokens_per_minute: defaults::DEFAULT_RATE_LIMIT_MAX_TOKENS,
        }
    }
}

impl GatewaySettings {
    /// 从 JSON 文件加载网关设置；文件不存在时返回默认值
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            let settings = Self::default();
            settings.save(path)?;
            return Ok(settings);
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("读取设置文件失败: {e}"))?;
        let mut settings = serde_json::from_str::<Self>(&content)
            .map_err(|e| format!("解析设置文件失败: {e}"))?;

        // 自动迁移：端口 2013 在部分 Windows 系统上被排除范围占用（Hyper-V/Docker），
        // 静默迁移到新默认端口 9876
        if settings.bind_port == 2013 {
            tracing::info!(old_port = 2013, new_port = defaults::DEFAULT_BIND_PORT, "自动迁移网关端口");
            settings.bind_port = defaults::DEFAULT_BIND_PORT;
            settings.save(path)?;
        }

        Ok(settings)
    }

    /// 保存网关设置到 JSON 文件
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建设置目录失败: {e}"))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化设置失败: {e}"))?;
        std::fs::write(path, content)
            .map_err(|e| format!("写入设置文件失败: {e}"))
    }

    /// 获取网络配置子集
    pub fn network_config(&self) -> NetworkConfig {
        NetworkConfig {
            bind_host: self.bind_host.clone(),
            bind_port: self.bind_port,
            allow_remote: self.allow_remote,
        }
    }

    /// 获取速率限制配置子集
    pub fn rate_limit_config(&self) -> RateLimitConfig {
        RateLimitConfig {
            enabled: self.rate_limit_enabled,
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

    /// 获取桌面行为配置子集
    pub fn desktop_behavior_config(&self) -> DesktopBehaviorConfig {
        DesktopBehaviorConfig {
            launch_at_startup: self.launch_at_startup,
            minimize_to_tray: self.minimize_to_tray,
            close_to_tray: self.close_to_tray,
            auto_start_gateway: self.auto_start_gateway,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGatewaySettings {
    pub bind_host: Option<String>,
    pub bind_port: Option<i64>,
    pub allow_remote: Option<bool>,
    pub log_retention_days: Option<i64>,
    pub launch_at_startup: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub auto_start_gateway: Option<bool>,
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
            launch_at_startup: None,
            minimize_to_tray: None,
            close_to_tray: None,
            auto_start_gateway: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }
    }
}
