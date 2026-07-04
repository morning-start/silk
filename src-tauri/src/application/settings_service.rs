use serde::{Deserialize, Serialize};

use crate::error::{bad_request, ServiceError};
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
    let path = crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
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
    validate_update_payload(&payload)?;

    let path = crate::get_settings_path().ok_or_else(|| ServiceError::Internal {
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

    let settings =
        GatewaySettingsRepo::update(path, &update).map_err(|e| ServiceError::Internal {
            message: format!("保存设置失败: {e}"),
            detail: None,
        })?;

    {
        let gateway_guard = state.gateway.read().await;
        let mut current = gateway_guard.settings.write().await;
        *current = settings.clone();

        // 热更新限流配置（不影响已有计数器）
        gateway_guard
            .rate_limit_state
            .update_config(
                settings.rate_limit_enabled,
                settings.rate_limit_max_requests_per_minute as u64,
                settings.rate_limit_max_tokens_per_minute as u64,
            )
            .await;
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
    before.bind_host != after.bind_host || before.bind_port != after.bind_port
}

fn validate_update_payload(payload: &UpdateSettingsPayload) -> Result<(), ServiceError> {
    if let Some(host) = &payload.bind_host {
        if host.trim().is_empty() {
            return bad_request("绑定地址不能为空");
        }
    }
    if let Some(port) = payload.bind_port {
        if !(1..=65535).contains(&port) {
            return bad_request("绑定端口必须在 1-65535 之间");
        }
    }
    if let Some(days) = payload.log_retention_days {
        if !(1..=3650).contains(&days) {
            return bad_request("日志保留天数必须在 1-3650 之间");
        }
    }
    if let Some(max) = payload.rate_limit_max_requests_per_minute {
        if max < 1 {
            return bad_request("每分钟请求数限制必须大于 0");
        }
    }
    if let Some(max) = payload.rate_limit_max_tokens_per_minute {
        if max < 1 {
            return bad_request("每分钟 Token 限制必须大于 0");
        }
    }
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_invalid_runtime_settings() {
        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: Some(" ".to_string()),
            bind_port: None,
            allow_remote: None,
            log_retention_days: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }));

        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: None,
            bind_port: Some(0),
            allow_remote: None,
            log_retention_days: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }));

        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: None,
            bind_port: None,
            allow_remote: None,
            log_retention_days: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: Some(-1),
            rate_limit_max_tokens_per_minute: None,
        }));
    }

    #[test]
    fn validate_accepts_reasonable_settings() {
        validate_update_payload(&UpdateSettingsPayload {
            bind_host: Some("127.0.0.1".to_string()),
            bind_port: Some(2013),
            allow_remote: Some(false),
            log_retention_days: Some(30),
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: Some(true),
            rate_limit_max_requests_per_minute: Some(1000),
            rate_limit_max_tokens_per_minute: Some(500000),
        })
        .expect("valid settings");
    }

    fn assert_bad_request(result: Result<(), ServiceError>) {
        assert!(matches!(result, Err(ServiceError::BadRequest { .. })));
    }
}
