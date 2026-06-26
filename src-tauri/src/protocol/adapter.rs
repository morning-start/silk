use async_trait::async_trait;
use axum::http::HeaderMap;
use thiserror::Error;

use crate::protocol::canonical::{CanonicalRequest, CanonicalResponse};
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

    #[error("序列化错误: {0}")]
    SerializationError(String),

    #[error("不支持的 content 类型: {0}")]
    UnsupportedContentType(String),
}

impl From<ProtocolError> for crate::error::AppError {
    fn from(err: ProtocolError) -> Self {
        crate::error::AppError::Protocol(err.to_string())
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::SerializationError(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Upstream Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UpstreamRequest {
    pub url: String,
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

    /// 将 Canonical Request 转为上游请求
    ///
    # [allow(dead_code)]
    async fn canonicalize_request(
        &self,
        req: &CanonicalRequest,
        _provider: &Provider,
    ) -> Result<UpstreamRequest, ProtocolError> {
        let _ = (req, _provider);
        Err(ProtocolError::UnsupportedFormat(self.provider_type().to_string()))
    }

    /// 将上游响应转为 Canonical Response
    ///
    # [allow(dead_code)]
    async fn parse_response(
        &self,
        _resp: &UpstreamResponse,
    ) -> Result<CanonicalResponse, ProtocolError> {
        Err(ProtocolError::UnsupportedFormat(self.provider_type().to_string()))
    }
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
        assert_eq!(
            err.to_string(),
            "不支持的协议格式: custom"
        );
    }
}
