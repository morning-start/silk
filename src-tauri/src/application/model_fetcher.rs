use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::error::ServiceError;
use crate::application::provider_service::normalize_api_base_url;

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
/// 端点指向本机网关时直接读本地模型池（不走 HTTP）。
pub async fn fetch_models(
    payload: FetchModelsPayload,
) -> Result<Vec<ProviderModelInfo>, ServiceError> {
    let base_url = normalize_api_base_url(&payload.api_base_url);

    // 端点指向本机网关（默认预设值）→ 本地直查模型池，避免 HTTP 往返
    if is_local_gateway(&base_url) {
        tracing::info!("[fetch_provider_models] 端点是本机网关，直接读本地模型池");
        return list_local_pool_models().await;
    }

    let test_url = format!("{}/v1/models", base_url);
    let timeout_secs = payload.timeout_seconds.unwrap_or(10).clamp(1, 30) as u64;

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

    parse_models_response(&json)
}

/// 判断端点是否指向本机网关（预设默认值场景：表单端点 = silk 网关地址）
fn is_local_gateway(base_url: &str) -> bool {
    let Ok(settings) = crate::models::GatewaySettings::load(
        crate::get_settings_path().unwrap_or(std::path::Path::new("")),
    ) else {
        return false;
    };
    let expected = format!("http://{}:{}", settings.bind_host, settings.bind_port);
    // 兼容 host 归一化：127.0.0.1 / localhost / [::1]
    let norm = |h: &str| h.replace("127.0.0.1", "localhost").replace("[::1]", "localhost");
    let base_norm = norm(base_url.trim_end_matches('/'));
    let expect_norm = norm(&expected);
    base_norm == expect_norm
}

/// 直接读本地模型池（list_all_models = 模型池 + 全部启用渠道的模型），
/// 与网关 /v1/models 结果一致。空池返回明确错误，提示先配置渠道/模型池。
async fn list_local_pool_models() -> Result<Vec<ProviderModelInfo>, ServiceError> {
    let items = crate::application::models_listing::list_all_models().await?;
    if items.is_empty() {
        return Err(ServiceError::BadRequest {
            message: "本机模型池为空：请先在「渠道 / 模型」页添加渠道并拉取模型，或创建模型映射".to_string(),
            code: None,
        });
    }
    let mut result: Vec<ProviderModelInfo> = items
        .iter()
        .map(|item| ProviderModelInfo {
            id: item.id.clone(),
            object: Some(item.object.clone()),
            created: Some(item.created),
            owned_by: Some(item.owned_by.clone()),
            supported_endpoint_types: Vec::new(),
        })
        .collect();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    tracing::info!("[fetch_provider_models] 本地模型池共 {} 个模型", result.len());
    Ok(result)
}

