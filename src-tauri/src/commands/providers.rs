use serde::{Deserialize, Serialize};
use tauri::State;

use crate::crypto::{encrypt_api_key, derive_key_from_password};
use crate::models::{NewProvider, Provider, UpdateProvider};
use crate::persistence::ProviderRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有 Provider
#[tauri::command]
pub async fn list_providers(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let providers = ProviderRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询 Provider 失败: {e}"))?;

    Ok(providers.into_iter().map(ProviderResponse::from).collect())
}

/// 获取单个 Provider
#[tauri::command]
pub async fn get_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let provider = ProviderRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询 Provider 失败: {e}"))?
        .ok_or("Provider 不存在")?;

    Ok(ProviderResponse::from(provider))
}

/// 创建 Provider
#[tauri::command]
pub async fn create_provider(
    state: State<'_, AppState>,
    payload: CreateProviderPayload,
) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    // 加密 API Key
    let master_key = derive_key_from_password("silk-master-key", b"salt")
        .map_err(|e| format!("密钥派生失败: {e}"))?;
    let encrypted_key = encrypt_api_key(&payload.api_key, &master_key)
        .map_err(|e| format!("加密 API Key 失败: {e}"))?;

    let new = NewProvider {
        name: payload.name,
        provider_type: payload.provider_type,
        api_base_url: payload.api_base_url,
        api_key: payload.api_key, // 明文，Repo 会加密
        model_name: payload.model_name,
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: Some(payload.status.unwrap_or_else(|| "enabled".to_string())),
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = ProviderRepo::create(pool, &new, &encrypted_key)
        .await
        .map_err(|e| format!("创建 Provider 失败: {e}"))?;

    Ok(ProviderResponse::from(provider))
}

/// 更新 Provider
#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateProviderPayload,
) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    // 如果更新了 API Key，需要重新加密
    let encrypted_key = if let Some(ref api_key) = payload.api_key {
        let master_key = derive_key_from_password("silk-master-key", b"salt")
            .map_err(|e| format!("密钥派生失败: {e}"))?;
        Some(encrypt_api_key(api_key, &master_key)
            .map_err(|e| format!("加密 API Key 失败: {e}"))?)
    } else {
        None
    };

    let update = UpdateProvider {
        name: payload.name,
        provider_type: payload.provider_type,
        api_base_url: payload.api_base_url,
        api_key: payload.api_key,
        model_name: payload.model_name,
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: payload.status,
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = ProviderRepo::update(pool, &id, &update, encrypted_key.as_deref())
        .await
        .map_err(|e| format!("更新 Provider 失败: {e}"))?
        .ok_or("Provider 不存在")?;

    // 清除缓存
    state.gateway.provider_cache.invalidate(&id).await;

    Ok(ProviderResponse::from(provider))
}

/// 删除 Provider
#[tauri::command]
pub async fn delete_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = ProviderRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除 Provider 失败: {e}"))?;

    if deleted {
        state.gateway.provider_cache.invalidate(&id).await;
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub api_base_url: String,
    pub model_name: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: i64,
    pub max_retries: i64,
    pub status: String,
    pub health_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Provider> for ProviderResponse {
    fn from(p: Provider) -> Self {
        Self {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            api_base_url: p.api_base_url,
            model_name: p.model_name,
            proxy_url: p.proxy_url,
            timeout_seconds: p.timeout_seconds,
            max_retries: p.max_retries,
            status: p.status,
            health_status: p.health_status,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderPayload {
    pub name: String,
    pub provider_type: String,
    pub api_base_url: String,
    pub api_key: String,
    pub model_name: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderPayload {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub metadata_json: Option<String>,
}
