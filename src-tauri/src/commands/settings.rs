use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{GatewaySettings, UpdateGatewaySettings};
use crate::persistence::GatewaySettingsRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取网关设置
#[tauri::command]
pub async fn get_gateway_settings(
    state: State<'_, AppState>,
) -> Result<GatewaySettingsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let settings = GatewaySettingsRepo::load_effective(pool)
        .await
        .map_err(|e| format!("加载设置失败: {e}"))?;

    Ok(GatewaySettingsResponse::from(settings))
}

/// 更新网关设置
#[tauri::command]
pub async fn update_gateway_settings(
    state: State<'_, AppState>,
    payload: UpdateSettingsPayload,
) -> Result<GatewaySettingsResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateGatewaySettings {
        bind_host: payload.bind_host,
        bind_port: payload.bind_port,
        allow_remote: payload.allow_remote,
        auth_token_hash: payload.auth_token_hash,
        log_retention_days: payload.log_retention_days,
        default_provider_id: payload.default_provider_id,
        default_route_id: payload.default_route_id,
    };

    let settings = GatewaySettingsRepo::update(pool, &update)
        .await
        .map_err(|e| format!("更新设置失败: {e}"))?;

    // 更新内存中的设置
    {
        let mut current = state.gateway.settings.write().await;
        *current = settings.clone();
    }

    Ok(GatewaySettingsResponse::from(settings))
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GatewaySettingsResponse {
    pub id: String,
    pub bind_host: String,
    pub bind_port: i64,
    pub allow_remote: bool,
    pub auth_token_hash: Option<String>,
    pub log_retention_days: i64,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GatewaySettings> for GatewaySettingsResponse {
    fn from(s: GatewaySettings) -> Self {
        Self {
            id: s.id,
            bind_host: s.bind_host,
            bind_port: s.bind_port,
            allow_remote: s.allow_remote != 0,
            auth_token_hash: s.auth_token_hash,
            log_retention_days: s.log_retention_days,
            default_provider_id: s.default_provider_id,
            default_route_id: s.default_route_id,
            created_at: s.created_at.to_string(),
            updated_at: s.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsPayload {
    pub bind_host: Option<String>,
    pub bind_port: Option<i64>,
    pub allow_remote: Option<bool>,
    pub auth_token_hash: Option<String>,
    pub log_retention_days: Option<i64>,
    pub default_provider_id: Option<String>,
    pub default_route_id: Option<String>,
}
