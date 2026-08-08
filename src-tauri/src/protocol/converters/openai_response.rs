//! OpenAI Response协议转换器
//!
//! 支持OpenAI Responses协议的请求和响应转换。

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

/// OpenAI Response协议转换器
pub struct OpenAIResponseConverter;

impl OpenAIResponseConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAIResponseConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolConverter for OpenAIResponseConverter {
    fn name(&self) -> &str {
        "openai_response"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["openai_response", "openai_chat", "claude_messages"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["openai_response", "openai_chat", "claude_messages"]
    }

    async fn convert_request(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("openai_response", "openai_response") => Ok(Bytes::from(body.to_vec())),

            // OpenAI Response -> OpenAI Chat
            ("openai_response", "openai_chat") => {
                let open_req: linguafranca::open_responses::request::OpenResponsesRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}"))
                    })?;

                let chat_req =
                    linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest::from_open_responses(
                        open_req,
                        None,
                    )
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;

                let result = serde_json::to_vec(&chat_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Response -> Claude Messages
            ("openai_response", "claude_messages") => {
                let open_req: linguafranca::open_responses::request::OpenResponsesRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}"))
                    })?;

                let claude_req =
                    linguafranca::anthropic::request::AnthropicRequest::from_open_responses(
                        open_req,
                        None,
                    )
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;

                let result = serde_json::to_vec(&claude_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Chat -> OpenAI Response
            ("openai_chat", "openai_response") => {
                let chat_req: linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Chat请求失败: {e}"))
                    })?;

                let open_req = chat_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // Claude Messages -> OpenAI Response
            ("claude_messages", "openai_response") => {
                let claude_req: linguafranca::anthropic::request::AnthropicRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析Claude请求失败: {e}"))
                    })?;

                let open_req = claude_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_req.value).map_err(|e| {
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
            ("openai_response", "openai_response") => Ok(Bytes::from(body.to_vec())),

            // OpenAI Response -> OpenAI Chat
            ("openai_response", "openai_chat") => {
                let open_resp: linguafranca::open_responses::response::OpenResponsesResponse =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response响应失败: {e}"))
                    })?;

                let chat_resp =
                    linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse::from_open_responses(
                        open_resp,
                        None,
                    )
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;

                let result = serde_json::to_vec(&chat_resp.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Chat -> OpenAI Response
            ("openai_chat", "openai_response") => {
                let chat_resp: linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Chat响应失败: {e}"))
                    })?;

                let open_resp = chat_resp.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_resp.value).map_err(|e| {
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
        let converter = OpenAIResponseConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "input": "Hello"
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_response",
                "openai_response",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_openai_chat() {
        let converter = OpenAIResponseConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "input": "Hello"
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_response",
                "openai_chat",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_claude() {
        let converter = OpenAIResponseConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "input": "Hello"
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_response",
                "claude_messages",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_unsupported() {
        let converter = OpenAIResponseConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "input": "Hello"
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "unsupported",
                "openai_response",
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = OpenAIResponseConverter::new();
        assert_eq!(converter.name(), "openai_response");
    }

    #[test]
    fn test_converter_supports() {
        let converter = OpenAIResponseConverter::new();
        assert!(converter.supports("openai_response", "openai_response"));
        assert!(converter.supports("openai_response", "openai_chat"));
        assert!(converter.supports("openai_response", "claude_messages"));
        assert!(!converter.supports("unsupported", "openai_response"));
    }
}
