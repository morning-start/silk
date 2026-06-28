use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, warn};

use crate::models::{NewProvider, Provider, ProviderKeyEntry, UpdateProvider};
use crate::persistence::ProviderRepo;
use crate::AppState;

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

#[derive(Debug, Deserialize)]
pub struct FetchModelsPayload {
    pub api_base_url: String,
    pub api_key: String,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProviderModelInfo {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<i64>,
    pub owned_by: Option<String>,
    pub supported_endpoint_types: Vec<String>,
}

pub async fn list(_state: State<'_, AppState>) -> Result<Vec<ProviderResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let providers = ProviderRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询 Provider 失败: {e}"))?;

    Ok(providers
        .into_iter()
        .map(ProviderResponse::from_provider)
        .collect())
}

pub async fn get(_state: State<'_, AppState>, id: String) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let provider = ProviderRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询 Provider 失败: {e}"))?
        .ok_or("Provider 不存在")?;

    Ok(ProviderResponse::from_provider(provider))
}

pub async fn create(
    _state: State<'_, AppState>,
    payload: CreateProviderPayload,
) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let new = NewProvider {
        name: payload.name,
        protocols: payload.protocols,
        models: payload.models,
        keys: payload.keys, // 明文存储，不再加密
        key_strategy: payload.key_strategy,
        api_base_url: crate::models::Provider::normalize_api_base_url(&payload.api_base_url),
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: Some(payload.status.unwrap_or_else(|| "enabled".to_string())),
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = ProviderRepo::create(pool, &new)
        .await
        .map_err(|e| format!("创建 Provider 失败: {e}"))?;

    Ok(ProviderResponse::from_provider(provider))
}

pub async fn update(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateProviderPayload,
) -> Result<ProviderResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateProvider {
        name: payload.name,
        protocols: payload.protocols,
        models: payload.models,
        keys: payload.keys, // 明文存储，不再加密
        key_strategy: payload.key_strategy,
        api_base_url: payload
            .api_base_url
            .map(|u| crate::models::Provider::normalize_api_base_url(&u)),
        proxy_url: payload.proxy_url,
        timeout_seconds: payload.timeout_seconds,
        max_retries: payload.max_retries,
        status: payload.status,
        health_status: None,
        last_health_check_at: None,
        metadata_json: payload.metadata_json,
    };

    let provider = ProviderRepo::update(pool, &id, &update)
        .await
        .map_err(|e| format!("更新 Provider 失败: {e}"))?
        .ok_or("Provider 不存在")?;

    state
        .gateway
        .read()
        .await
        .provider_cache
        .invalidate(&id)
        .await;

    Ok(ProviderResponse::from_provider(provider))
}

pub async fn test(state: State<'_, AppState>, id: String) -> Result<ProviderTestResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let provider = ProviderRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询 Provider 失败: {e}"))?
        .ok_or("Provider 不存在")?;

    let base_url = crate::models::Provider::normalize_api_base_url(&provider.api_base_url);
    let test_url = format!("{}/v1/models", base_url);
    let timeout_secs = provider.timeout_seconds.min(10).max(1) as u64;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {e}"))?;

    let start = std::time::Instant::now();

    let api_key = provider
        .decrypted_api_key(&[0u8; 32]) // 明文存储，主密钥不再使用
        .map_err(|e| format!("获取 API Key 失败: {e}"))?;

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
    ProviderRepo::update_health_status(pool, &id, health_status, now)
        .await
        .map_err(|e| format!("更新健康状态失败: {e}"))?;

    state
        .gateway
        .read()
        .await
        .provider_cache
        .invalidate(&id)
        .await;

    Ok(ProviderTestResponse {
        status_code,
        response_time_ms: elapsed_ms,
        health_status: health_status.to_string(),
        error: error_msg,
    })
}

pub async fn delete(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = ProviderRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除 Provider 失败: {e}"))?;

    if deleted {
        state
            .gateway
            .read()
            .await
            .provider_cache
            .invalidate(&id)
            .await;
    }

    Ok(deleted)
}

