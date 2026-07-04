use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::error::ServiceError;
use crate::gateway::context::{GatewayContext, ProviderCache, RouteManager};
use crate::gateway::spawn_gateway_server;
use crate::protocol::AdapterRegistry;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct GatewayStatusResponse {
    pub running: bool,
    pub address: String,
    pub settings: GatewaySettingsInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GatewaySettingsInfo {
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GatewayStartResponse {
    pub success: bool,
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct GatewayStopResponse {
    pub success: bool,
    pub message: String,
}

pub async fn status(state: &AppState) -> Result<GatewayStatusResponse, ServiceError> {
    let running = {
        let server = state.gateway_server.read().await;
        server.is_some()
    };

    let gateway_guard = state.gateway.read().await;
    let settings = gateway_guard.settings.read().await;
    let gateway_settings = GatewaySettingsInfo::from(&*settings);

    Ok(GatewayStatusResponse {
        running,
        address: if running {
            format!(
                "{}:{}",
                gateway_settings.bind_host, gateway_settings.bind_port
            )
        } else {
            "未运行".to_string()
        },
        settings: gateway_settings,
    })
}

pub async fn start(state: &AppState) -> Result<GatewayStartResponse, ServiceError> {
    // 在写锁内完成检查和启动，避免 TOCTOU 竞态
    let mut server = state.gateway_server.write().await;
    if server.is_some() {
        return Err(ServiceError::BadRequest {
            message: "网关已在运行中".to_string(),
            code: None,
        });
    }
    // server 仍持有写锁，防止并发 start()

    let pool = crate::get_db_pool().ok_or(ServiceError::DbNotInitialized)?;
    let (log_sender, log_receiver) =
        tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);
    let _log_writer_handle = crate::gateway::spawn_log_writer(pool.clone(), log_receiver);

    let gateway = load_gateway_context(pool.clone(), log_sender)
        .await
        .map_err(|e| ServiceError::Internal {
            message: format!("加载网关上下文失败: {e}"),
            detail: None,
        })?;
    let gateway_server =
        spawn_gateway_server(gateway.clone())
            .await
            .map_err(|e| ServiceError::Internal {
                message: format!("启动网关失败: {e}"),
                detail: None,
            })?;

    // 先更新 gateway context（锁顺序：gateway -> gateway_server）
    {
        let mut state_gateway = state.gateway.write().await;
        *state_gateway = gateway.clone();
    }
    *server = Some(gateway_server);

    let settings = gateway.settings.read().await;
    Ok(GatewayStartResponse {
        success: true,
        address: format!("{}:{}", settings.bind_host, settings.bind_port),
    })
}

pub async fn stop(state: &AppState) -> Result<GatewayStopResponse, ServiceError> {
    let mut server = state.gateway_server.write().await;

    if let Some(handle) = server.take() {
        handle.stop().await;
        Ok(GatewayStopResponse {
            success: true,
            message: "网关已停止".to_string(),
        })
    } else {
        Err(ServiceError::BadRequest {
            message: "网关未运行".to_string(),
            code: None,
        })
    }
}

pub async fn restart(state: &AppState) -> Result<GatewayStartResponse, ServiceError> {
    // 停止旧服务器
    {
        let mut server = state.gateway_server.write().await;
        if let Some(handle) = server.take() {
            handle.stop().await;
        }
    }
    // 重新启动
    start(state).await
}

pub async fn start_existing_gateway(
    state: &AppState,
) -> Result<GatewayStartResponse, ServiceError> {
    // 在写锁内完成检查和启动
    let mut server = state.gateway_server.write().await;
    if server.is_some() {
        return Err(ServiceError::BadRequest {
            message: "网关已在运行中".to_string(),
            code: None,
        });
    }

    let gateway = state.gateway.read().await.clone();
    let gateway_server =
        spawn_gateway_server(gateway.clone())
            .await
            .map_err(|e| ServiceError::Internal {
                message: format!("启动网关失败: {e}"),
                detail: None,
            })?;

    *server = Some(gateway_server);

    let settings = gateway.settings.read().await;
    Ok(GatewayStartResponse {
        success: true,
        address: format!("{}:{}", settings.bind_host, settings.bind_port),
    })
}

/// 加载网关上下文（从 DB 读取路由规则、从文件读取设置等）
pub async fn load_gateway_context(
    pool: SqlitePool,
    log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
) -> Result<GatewayContext, sqlx::Error> {
    let settings_path = crate::get_settings_path()
        .ok_or_else(|| sqlx::Error::Protocol("网关设置路径未初始化".to_string()))?;
    let settings = crate::persistence::GatewaySettingsRepo::load_effective(settings_path);
    let route_manager = RouteManager::load(&pool).await?;
    let provider_cache = Arc::new(ProviderCache::new(Duration::from_secs(300)));
    let adapter_registry = Arc::new(AdapterRegistry::new());

    let plugins = crate::gateway::plugins::default_token_saving_plugins();

    Ok(GatewayContext::new(
        pool,
        Arc::new(RwLock::new(settings)),
        Arc::new(route_manager),
        provider_cache,
        log_sender,
        adapter_registry,
        plugins,
    )
    .await
    .map_err(|e| sqlx::Error::Protocol(e))?)
}

impl From<&crate::models::GatewaySettings> for GatewaySettingsInfo {
    fn from(settings: &crate::models::GatewaySettings) -> Self {
        Self {
            bind_host: settings.bind_host.clone(),
            bind_port: settings.bind_port,
            allow_remote: settings.allow_remote,
            log_retention_days: settings.log_retention_days,
            default_provider_id: settings.default_provider_id.clone(),
            default_route_id: settings.default_route_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tokio::sync::RwLock;

    use crate::persistence::GatewaySettingsRepo;
    use crate::{init_database, AppState, LookupCache};

    use super::load_gateway_context;
    use super::start_existing_gateway;
    use crate::models::UpdateGatewaySettings;

    #[tokio::test]
    async fn start_existing_gateway_marks_server_running() {
        let data_dir = unique_temp_dir();
        let pool = init_database(&data_dir).await.expect("db init");
        let bind_port = free_port();

        let update = UpdateGatewaySettings {
            bind_host: Some("127.0.0.1".to_string()),
            bind_port: Some(bind_port),
            ..Default::default()
        };
        crate::init_gateway_settings(&data_dir)
            .await
            .expect("init settings");
        let settings_path = crate::get_settings_path().expect("settings path");
        GatewaySettingsRepo::update(settings_path, &update).expect("update settings");

        let (log_sender, _log_receiver) = tokio::sync::mpsc::channel(1);
        let gateway = load_gateway_context(pool.clone(), log_sender)
            .await
            .expect("load gateway context");
        let (settings_change_tx, _settings_change_rx) = tokio::sync::broadcast::channel(16);
        let state = AppState {
            gateway: Arc::new(RwLock::new(gateway)),
            gateway_server: Arc::new(RwLock::new(None)),
            lookup_cache: Arc::new(RwLock::new(LookupCache::default())),
            settings_change_tx,
        };

        start_existing_gateway(&state)
            .await
            .expect("auto start gateway");

        assert!(state.gateway_server.read().await.is_some());
    }

    fn unique_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-gateway-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn free_port() -> i64 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        listener.local_addr().expect("local addr").port() as i64
    }
}