/// 从响应 JSON 解析模型列表（纯函数，便于单测）。
///
/// 兼容多种格式：
/// - OpenAI：`{"data": [{"id": "..."}]}`
/// - Gemini：`{"models": [{"name": "models/gemini-2.5-pro", "displayName": "..."}]}`
/// - Ollama：`{"models": [{"name": "llama3", "model": "llama3:latest"}]}`
/// - 条目 id 回退：id → model → name；Gemini `models/` 前缀剥离
fn parse_models_response(json: &serde_json::Value) -> Result<Vec<ProviderModelInfo>, ServiceError> {
    // 兼容多种模型列表容器：OpenAI 用 data[]，Gemini/Ollama 用 models[]
    let container = json["data"]
        .as_array()
        .or_else(|| json["models"].as_array())
        .or_else(|| json["model_list"].as_array())
        .ok_or_else(|| {
            let msg = "响应中未找到模型列表 (data/models 字段)".to_string();
            error!(
                "[fetch_provider_models] {msg}, 完整响应体: {}",
                serde_json::to_string_pretty(json).unwrap_or_default()
            );
            ServiceError::BadRequest { message: msg, code: None }
        })?;

    let mut result: Vec<ProviderModelInfo> = Vec::new();
    for item in container {
        // 条目 id 兼容：id → name → model（Gemini/Ollama 用 name，部分端点用 model），
        // Gemini 的 name 形如 "models/gemini-2.5-pro"，剥离前缀
        let raw_id = item["id"]
            .as_str()
            .or_else(|| item["name"].as_str())
            .or_else(|| item["model"].as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(model_id) = raw_id {
            let model_id = model_id
                .strip_prefix("models/")
                .unwrap_or(model_id)
                .to_string();
            result.push(ProviderModelInfo {
                id: model_id,
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
        let hint = if container.is_empty() {
            "端点返回了空模型列表".to_string()
        } else {
            format!(
                "响应含 {} 个条目但未解析出模型 id（不支持的格式）",
                container.len()
            )
        };
        warn!("[fetch_provider_models] {hint}");
        return Err(ServiceError::BadRequest { message: hint, code: None });
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    info!("[fetch_provider_models] 成功获取 {} 个模型", result.len());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Vec<ProviderModelInfo>, ServiceError> {
        let json: serde_json::Value = serde_json::from_str(text).expect("测试 JSON 应合法");
        parse_models_response(&json)
    }

    #[test]
    fn parses_openai_data_format() {
        let models = parse(
            r#"{"object":"list","data":[{"id":"gpt-5","object":"model","owned_by":"openai"},{"id":"gpt-4o","object":"model"}]}"#,
        )
        .unwrap();
        assert_eq!(models.len(), 2);
        // 按 id 排序后 gpt-4o 在前
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[0].owned_by, None);
        assert_eq!(models[1].id, "gpt-5");
        assert_eq!(models[1].owned_by.as_deref(), Some("openai"));
    }

    #[test]
    fn parses_gemini_models_format_with_prefix_strip() {
        let models = parse(
            r#"{"models":[{"name":"models/gemini-2.5-pro","displayName":"Gemini 2.5 Pro"},{"name":"models/gemini-2.5-flash"}]}"#,
        )
        .unwrap();
        assert_eq!(models.len(), 2);
        // 按 id 排序后 flash 在前；Gemini name 的 "models/" 前缀被剥离
        assert_eq!(models[0].id, "gemini-2.5-flash");
        assert_eq!(models[1].id, "gemini-2.5-pro");
    }

    #[test]
    fn parses_ollama_models_format_via_name_field() {
        let models = parse(r#"{"models":[{"name":"llama3","model":"llama3:latest"},{"name":"qwen2.5"}]}"#)
            .unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3");
        assert_eq!(models[1].id, "qwen2.5");
    }

    #[test]
    fn sorts_and_dedups_no_duplicates() {
        let models = parse(
            r#"{"data":[{"id":"b-model"},{"id":"a-model"},{"id":"c-model"}]}"#,
        )
        .unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["a-model", "b-model", "c-model"]);
    }

    #[test]
    fn empty_data_array_reports_clear_error() {
        let err = parse(r#"{"object":"list","data":[]}"#).unwrap_err();
        assert!(err.to_string().contains("空模型列表"), "错误应说明端点返回空: {err}");
    }

    #[test]
    fn entries_without_id_report_format_error() {
        let err = parse(r#"{"data":[{"display_name":"x"},{"display_name":"y"}]}"#).unwrap_err();
        assert!(err.to_string().contains("未解析出模型 id"), "错误应说明格式不支持: {err}");
    }

    #[test]
    fn missing_container_reports_missing_field() {
        let err = parse(r#"{"error":"unauthorized"}"#).unwrap_err();
        assert!(err.to_string().contains("data/models"), "错误应指出缺少模型列表字段: {err}");
    }

    // ---- 本机网关判定（纯 host/port 匹配逻辑）----

    #[test]
    fn local_gateway_matches_default_endpoint() {
        // 与 fetch_models 的 normalize 一致：127.0.0.1:1877/v1 → http://127.0.0.1:1877
        let normalized = crate::application::provider_service::normalize_api_base_url(
            "http://127.0.0.1:1877/v1",
        );
        assert_eq!(normalized, "http://127.0.0.1:1877");
        // host 归一化后与网关设置（127.0.0.1:1877）一致
        assert_eq!(
            normalize_host(&normalized),
            normalize_host("http://localhost:1877")
        );
    }

    #[test]
    fn remote_endpoint_is_not_local_gateway() {
        assert_ne!(
            normalize_host("http://api.example.com:443"),
            normalize_host("http://localhost:1877")
        );
    }

    fn normalize_host(h: &str) -> String {
        h.trim_end_matches('/')
            .replace("127.0.0.1", "localhost")
            .replace("[::1]", "localhost")
    }
}
