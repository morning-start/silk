use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use serde::Serialize;
use thiserror::Error;

use crate::models::Provider;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("不支持的协议格式: {0}")]
    UnsupportedFormat(String),

    #[error("缺少必填字段: {0}")]
    MissingField(String),

    #[error("无效值: 字段 {field}, 原因 {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("上游错误: HTTP {status}: {message}")]
    UpstreamError { status: u16, message: String },

    #[error("序列化错误: {0}")]
    SerializationError(String),

    #[error("不支持的 content 类型: {0}")]
    UnsupportedContentType(String),

    #[error("转换错误: {0}")]
    Transform(String),
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::SerializationError(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// 共享常量
// ---------------------------------------------------------------------------

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

// ---------------------------------------------------------------------------
// 共享工具函数
// ---------------------------------------------------------------------------

/// 从上游错误响应中提取错误信息
pub fn extract_error_message(body: &serde_json::Value) -> String {
    body["error"]["message"]
        .as_str()
        .or_else(|| body["error"]["type"].as_str())
        .unwrap_or("unknown error")
        .to_string()
}

/// 构建 OpenAI 风格的认证头（Authorization: Bearer）
pub fn build_bearer_headers(api_key: &str) -> Result<HeaderMap, ProtocolError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|e| {
            ProtocolError::InvalidValue {
                field: "Authorization".to_string(),
                reason: e.to_string(),
            }
        })?,
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

/// 构建 Anthropic 风格的认证头（x-api-key）
pub fn build_anthropic_headers(api_key: &str) -> Result<HeaderMap, ProtocolError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key).map_err(|e| ProtocolError::InvalidValue {
            field: "x-api-key".to_string(),
            reason: e.to_string(),
        })?,
    );
    headers.insert("anthropic-version", HeaderValue::from_static(ANTHROPIC_API_VERSION));
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

/// 通用的上游请求构建函数
///
/// 三个适配器的 `transform_request` 遵循相同模式：
/// 反序列化 → 重序列化 → 构建 URL → 构建 headers。
/// 通过泛型 `T` 统一这一流程，仅需调用方提供类型、路径和 header 构建器。
pub fn build_upstream<T: Serialize + serde::de::DeserializeOwned>(
    req_body: &[u8],
    provider: &Provider,
    api_key: &str,
    path: &str,
    headers: fn(&str) -> Result<HeaderMap, ProtocolError>,
) -> Result<UpstreamRequest, ProtocolError> {
    let req: T = serde_json::from_slice(req_body)
        .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
    let body = serde_json::to_value(&req)
        .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
    Ok(UpstreamRequest {
        url: format!("{}/{}", provider.api_base_url, path),
        method: "POST".to_string(),
        headers: headers(api_key)?,
        body,
    })
}

// ---------------------------------------------------------------------------
// Upstream Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UpstreamRequest {
    /// 上游 URL（适配器决定具体路径，如 /v1/chat/completions）
    pub url: String,
    /// HTTP 方法（适配器决定，通常为 POST）
    pub method: String,
    pub headers: HeaderMap,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpstreamResponse {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: serde_json::Value,
}

// ---------------------------------------------------------------------------
// ProviderAdapter Trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// 适配器类型标识（如 "openai_chat", "claude_messages", "openai_response"）
    fn provider_type(&self) -> &'static str;

    /// 将原始请求体（JSON）转为上游请求
    ///
    /// req_body 是客户端发送的原始 JSON 字节
    async fn transform_request(
        &self,
        req_body: &[u8],
        provider: &Provider,
        selected_api_key: &str,
    ) -> Result<UpstreamRequest, ProtocolError>;

    /// 将上游响应转为客户端期望的格式（JSON）
    ///
    /// resp 是上游返回的原始响应
    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_error_display() {
        let err = ProtocolError::MissingField("model".to_string());
        assert!(err.to_string().contains("model"));

        let err = ProtocolError::InvalidValue {
            field: "temperature".to_string(),
            reason: "must be between 0 and 2".to_string(),
        };
        assert!(err.to_string().contains("temperature"));
    }

    #[test]
    fn test_unsupported_format() {
        let err = ProtocolError::UnsupportedFormat("custom".to_string());
        assert_eq!(err.to_string(), "不支持的协议格式: custom");
    }
}
