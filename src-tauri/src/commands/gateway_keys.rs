use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{GatewayKey, NewGatewayKey, UpdateGatewayKey};
use crate::persistence::GatewayKeyRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有 Key
#[tauri::command]
pub async fn list_gateway_keys(
    _state: State<'_, AppState>,
) -> Result<Vec<GatewayKeyResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let keys = GatewayKeyRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询 Key 失败: {e}"))?;

    Ok(keys.into_iter().map(GatewayKeyResponse::from).collect())
}

/// 获取单个 Key
#[tauri::command]
pub async fn get_gateway_key(
    _state: State<'_, AppState>,
    id: String,
) -> Result<GatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let key = GatewayKeyRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询 Key 失败: {e}"))?
        .ok_or("Key 不存在")?;

    Ok(GatewayKeyResponse::from(key))
}

/// 创建 Key
#[tauri::command]
pub async fn create_gateway_key(
    _state: State<'_, AppState>,
    payload: CreateGatewayKeyPayload,
) -> Result<CreateGatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let new = NewGatewayKey {
        name: payload.name,
        key_value: payload.key_value,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };

    let (key, plain_key) = GatewayKeyRepo::create(pool, &new)
        .await
        .map_err(|e| format!("创建 Key 失败: {e}"))?;

    Ok(CreateGatewayKeyResponse {
        key: GatewayKeyResponse::from(key),
        plain_key,
    })
}

/// 更新 Key
#[tauri::command]
pub async fn update_gateway_key(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateGatewayKeyPayload,
) -> Result<GatewayKeyResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateGatewayKey {
        name: payload.name,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };

    let key = GatewayKeyRepo::update(pool, &id, &update)
        .await
        .map_err(|e| format!("更新 Key 失败: {e}"))?
        .ok_or("Key 不存在")?;

    Ok(GatewayKeyResponse::from(key))
}

/// 删除 Key
#[tauri::command]
pub async fn delete_gateway_key(
    _state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = GatewayKeyRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除 Key 失败: {e}"))?;

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GatewayKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub max_concurrent: i64,
    pub is_expired: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GatewayKey> for GatewayKeyResponse {
    fn from(k: GatewayKey) -> Self {
        let is_expired = k.is_expired();
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            enabled: k.enabled != 0,
            expires_at: k.expires_at.map(|d| d.to_string()),
            max_concurrent: k.max_concurrent,
            is_expired,
            created_at: k.created_at.to_string(),
            updated_at: k.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateGatewayKeyResponse {
    pub key: GatewayKeyResponse,
    /// 明文 Key（仅创建时返回一次）
    pub plain_key: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGatewayKeyPayload {
    pub name: String,
    pub key_value: String,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGatewayKeyPayload {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}
