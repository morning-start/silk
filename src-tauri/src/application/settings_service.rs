use serde::{Deserialize, Serialize};

use crate::error::{require_db, ServiceError};
use crate::models::{GatewaySettings, UpdateGatewaySettings};
use crate::persistence::GatewaySettingsRepo;
use crate::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct GatewaySettingsResponse {
    pub id: String,
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_max_requests_per_minute: i64,
    pub rate_limit_max_tokens_per_minute: i64,
    pub created_at: String,
    pub updated_at: String,
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

pub async fn get() -> Result<GatewaySettingsResponse, ServiceError> {
    let pool = require_db()?;
    let settings = GatewaySettingsRepo::load_effective(pool).await?;

    Ok(GatewaySettingsResponse::from(settings))
}

pub async fn update(
    state: &AppState,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, ServiceError> {
    let pool = require_db()?;
    let previous_settings = {
        let gateway_guard = state.gateway.read().await;
        let settings_guard = gateway_guard.settings.read().await;
        settings_guard.clone()
    };

    let update = UpdateGatewaySettings {
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

    let settings = GatewaySettingsRepo::update(pool, &update).await?;

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
            id: s.id,
            bind_host: s.bind_host,
            bind_port: s.bind_port,
            allow_remote: s.allow_remote != 0,
            log_retention_days: s.log_retention_days,
            default_provider_id: s.default_provider_id,
            default_route_id: s.default_route_id,
            rate_limit_enabled: s.rate_limit_enabled != 0,
            rate_limit_max_requests_per_minute: s.rate_limit_max_requests_per_minute,
            rate_limit_max_tokens_per_minute: s.rate_limit_max_tokens_per_minute,
            created_at: s.created_at.to_string(),
            updated_at: s.updated_at.to_string(),
        }
    }
}

fn runtime_settings_changed(before: &GatewaySettings, after: &GatewaySettings) -> bool {
    before.bind_host != after.bind_host
        || before.bind_port != after.bind_port
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tokio::sync::RwLock;

    use crate::application::gateway_service::{load_gateway_context, start_existing_gateway};
    use crate::models::UpdateGatewaySettings;
    use crate::{init_database, AppState};

    use super::{update, UpdateSettingsPayload};

    fn unique_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-settings-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn free_port() -> i64 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        listener.local_addr().expect("local addr").port() as i64
    }

    #[tokio::test]
    async fn update_restarts_running_gateway_when_bind_port_changes() {
        let data_dir = unique_temp_dir();
        let pool = init_database(&data_dir).await.expect("db init");
        let old_port = free_port();
        let mut new_port = free_port();
        while new_port == old_port {
            new_port = free_port();
        }

        let initial = UpdateGatewaySettings {
            bind_host: Some("127.0.0.1".to_string()),
            bind_port: Some(old_port),
            ..Default::default()
        };
        crate::persistence::GatewaySettingsRepo::update(&pool, &initial)
            .await
            .expect("seed settings");

        let (log_sender, _log_receiver) = tokio::sync::mpsc::channel(1);
        let gateway = load_gateway_context(pool.clone(), log_sender)
            .await
            .expect("load gateway context");

        let (settings_change_tx, mut settings_change_rx) =
            tokio::sync::broadcast::channel::<()>(16);

        let state = AppState {
            gateway: Arc::new(RwLock::new(gateway)),
            gateway_server: Arc::new(RwLock::new(None)),
            provider_name_cache: Arc::new(RwLock::new(HashMap::new())),
            settings_change_tx,
        };

        start_existing_gateway(&state)
            .await
            .expect("start gateway");

        // 后台监听任务：收到 settings_change 事件后重启网关
        let state_clone = state.clone();
        let listener_handle = tokio::spawn(async move {
            loop {
                match settings_change_rx.recv().await {
                    Ok(()) => {
                        let _ = crate::application::gateway_service::restart(&state_clone).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let payload = UpdateSettingsPayload {
            bind_host: Some("127.0.0.1".to_string()),
            bind_port: Some(new_port),
            allow_remote: None,
            log_retention_days: None,
            default_provider_id: None,
            default_route_id: None,
            rate_limit_enabled: None,
            rate_limit_max_requests_per_minute: None,
            rate_limit_max_tokens_per_minute: None,
        };

        update(&state, payload).await.expect("update settings");

        // 由于重启异步发生，需要轮询等待新端口就绪
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .expect("build client");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut last_err = None;
        while std::time::Instant::now() < deadline {
            match client
                .get(format!("http://127.0.0.1:{new_port}/health"))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    listener_handle.abort(); // 清理监听任务
                    return;
                }
                Ok(_) => {
                    last_err = Some("非成功状态码".to_string());
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        panic!("网关未在 5s 内在新端口上响应: {:?}", last_err);
    }
}