pub async fn fetch_models(
    _state: State<'_, AppState>,
    payload: FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, String> {
    let base_url = crate::models::Provider::normalize_api_base_url(&payload.api_base_url);
    let test_url = format!("{}/v1/models", base_url);
    let timeout_secs = payload.timeout_seconds.unwrap_or(10).min(30).max(1) as u64;

    info!("[fetch_provider_models] 请求URL: {test_url}, 超时: {timeout_secs}s");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| {
            error!("[fetch_provider_models] 构建客户端失败: {e}");
            format!("构建 HTTP 客户端失败: {e}")
        })?;

    let mut req = client
        .get(&test_url)
        .header("Authorization", format!("Bearer {}", payload.api_key))
        .header("Content-Type", "application/json");

    if let Some(ref proxy) = payload.proxy_url {
        if !proxy.is_empty() {
            info!("[fetch_provider_models] 使用代理: {proxy}");
            req = req.header("X-Proxy-Url", proxy);
        }
    }

    let resp = req.send().await.map_err(|e| {
        error!("[fetch_provider_models] 网络请求失败: {e}");
        format!("请求模型列表失败: {e}")
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = if body.len() > 200 {
            format!("{}: {}", status, &body[..200])
        } else {
            format!("{}: {}", status, body)
        };
        warn!("[fetch_provider_models] HTTP {status} 非成功状态码: {msg}");
        return Err(msg);
    }

    let text = resp.text().await.map_err(|e| {
        error!("[fetch_provider_models] 读取响应体失败: {e}");
        format!("读取响应失败: {e}")
    })?;

    info!("[fetch_provider_models] 响应体长度: {} bytes", text.len());

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!(
            "[fetch_provider_models] JSON 解析失败: {e}, 原始响应前200字符: {}",
            &text[..text.len().min(200)]
        );
        format!("解析响应 JSON 失败: {e}")
    })?;

    if let Some(success) = json.get("success").and_then(|v| v.as_bool()) {
        if !success {
            let msg = json["message"].as_str().unwrap_or("未知错误");
            warn!("[fetch_provider_models] API 返回 success: false, message: {msg}");
            return Err(msg.to_string());
        }
    }

    let models = json["data"].as_array().ok_or_else(|| {
        let msg = "响应中未找到模型列表 (data 字段)".to_string();
        error!(
            "[fetch_provider_models] {msg}, 完整响应体: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );
        msg
    })?;

    let mut result: Vec<ProviderModelInfo> = Vec::new();
    for item in models {
        if let Some(model_id) = item["id"].as_str() {
            result.push(ProviderModelInfo {
                id: model_id.to_string(),
                object: item["object"].as_str().map(|s| s.to_string()),
                created: item["created"].as_i64(),
                owned_by: item["owned_by"].as_str().map(|s| s.to_string()),
                supported_endpoint_types: item["supported_endpoint_types"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }

    if result.is_empty() {
        warn!("[fetch_provider_models] data 数组存在但未解析到任何模型 id");
        return Err("未获取到任何模型".to_string());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    info!("[fetch_provider_models] 成功获取 {} 个模型", result.len());
    Ok(result)
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

    /// 解析 keys JSON 字段（明文存储，直接返回）
    fn parse_keys(keys_json: &str) -> (Vec<ProviderKeyEntry>, i64) {
        let entries: Vec<ProviderKeyEntry> = serde_json::from_str(keys_json).unwrap_or_default();
        let count = entries.len() as i64;
        // 旧加密数据视为无效，清空 value 让用户重新填写
        let cleaned: Vec<ProviderKeyEntry> = entries
            .into_iter()
            .map(|mut k| {
                if k.value.starts_with('{') && k.value.contains("\"ciphertext\"") {
                    k.value = String::new();
                }
                k
            })
            .collect();
        (cleaned, count)
    }
}
