//! OpenAI Chat流式转换器
//!
//! 支持OpenAI Chat Completions协议的SSE流式转换。

use async_trait::async_trait;
use linguafranca::chat_completions_openai::convert::stream::{
    ChatCompletionsToOpenResponsesStream, OpenResponsesToChatCompletionsStream,
};
use linguafranca::chat_completions_openai::stream::ChatCompletionsStreamChunk;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;
use std::sync::Mutex;

use super::converter::{SseEvent, StreamConversionError, StreamConverter};

/// OpenAI Chat流式转换器
pub struct OpenAIChatStreamConverter {
    chat_to_hub: Mutex<ChatCompletionsToOpenResponsesStream>,
    hub_to_chat: Mutex<OpenResponsesToChatCompletionsStream>,
}

impl OpenAIChatStreamConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self {
            chat_to_hub: Mutex::new(ChatCompletionsToOpenResponsesStream::new()),
            hub_to_chat: Mutex::new(OpenResponsesToChatCompletionsStream::new()),
        }
    }
}

impl Default for OpenAIChatStreamConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamConverter for OpenAIChatStreamConverter {
    fn name(&self) -> &str {
        "openai_chat_stream"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "openai_response"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "openai_response"]
    }

    async fn convert_event(
        &self,
        event: &SseEvent,
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Vec<SseEvent>, StreamConversionError> {
        // 忽略非数据事件（注释、心跳等）
        let data = match &event.data {
            Some(d) => d,
            None => return Ok(vec![event.clone()]),
        };

        // 忽略结束标记
        if event.is_end() {
            return Ok(vec![event.clone()]);
        }

        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("openai_chat", "openai_chat") => Ok(vec![event.clone()]),

            // OpenAI Chat -> OpenAI Response (hub)
            ("openai_chat", "openai_response") => {
                let chunk: ChatCompletionsStreamChunk = serde_json::from_str(data).map_err(
                    |e| StreamConversionError::ParseError(format!("解析Chat chunk失败: {e}")),
                )?;

                let hub_events = self.chat_to_hub.lock().unwrap().transform(chunk).map_err(|e| {
                    StreamConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let mut result = Vec::new();
                for hub_event in hub_events {
                    let json_str = serde_json::to_string(&hub_event).map_err(|e| {
                        StreamConversionError::SerializationError(format!("序列化失败: {e}"))
                    })?;

                    let et = serde_json::to_value(&hub_event)
                        .ok()
                        .and_then(|v| {
                            v.get("type").and_then(|t| t.as_str()).map(|s| s.to_string())
                        });

                    let sse_event = SseEvent {
                        event: et,
                        data: Some(json_str),
                        id: None,
                        retry: None,
                        comment: None,
                    };
                    result.push(sse_event);
                }

                Ok(result)
            }

            // OpenAI Response (hub) -> OpenAI Chat
            ("openai_response", "openai_chat") => {
                let hub_event: OpenResponsesStreamEvent = serde_json::from_str(data).map_err(
                    |e| {
                        StreamConversionError::ParseError(format!(
                            "解析OpenResponses event失败: {e}"
                        ))
                    },
                )?;

                let chat_chunks = self.hub_to_chat.lock().unwrap().transform(hub_event).map_err(|e| {
                    StreamConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let mut result = Vec::new();
                for chunk in chat_chunks {
                    let json_str = serde_json::to_string(&chunk).map_err(|e| {
                        StreamConversionError::SerializationError(format!("序列化失败: {e}"))
                    })?;

                    let sse_event = SseEvent {
                        event: None,
                        data: Some(json_str),
                        id: None,
                        retry: None,
                        comment: None,
                    };
                    result.push(sse_event);
                }

                Ok(result)
            }

            // OpenAI Response -> OpenAI Response (同协议)
            ("openai_response", "openai_response") => Ok(vec![event.clone()]),

            _ => Err(StreamConversionError::UnsupportedProtocol(format!(
                "不支持的转换: {from_protocol} -> {to_protocol}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_event_same_protocol() {
        let converter = OpenAIChatStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some(r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "openai_chat", "openai_chat")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    #[ignore = "linguafranca stream transform requires complete stream data"]
    async fn test_convert_event_to_openai_response() {
        let converter = OpenAIChatStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some(r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "openai_chat", "openai_response")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_event_end_marker() {
        let converter = OpenAIChatStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some("[DONE]".to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "openai_chat", "openai_response")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_convert_event_unsupported() {
        let converter = OpenAIChatStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some("{}".to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "unsupported", "openai_chat")
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = OpenAIChatStreamConverter::new();
        assert_eq!(converter.name(), "openai_chat_stream");
    }

    #[test]
    fn test_converter_supports() {
        let converter = OpenAIChatStreamConverter::new();
        assert!(converter.supports("openai_chat", "openai_chat"));
        assert!(converter.supports("openai_chat", "openai_response"));
        assert!(!converter.supports("unsupported", "openai_chat"));
    }
}
