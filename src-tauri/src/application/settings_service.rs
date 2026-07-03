use serde::{Deserialize, Serialize};

use crate::error::ServiceError;
use crate::models::GatewaySettings;
use crate::persistence::GatewaySettingsRepo;
use crate::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct GatewaySettingsResponse {
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_max_requests_per_minute: i64,
    pub rate_limit_max_tokens_per_minute: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsPayload {
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

pub fn get() -> Result<GatewaySettingsResponse, ServiceError> {
    // 同步读取 JSON 文件，不阻塞
    let path = crate::get_settings_path()
        .ok_or_else(|| ServiceError::Internal {
            message: "网关设置路径未初始化".to_string(),
            detail: None,
        })?;
    let settings = GatewaySettingsRepo::load_effective(path);

    Ok(GatewaySettingsResponse::from(settings))
}

pub async fn update(
    state: &AppState,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, ServiceError> {
    let path = crate::get_settings_path()
        .ok_or_else(|| ServiceError::Internal {
            message: "网关设置路径未初始化".to_string(),
            detail: None,
        })?;

    let previous_settings = {
        let gateway_guard = state.gateway.read().await;
        let settings_guard = gateway_guard.settings.read().await;
        settings_guard.clone()
    };

    let update = crate::models::UpdateGatewaySettings {
        bind_host: payload.bind_host,
        bind_port: payload.bind_port,
        allow_remote: payload.allow_remote,
        log_retention_days: payload.log_retention_days,
        default_provider_id: payload.default_provider_id,
        default_route_id: payload.default_route_id,
        rate_limit_enabled: payload.rate_limit_enabled,
        rate_limit_max_requests_per_minute: payload.rate_limit_max_requests_per_minute,
        rate_limit_max_tokens_per_minute: payload.rate_limit_max_tokens_per_minute,
    };

    let settings = GatewaySettingsRepo::update(path, &update)
        .map_err(|e| ServiceError::Internal {
            message: format!("保存设置失败: {e}"),
            detail: None,
        })?;

    {
        let gateway_guard = state.gateway.read().await;
        let mut current = gateway_guard.settings.write().await;
        *current = settings.clone();
    }

    if runtime_settings_changed(&previous_settings, &settings) {
        // 通过 broadcast channel 通知配置变更，
        // 由 lib.rs 中的监听任务处理网关重启，避免 application 层内部循环依赖
        let _ = state.settings_change_tx.send(());
    }

    Ok(GatewaySettingsResponse::from(settings))
}

impl From<GatewaySettings> for GatewaySettingsResponse {
    fn from(s: GatewaySettings) -> Self {
        Self {
            bind_host: s.bind_host,
            bind_port: s.bind_port,
            allow_remote: s.allow_remote,
            log_retention_days: s.log_retention_days,
            default_provider_id: s.default_provider_id,
            default_route_id: s.default_route_id,
            rate_limit_enabled: s.rate_limit_enabled,
            rate_limit_max_requests_per_minute: s.rate_limit_max_requests_per_minute,
            rate_limit_max_tokens_per_minute: s.rate_limit_max_tokens_per_minute,
        }
    }
}

fn runtime_settings_changed(before: &GatewaySettings, after: &GatewaySettings) -> bool {
    before.bind_host != after.bind_host
        || before.bind_port != after.bind_port
}
