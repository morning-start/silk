use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

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
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub auto_start_gateway: bool,
    pub default_provider_id: Option<String>,
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
    pub launch_at_startup: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub auto_start_gateway: Option<bool>,
    pub default_provider_id: Option<String>,
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
    app_handle: &AppHandle,
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
        launch_at_startup: payload.launch_at_startup,
        minimize_to_tray: payload.minimize_to_tray,
        close_to_tray: payload.close_to_tray,
        auto_start_gateway: payload.auto_start_gateway,
        default_provider_id: payload.default_provider_id,
        rate_limit_enabled: payload.rate_limit_enabled,
        rate_limit_max_requests_per_minute: payload.rate_limit_max_requests_per_minute,
        rate_limit_max_tokens_per_minute: payload.rate_limit_max_tokens_per_minute,
    };

    let settings =
        GatewaySettingsRepo::update(path, &update).map_err(|e| ServiceError::Internal {
            message: format!("保存设置失败: {e}"),
            detail: None,
        })?;

    if previous_settings.launch_at_startup != settings.launch_at_startup {
        sync_autostart(app_handle, settings.launch_at_startup)?;
    }

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
            launch_at_startup: s.launch_at_startup,
            minimize_to_tray: s.minimize_to_tray,
            close_to_tray: s.close_to_tray,
            auto_start_gateway: s.auto_start_gateway,
            default_provider_id: s.default_provider_id,
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
        if port < 1024 {
            return bad_request("绑定端口不能小于 1024（特权端口），建议使用 1024-65535 之间的端口");
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

pub fn sync_autostart(app_handle: &AppHandle, enabled: bool) -> Result<(), ServiceError> {
    let autostart = app_handle.autolaunch();
    let current = autostart.is_enabled().map_err(|e| ServiceError::Internal {
        message: "读取系统自启动状态失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    if current == enabled {
        return Ok(());
    }

    if enabled {
        autostart.enable().map_err(|e| ServiceError::Internal {
            message: "启用系统自启动失败".to_string(),
            detail: Some(e.to_string()),
        })?;
    } else {
        autostart.disable().map_err(|e| ServiceError::Internal {
            message: "关闭系统自启动失败".to_string(),
            detail: Some(e.to_string()),
        })?;
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
            launch_at_startup: None,
            minimize_to_tray: None,
            close_to_tray: None,
            auto_start_gateway: None,
            default_provider_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }));

        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: None,
            bind_port: Some(0),
            allow_remote: None,
            log_retention_days: None,
            launch_at_startup: None,
            minimize_to_tray: None,
            close_to_tray: None,
            auto_start_gateway: None,
            default_provider_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }));

        // 特权端口也应拒绝
        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: None,
            bind_port: Some(1023),
            allow_remote: None,
            log_retention_days: None,
            launch_at_startup: None,
            minimize_to_tray: None,
            close_to_tray: None,
            auto_start_gateway: None,
            default_provider_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        }));

        assert_bad_request(validate_update_payload(&UpdateSettingsPayload {
            bind_host: None,
            bind_port: None,
            allow_remote: None,
            log_retention_days: None,
            launch_at_startup: None,
            minimize_to_tray: None,
            close_to_tray: None,
            auto_start_gateway: None,
            default_provider_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: Some(-1),
            rate_limit_max_tokens_per_minute: None,
        }));
    }

    #[test]
    fn validate_accepts_reasonable_settings() {
        validate_update_payload(&UpdateSettingsPayload {
            bind_host: Some("127.0.0.1".to_string()),
            bind_port: Some(1877),
            allow_remote: Some(false),
            log_retention_days: Some(30),
            launch_at_startup: Some(false),
            minimize_to_tray: Some(true),
            close_to_tray: Some(true),
            auto_start_gateway: Some(false),
            default_provider_id: None,
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
