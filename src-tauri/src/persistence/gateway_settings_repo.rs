use std::path::Path;

use crate::models::{GatewaySettings, UpdateGatewaySettings};

/// 网关设置持久化（JSON 文件）仓库
///
/// 替代了旧版 DB 表存储。配置文件路径为 `data_dir/gateway.json`。
pub struct GatewaySettingsRepo;

impl GatewaySettingsRepo {
    /// 读取当前全局网关设置；如果文件不存在，创建默认设置后返回
    pub fn load_effective(path: &Path) -> GatewaySettings {
        GatewaySettings::load(path).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "加载网关设置失败，使用默认值");
            GatewaySettings::default()
        })
    }

    /// 更新全局网关设置
    pub fn update(path: &Path, update: &UpdateGatewaySettings) -> Result<GatewaySettings, String> {
        let mut settings = Self::load_effective(path);

        if let Some(v) = &update.bind_host {
            settings.bind_host = v.clone();
        }
        if let Some(v) = update.bind_port {
            settings.bind_port = v;
        }
        if let Some(v) = update.allow_remote {
            settings.allow_remote = v;
        }
        if let Some(v) = update.log_retention_days {
            settings.log_retention_days = v;
        }
        if let Some(v) = update.launch_at_startup {
            settings.launch_at_startup = v;
        }
        if let Some(v) = update.minimize_to_tray {
            settings.minimize_to_tray = v;
        }
        if let Some(v) = update.close_to_tray {
            settings.close_to_tray = v;
        }
        if let Some(v) = update.auto_start_gateway {
            settings.auto_start_gateway = v;
        }
        if let Some(v) = &update.default_provider_id {
            settings.default_provider_id = Some(v.clone());
        }
        if let Some(ref v) = update.default_route_id {
            settings.default_route_id = Some(v.clone());
        }
        if let Some(v) = update.rate_limit_enabled {
            settings.rate_limit_enabled = v;
        }
        if let Some(v) = update.rate_limit_max_requests_per_minute {
            settings.rate_limit_max_requests_per_minute = v;
        }
        if let Some(v) = update.rate_limit_max_tokens_per_minute {
            settings.rate_limit_max_tokens_per_minute = v;
        }

        settings.save(path)?;
        Ok(settings)
    }
}
