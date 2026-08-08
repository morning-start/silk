//! OpenAI Chat协议转换器
//!
//! 支持OpenAI Chat Completions协议的请求和响应转换。

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

/// OpenAI Chat协议转换器
pub struct OpenAIChatConverter;

impl OpenAIChatConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAIChatConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolConverter for OpenAIChatConverter {
    fn name(&self) -> &str {
        "openai_chat"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "openai_response"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "claude_messages", "openai_response"]
    }

    async fn convert_request(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("openai_chat", "openai_chat") => Ok(Bytes::from(body.to_vec())),

            // OpenAI Chat -> OpenAI Response (通过hub)
            ("openai_chat", "openai_response") => {
                let chat_req: ChatCompletionsOpenAiRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Chat请求失败: {e}"))
                    })?;

                let open_responses = chat_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let result = serde_json::to_vec(&open_responses.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Chat -> Claude Messages (通过hub)
            ("openai_chat", "claude_messages") => {
                let chat_req: ChatCompletionsOpenAiRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Chat请求失败: {e}"))
                    })?;

                // 先转为hub格式
                let open_responses = chat_req.into_open_responses(None).map_err(|e| {
                    ConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                // 再从hub转为Claude格式
                let claude_req =
                    linguafranca::anthropic::request::AnthropicRequest::from_open_responses(
                        open_responses.value,
                        None,
                    )
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;

                let result = serde_json::to_vec(&claude_req.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Response -> OpenAI Chat
            ("openai_response", "openai_chat") => {
                let open_responses: linguafranca::open_responses::request::OpenResponsesRequest =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}"))
                    })?;

                let chat_req =
                    ChatCompletionsOpenAiRequest::from_open_responses(open_responses, None)
                        .map_err(|e| {
                            ConversionError::TransformError(format!("转换失败: {e}"))
                        })?;

                let result = serde_json::to_vec(&chat_req.value).map_err(|e| {
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
            ("openai_chat", "openai_chat") => Ok(Bytes::from(body.to_vec())),

            // OpenAI Response -> OpenAI Chat
            ("openai_response", "openai_chat") => {
                let open_responses: linguafranca::open_responses::response::OpenResponsesResponse =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Response响应失败: {e}"))
                    })?;

                let chat_resp =
                    ChatCompletionsOpenAiResponse::from_open_responses(open_responses, None)
                        .map_err(|e| {
                            ConversionError::TransformError(format!("转换失败: {e}"))
                        })?;

                let result = serde_json::to_vec(&chat_resp.value).map_err(|e| {
                    ConversionError::SerializationError(format!("序列化失败: {e}"))
                })?;

                Ok(Bytes::from(result))
            }

            // OpenAI Chat -> OpenAI Response
            ("openai_chat", "openai_response") => {
                let chat_resp: ChatCompletionsOpenAiResponse =
                    serde_json::from_slice(body).map_err(|e| {
                        ConversionError::ParseError(format!("解析OpenAI Chat响应失败: {e}"))
                    })?;

                let open_responses = chat_resp.into_open_responses(None).map_err(|e| {
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
        let converter = OpenAIChatConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_chat",
                "openai_chat",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_openai_response() {
        let converter = OpenAIChatConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_chat",
                "openai_response",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_to_claude() {
        let converter = OpenAIChatConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "openai_chat",
                "claude_messages",
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_request_unsupported() {
        let converter = OpenAIChatConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let result = converter
            .convert_request(
                &serde_json::to_vec(&body).unwrap(),
                "unsupported",
                "openai_chat",
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = OpenAIChatConverter::new();
        assert_eq!(converter.name(), "openai_chat");
    }

    #[test]
    fn test_converter_supports() {
        let converter = OpenAIChatConverter::new();
        assert!(converter.supports("openai_chat", "openai_chat"));
        assert!(converter.supports("openai_chat", "openai_response"));
        assert!(converter.supports("openai_chat", "claude_messages"));
        assert!(!converter.supports("unsupported", "openai_chat"));
    }
}
