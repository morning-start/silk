//! Claude Messages协议转换器
//!
//! 支持Claude Messages协议的请求和响应转换。

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::anthropic::request::AnthropicRequest;
use linguafranca::anthropic::response::AnthropicResponse;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

/// Claude Messages协议转换器
pub struct ClaudeMessagesConverter;

impl ClaudeMessagesConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeMessagesConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolConverter for ClaudeMessagesConverter {
    fn name(&self) -> &str {
        "claude_messages"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_response"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_chat", "openai_response"]
    }

    async fn convert_request(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("claude_messages", "claude_messages") => Ok(Bytes::from(body.to_vec())),

            // Claude Messages -> OpenAI Chat (通过hub)
            ("claude_messages", "openai_chat") => {
                let claude_req: AnthropicRequest = serde_json::from_slice(body).map_err(|e| {
                    ConversionError::ParseError(format!("解析Claude请求失败: {e}"))
                })?;

                let open_responses = claude_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let chat_req =
                    linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest::from_open_responses(
                        open_responses.value,
                        None,
                    )
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;

                let result = serde_json::to_vec(&chat_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // Claude Messages -> OpenAI Response (通过hub)
            ("claude_messages", "openai_response") => {
                let claude_req: AnthropicRequest = serde_json::from_slice(body).map_err(|e| {
                    ConversionError::ParseError(format!("解析Claude请求失败: {e}"))
                })?;

                let open_responses = claude_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_responses.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Response -> Claude Messages
            ("openai_response", "claude_messages") => {
                let open_responses: linguafranca::open_responses::request::OpenResponsesRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}"))
                    })?;

                let claude_req =
                    AnthropicRequest::from_open_responses(open_responses, None).map_err(|e| {
                        ConversionError::TransformError(format!("转换失败: {e}"))
                    })?;

                let result = serde_json::to_vec(&claude_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            _ => Err(ConversionError::UnsupportedProtocol(format!(
                "不支持的转换: {from_protocol} -> {to_protocol}"
            ))),
        }
    }

    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("claude_messages", "claude_messages") => Ok(Bytes::from(body.to_vec())),

            // OpenAI Response -> Claude Messages
            ("openai_response", "claude_messages") => {
                let open_responses: linguafranca::open_responses::response::OpenResponsesResponse =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response响应失败: {e}"))
                    })?;

                let claude_resp =
                    AnthropicResponse::from_open_responses(open_responses, None).map_err(|e| {
                        ConversionError::TransformError(format!("转换失败: {e}"))
                    })?;

                let result = serde_json::to_vec(&claude_resp.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // Claude Messages -> OpenAI Response (通过hub)
            ("claude_messages", "openai_response") => {
                let claude_resp: AnthropicResponse = serde_json::from_slice(body).map_err(|e| {
                    ConversionError::ParseError(format!("解析Claude响应失败: {e}"))
                })?;

                let open_responses = claude_resp.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_responses.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            _ => Err(ConversionError::UnsupportedProtocol(format!(
                "不支持的转换: {from_protocol} -> {to_protocol}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_request_same_protocol() {
        let converter = ClaudeMessagesConverter::new();
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "claude_messages",
                "claude_messages",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_openai_chat() {
        let converter = ClaudeMessagesConverter::new();
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "claude_messages",
                "openai_chat",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_openai_response() {
        let converter = ClaudeMessagesConverter::new();
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "claude_messages",
                "openai_response",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_unsupported() {
        let converter = ClaudeMessagesConverter::new();
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "unsupported",
                "claude_messages",
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = ClaudeMessagesConverter::new();
        assert_eq!(converter.name(), "claude_messages");
    }

    #[test]
    fn test_converter_supports() {
        let converter = ClaudeMessagesConverter::new();
        assert!(converter.supports("claude_messages", "claude_messages"));
        assert!(converter.supports("claude_messages", "openai_chat"));
        assert!(converter.supports("claude_messages", "openai_response"));
        assert!(!converter.supports("unsupported", "claude_messages"));
    }
}
