//! 协议适配层（数据驱动）
//!
//! Prism WASM 集成后，协议转换已移入 `prism_wasm.rs`。
//! 本模块仅负责：根据出站协议构建上游请求的 URL、认证头、Content-Type。
//!
//! 设计：配置表取代 trait 多态。三个协议的差异仅在路径和认证头，
//! 用 `ProtocolConfig` 数据表覆盖，无需 trait + 三个 impl。

use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderValue};
use once_cell::sync::Lazy;

use crate::gateway::error::GatewayError;
use crate::models::Provider;

// ---------------------------------------------------------------------------
// 上游请求（供中间件消费）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UpstreamRequest {
    pub url: String,
    pub method: String,
    pub headers: HeaderMap,
    pub body: serde_json::Value,
}

// ---------------------------------------------------------------------------
// 协议配置表
// ---------------------------------------------------------------------------

struct ProtocolConfig {
    path: &'static str,
    build_headers: fn(&str) -> Result<HeaderMap, GatewayError>,
}

static CONFIGS: Lazy<HashMap<&'static str, ProtocolConfig>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // OpenAI Chat Completions（Bearer Token 认证）
    m.insert(
        "openai",
        ProtocolConfig {
            path: "v1/chat/completions",
            build_headers: build_bearer_headers,
        },
    );
    // OpenAI Responses API（Bearer Token 认证）
    m.insert(
        "responses",
        ProtocolConfig {
            path: "v1/responses",
            build_headers: build_bearer_headers,
        },
    );
    // Anthropic Messages（x-api-key 认证）
    m.insert(
        "messages",
        ProtocolConfig {
            path: "v1/messages",
            build_headers: build_anthropic_headers,
        },
    );
    // Google Gemini generateContent（API Key 通过 URL 参数传递，Header 仅 Content-Type）
    m.insert(
        "gemini",
        ProtocolConfig {
            path: "v1beta/models",
            build_headers: build_gemini_headers,
        },
    );
    // Azure OpenAI（Bearer Token，路径含部署名，由 Provider base_url 承载）
    m.insert(
        "azure_openai",
        ProtocolConfig {
            path: "chat/completions",
            build_headers: build_bearer_headers,
        },
    );
    // Google Vertex AI（OAuth Bearer Token）
    m.insert(
        "google_vertex",
        ProtocolConfig {
            path: "publishers/google/models",
            build_headers: build_bearer_headers,
        },
    );
    // OpenAI Codex /agents（Bearer Token）
    m.insert(
        "openai_codex",
        ProtocolConfig {
            path: "v1/responses",
            build_headers: build_bearer_headers,
        },
    );
    // vLLM /v1/completions（Bearer Token）
    m.insert(
        "openai_vllm",
        ProtocolConfig {
            path: "v1/completions",
            build_headers: build_bearer_headers,
        },
    );
    // 兼容旧名
    m.insert(
        "openai_chat",
        ProtocolConfig {
            path: "v1/chat/completions",
            build_headers: build_bearer_headers,
        },
    );
    m.insert(
        "claude_messages",
        ProtocolConfig {
            path: "v1/messages",
            build_headers: build_anthropic_headers,
        },
    );
    m.insert(
        "openai_response",
        ProtocolConfig {
            path: "v1/responses",
            build_headers: build_bearer_headers,
        },
    );
    m
});

/// 协议是否受支持
pub fn is_supported(protocol: &str) -> bool {
    CONFIGS.contains_key(protocol)
}

// ---------------------------------------------------------------------------
// Headers 构建
// ---------------------------------------------------------------------------

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

fn build_bearer_headers(api_key: &str) -> Result<HeaderMap, GatewayError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|e| GatewayError::Internal(format!("无效的 Authorization 值: {e}")))?,
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

fn build_anthropic_headers(api_key: &str) -> Result<HeaderMap, GatewayError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key)
            .map_err(|e| GatewayError::Internal(format!("无效的 x-api-key 值: {e}")))?,
    );
    headers.insert(
        "anthropic-version",
        HeaderValue::from_static(ANTHROPIC_API_VERSION),
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

/// Gemini API Headers（API Key 通过 URL ?key= 参数传递，此处仅设置 Content-Type）
fn build_gemini_headers(_api_key: &str) -> Result<HeaderMap, GatewayError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

// ---------------------------------------------------------------------------
// 核心入口：构建上游请求
// ---------------------------------------------------------------------------

/// 根据出站协议构建上游请求（URL + Headers + Body）
///
/// 请求体已由 prism.wasm 转换为目标协议格式，此处仅透传并构建 URL/headers。
pub fn build_upstream_request(
    req_body: &[u8],
    provider: &Provider,
    api_key: &str,
    outbound_protocol: &str,
) -> Result<UpstreamRequest, GatewayError> {
    let config = CONFIGS.get(outbound_protocol).ok_or_else(|| {
        GatewayError::Transform(format!("不支持的出站协议: {outbound_protocol}"))
    })?;

    let body: serde_json::Value = serde_json::from_slice(req_body)
        .map_err(|e| GatewayError::Serialization(e.to_string()))?;

    Ok(UpstreamRequest {
        url: format!("{}/{}", provider.api_base_url, config.path),
        method: "POST".to_string(),
        headers: (config.build_headers)(api_key)?,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> Provider {
        let now = chrono::Utc::now().naive_utc();
        Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            protocols: r#"["chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: r#"[]"#.to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://api.openai.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 3,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            custom_headers: "[]".to_string(),
            models_passthrough: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_build_upstream_request_openai() {
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = build_upstream_request(&req_bytes, &provider, "sk-test", "openai").unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(result.method, "POST");
        assert!(result.headers.contains_key(axum::http::header::AUTHORIZATION));
        assert_eq!(result.body["model"], "gpt-4");
    }

    #[test]
    fn test_build_upstream_request_messages() {
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 1024
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = build_upstream_request(&req_bytes, &provider, "sk-test", "messages").unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/messages");
        assert!(result.headers.contains_key("x-api-key"));
        assert!(result.headers.contains_key("anthropic-version"));
    }

    #[test]
    fn test_build_upstream_request_unsupported() {
        let provider = test_provider();
        let result = build_upstream_request(b"{}", &provider, "sk-test", "unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_supported() {
        assert!(is_supported("openai"));
        assert!(is_supported("messages"));
        assert!(is_supported("responses"));
        assert!(!is_supported("unknown"));
    }
}
