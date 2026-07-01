use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::error::ServiceError;

/// 远程模型获取请求载荷
#[derive(Debug, Deserialize)]
pub struct FetchModelsPayload {
    pub api_base_url: String,
    pub api_key: String,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
}

/// 远程模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelInfo {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<i64>,
    pub owned_by: Option<String>,
    pub supported_endpoint_types: Vec<String>,
}

/// 从远程 Provider 获取模型列表
///
/// 发送 GET /v1/models 请求并解析响应，支持代理和自定义超时。
/// 内部处理了多种 API 响应格式（OpenAI, SillyTavern 等）。
pub async fn fetch_models(
    payload: FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, ServiceError> {
    let base_url = crate::models::Provider::normalize_api_base_url(&payload.api_base_url);
    let test_url = format!("{}/v1/models", base_url);
    let timeout_secs = payload.timeout_seconds.unwrap_or(10).min(30).max(1) as u64;

    info!("[fetch_provider_models] 请求URL: {test_url}, 超时: {timeout_secs}s");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| {
            error!("[fetch_provider_models] 构建客户端失败: {e}");
            ServiceError::Internal { message: format!("构建 HTTP 客户端失败: {e}"), detail: None }
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
        ServiceError::Internal { message: format!("请求模型列表失败: {e}"), detail: None }
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
        return Err(ServiceError::BadRequest { message: msg, code: None });
    }

    let text = resp.text().await.map_err(|e| {
        error!("[fetch_provider_models] 读取响应体失败: {e}");
        ServiceError::Internal { message: format!("读取响应失败: {e}"), detail: None }
    })?;

    info!("[fetch_provider_models] 响应体长度: {} bytes", text.len());

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!(
            "[fetch_provider_models] JSON 解析失败: {e}, 原始响应前200字符: {}",
            &text[..text.len().min(200)]
        );
        ServiceError::Internal { message: format!("解析响应 JSON 失败: {e}"), detail: None }
    })?;

    if let Some(success) = json.get("success").and_then(|v| v.as_bool()) {
        if !success {
            let msg = json["message"].as_str().unwrap_or("未知错误");
            warn!("[fetch_provider_models] API 返回 success: false, message: {msg}");
            return Err(ServiceError::BadRequest { message: msg.to_string(), code: None });
        }
    }

    let models = json["data"].as_array().ok_or_else(|| {
        let msg = "响应中未找到模型列表 (data 字段)".to_string();
        error!(
            "[fetch_provider_models] {msg}, 完整响应体: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );
        ServiceError::BadRequest { message: msg, code: None }
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
        return Err(ServiceError::BadRequest { message: "未获取到任何模型".to_string(), code: None });
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    info!("[fetch_provider_models] 成功获取 {} 个模型", result.len());
    Ok(result)
}
