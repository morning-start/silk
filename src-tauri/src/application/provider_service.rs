use serde::{Deserialize, Serialize};

use crate::crypto::CryptoError;
use crate::error::{require_db, require_found, ServiceError};
use crate::load_balancer::{LoadBalanceStrategy, LoadBalancer};
use crate::models::{NewProvider, Provider, ProviderKeyEntry, UpdateProvider};
use crate::persistence::ProviderRepo;
use crate::AppState;

/// 按负载均衡策略选择一个 API Key（已从 models/provider.rs 移出）
pub fn select_api_key(provider: &Provider) -> Result<String, CryptoError> {
    let entries = provider.keys_vec();
    let strategy = LoadBalanceStrategy::from_str(&provider.key_strategy);
    let balancer = LoadBalancer::new(entries, strategy);
    let selected = balancer
        .select()
        .ok_or(CryptoError::InvalidFormat)?;

    if selected.enabled && !selected.value.is_empty() {
        let decrypted = crate::crypto::decrypt(&selected.value)?;
        Ok(decrypted)
    } else {
        Err(CryptoError::InvalidFormat)
    }
}

/// 规范化 API Base URL：去除尾部 /v1 或 /v1/
pub fn normalize_api_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed[..trimmed.len() - 3]
            .trim_end_matches('/')
            .to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub protocols: Vec<String>,
    pub models: Vec<String>,
    pub key_count: i64,
    /// 密钥条目列表（value 已解密用于展示）
    pub keys: Vec<ProviderKeyEntry>,
    pub api_base_url: String,
    pub proxy_url: Option<String>,
    pub timeout_seconds: i64,
    pub max_retries: i64,
    pub status: String,
    pub health_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderPayload {
    pub name: String,
    pub protocols: Vec<String>,
    pub api_base_url: String,
    pub models: Vec<String>,
    pub keys: Vec<crate::models::ProviderKeyEntry>,
    pub key_strategy: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProviderTestResponse {
    pub status_code: u16,
    pub response_time_ms: i64,
    pub health_status: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderPayload {
    pub name: Option<String>,
    pub protocols: Option<Vec<String>>,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub models: Option<Vec<String>>,
    pub keys: Option<Vec<crate::models::ProviderKeyEntry>>,
    pub key_strategy: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub metadata_json: Option<String>,
}

pub use super::model_fetcher::{fetch_models, FetchModelsPayload, ProviderModelInfo};

pub async fn list() -> Result<Vec<ProviderResponse>, ServiceError> {
    let pool = require_db()?;
    let providers = ProviderRepo::find_all(pool).await?;

    Ok(providers
        .into_iter()
        .map(ProviderResponse::from_provider)
        .collect())
}

pub async fn get(id: String) -> Result<ProviderResponse, ServiceError> {
    let pool = require_db()?;
    let provider = require_found(ProviderRepo::find_by_id(pool, &id).await?, "Provider")?;
    Ok(ProviderResponse::from_provider(provider))
}

pub async fn create(
    state: &AppState,
    payload: CreateProviderPayload,
) -> Result<ProviderResponse, ServiceError> {
    let pool = require_db()?;

    let mut keys = payload.keys;
    for entry in &mut keys {
        if !entry.value.is_empty() {
            entry.value = crate::crypto::encrypt(&entry.value)
                .map_err(|e| ServiceError::Internal { message: format!("加密 API Key 失败: {e}"), detail: None })?;
        }
    }

    let new = NewProvider {
        name: payload.name,
        protocols: payload.protocols,
        models: payload.models,
        keys,
        key_strategy: payload.key_strategy,
        api_base_url: normalize_api_base_url(&payload.api_base_url),
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: Some(payload.status.unwrap_or_else(|| "enabled".to_string())),
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = ProviderRepo::create(pool, &new).await?;

    state.refresh_lookup().await;

    Ok(ProviderResponse::from_provider(provider))
}

pub async fn update(
    state: &AppState,
    id: String,
    payload: UpdateProviderPayload,
) -> Result<ProviderResponse, ServiceError> {
    let pool = require_db()?;

    let keys = match payload.keys {
        Some(mut ks) => {
            for entry in &mut ks {
                if !entry.value.is_empty() {
                    entry.value = crate::crypto::encrypt(&entry.value)
                        .map_err(|e| ServiceError::Internal { message: format!("加密 API Key 失败: {e}"), detail: None })?;
                }
            }
            Some(ks)
        }
        None => None,
    };

    let update = UpdateProvider {
        name: payload.name,
        protocols: payload.protocols,
        models: payload.models,
        keys,
        key_strategy: payload.key_strategy,
        api_base_url: payload
            .api_base_url
            .map(|u| normalize_api_base_url(&u)),
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: payload.status,
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = require_found(ProviderRepo::update(pool, &id, &update).await?, "Provider")?;

    state.invalidate_cache(&id).await;
    state.refresh_lookup().await;

    Ok(ProviderResponse::from_provider(provider))
}

pub async fn test(state: &AppState, id: String) -> Result<ProviderTestResponse, ServiceError> {
    let pool = require_db()?;

    let provider = require_found(ProviderRepo::find_by_id(pool, &id).await?, "Provider")?;

    let base_url = normalize_api_base_url(&provider.api_base_url);
    let test_url = format!("{}/v1/models", base_url);
    let timeout_secs = provider.timeout_seconds.min(10).max(1) as u64;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| ServiceError::Internal { message: format!("构建 HTTP 客户端失败: {e}"), detail: None })?;

    let start = std::time::Instant::now();

    let api_key = select_api_key(&provider)
        .map_err(|e| ServiceError::Internal { message: format!("获取 API Key 失败: {e}"), detail: None })?;

    let result = client
        .get(&test_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send()
        .await;

    let elapsed_ms = start.elapsed().as_millis() as i64;

    let (status_code, error_msg) = match result {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if resp.status().is_success() {
                (code, None)
            } else {
                let body = resp.text().await.unwrap_or_default();
                let err = if body.len() > 200 {
                    format!("{}: {}", code, &body[..200])
                } else {
                    format!("{}: {}", code, body)
                };
                (code, Some(err))
            }
        }
        Err(e) => (0, Some(e.to_string())),
    };

    let health_status = if error_msg.is_none() && (200..300).contains(&status_code) {
        "healthy"
    } else {
        "unhealthy"
    };

    let now = chrono::Utc::now().naive_utc();
    ProviderRepo::update_health_status(pool, &id, health_status, now).await?;

    state.invalidate_cache(&id).await;

    Ok(ProviderTestResponse {
        status_code,
        response_time_ms: elapsed_ms,
        health_status: health_status.to_string(),
        error: error_msg,
    })
}

pub async fn delete(state: &AppState, id: String) -> Result<bool, ServiceError> {
    let pool = require_db()?;
    let deleted = ProviderRepo::delete(pool, &id).await?;

    if deleted {
        state.invalidate_cache(&id).await;
        state.refresh_lookup().await;
    }

    Ok(deleted)
}

impl ProviderResponse {
    fn from_provider(p: Provider) -> Self {
        let (keys, key_count) = Self::parse_keys(&p.keys);
        let protocols = p.protocols_vec();
        let models = p.models_vec();
        Self {
            id: p.id,
            name: p.name,
            protocols,
            models,
            key_count,
            keys,
            api_base_url: p.api_base_url,
            proxy_url: p.proxy_url,
            timeout_seconds: p.timeout_seconds,
            max_retries: p.max_retries,
            status: p.status,
            health_status: p.health_status,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        }
    }

    /// 解析 keys JSON 字段并解密用于展示
    fn parse_keys(keys_json: &str) -> (Vec<ProviderKeyEntry>, i64) {
        let entries: Vec<ProviderKeyEntry> = serde_json::from_str(keys_json).unwrap_or_default();
        let count = entries.len() as i64;
        let decrypted: Vec<ProviderKeyEntry> = entries
            .into_iter()
            .map(|mut k| {
                if !k.value.is_empty() {
                    if let Ok(decrypted_val) = crate::crypto::decrypt(&k.value) {
                        k.value = decrypted_val;
                    } else {
                        // 如果是加密 JSON 格式但解密失败（如主密钥改变），清空它以提示重新填写
                        if k.value.starts_with('{') && k.value.contains("\"ciphertext\"") {
                            k.value = String::new();
                        }
                    }
                }
                k
            })
            .collect();
        (decrypted, count)
    }
}