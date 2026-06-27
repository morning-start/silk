use serde::{Deserialize, Serialize};
use tauri::State;

use crate::gateway::{load_gateway_context, spawn_gateway_server};
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取网关运行状态
#[tauri::command]
pub async fn gateway_status(
    state: State<'_, AppState>,
) -> Result<GatewayStatusResponse, String> {
    let server = state.gateway_server.read().await;
    let running = server.is_some();

    let gateway_guard = state.gateway.read().await;
    let settings = gateway_guard.settings.read().await;
    let gateway_settings = GatewaySettingsInfo {
        id: settings.id.clone(),
        bind_host: settings.bind_host.clone(),
        bind_port: settings.bind_port,
        allow_remote: settings.allow_remote != 0,
        auth_token_hash: settings.auth_token_hash.clone(),
        log_retention_days: settings.log_retention_days,
        default_provider_id: settings.default_provider_id.clone(),
        default_route_id: settings.default_route_id.clone(),
        rate_limit_enabled: settings.rate_limit_enabled != 0,
        rate_limit_max_requests_per_minute: settings.rate_limit_max_requests_per_minute,
        rate_limit_max_tokens_per_minute: settings.rate_limit_max_tokens_per_minute,
        created_at: settings.created_at.to_string(),
        updated_at: settings.updated_at.to_string(),
    };

    Ok(GatewayStatusResponse {
        running,
        address: if running {
            format!("{}:{}", gateway_settings.bind_host, gateway_settings.bind_port)
        } else {
            "未运行".to_string()
        },
        settings: gateway_settings,
    })
}

/// 启动网关服务
#[tauri::command]
pub async fn gateway_start(
    state: State<'_, AppState>,
) -> Result<GatewayStartResponse, String> {
    // 检查是否已运行
    {
        let server = state.gateway_server.read().await;
        if server.is_some() {
            return Err("网关已在运行中".to_string());
        }
    }

    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let (log_sender, log_receiver) = tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);

    // 启动后台日志写入任务
    let _log_writer_handle = crate::gateway::spawn_log_writer(pool.clone(), log_receiver);

    // 加载网关上下文并启动服务
    let gateway = load_gateway_context(pool.clone(), log_sender)
        .await
        .map_err(|e| format!("加载网关上下文失败: {e}"))?;
    let gateway_server = spawn_gateway_server(gateway.clone())
        .await
        .map_err(|e| format!("启动网关失败: {e}"))?;

    // 同步更新状态 —— 替换内存中的上下文和新服务句柄
    {
        let mut state_gateway = state.gateway.write().await;
        *state_gateway = gateway.clone();
        let mut server = state.gateway_server.write().await;
        *server = Some(gateway_server);
    }

    let settings = &gateway.settings;
    tracing::info!("网关已启动");

    Ok(GatewayStartResponse {
        success: true,
        address: format!(
            "{}:{}",
            settings.read().await.bind_host,
            settings.read().await.bind_port
        ),
    })
}

/// 停止网关服务
#[tauri::command]
pub async fn gateway_stop(
    state: State<'_, AppState>,
) -> Result<GatewayStopResponse, String> {
    let mut server = state.gateway_server.write().await;

    if let Some(handle) = server.take() {
        handle.stop().await;
        Ok(GatewayStopResponse {
            success: true,
            message: "网关已停止".to_string(),
        })
    } else {
        Err("网关未运行".to_string())
    }
}

/// 重启网关服务
#[tauri::command]
pub async fn gateway_restart(
    state: State<'_, AppState>,
) -> Result<GatewayStartResponse, String> {
    // 先停止
    {
        let mut server = state.gateway_server.write().await;
        if let Some(handle) = server.take() {
            handle.stop().await;
        }
    }

    // 再启动
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let (log_sender, log_receiver) = tokio::sync::mpsc::channel::<crate::models::NewRequestLog>(1000);

    let _log_writer_handle = crate::gateway::spawn_log_writer(pool.clone(), log_receiver);

    let gateway = load_gateway_context(pool.clone(), log_sender)
        .await
        .map_err(|e| format!("加载网关上下文失败: {e}"))?;
    let gateway_server = spawn_gateway_server(gateway.clone())
        .await
        .map_err(|e| format!("启动网关失败: {e}"))?;

    {
        let mut server = state.gateway_server.write().await;
        *server = Some(gateway_server);
    }

    let settings = gateway.settings.read().await;
    Ok(GatewayStartResponse {
        success: true,
        address: format!("{}:{}", settings.bind_host, settings.bind_port),
    })
}

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct GatewayStatusResponse {
    pub running: bool,
    pub address: String,
    pub settings: GatewaySettingsInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GatewaySettingsInfo {
    pub id: String,
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub auth_token_hash: Option<String>,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_max_requests_per_minute: i64,
    pub rate_limit_max_tokens_per_minute: i64,
    pub created_at: String,
    pub updated_at: String,
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
