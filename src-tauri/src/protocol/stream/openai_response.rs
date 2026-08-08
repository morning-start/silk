//! OpenAI Response流式转换器
//!
//! 支持OpenAI Responses协议的SSE流式转换。

use async_trait::async_trait;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;

use super::converter::{SseEvent, StreamConversionError, StreamConverter};

/// OpenAI Response流式转换器
pub struct OpenAIResponseStreamConverter;

impl OpenAIResponseStreamConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAIResponseStreamConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamConverter for OpenAIResponseStreamConverter {
    fn name(&self) -> &str {
        "openai_response_stream"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["openai_response", "openai_chat", "claude_messages"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["openai_response", "openai_chat", "claude_messages"]
    }

    async fn convert_event(
        &self,
        event: &SseEvent,
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Vec<SseEvent>, StreamConversionError> {
        // 忽略非数据事件
        let _data = match &event.data {
            Some(d) => d,
            None => return Ok(vec![event.clone()]),
        };

        // 忽略结束标记
        if event.is_end() {
            return Ok(vec![event.clone()]);
        }

        match (from_protocol, to_protocol) {
            // 同协议，直接返回
            ("openai_response", "openai_response") => Ok(vec![event.clone()]),

            // OpenAI Response -> OpenAI Chat (通过其他转换器)
            ("openai_response", "openai_chat") => {
                // 这里需要使用OpenResponsesToChatCompletionsStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }

            // OpenAI Response -> Claude Messages (通过其他转换器)
            ("openai_response", "claude_messages") => {
                // 这里需要使用OpenResponsesToAnthropicMessagesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }

            // OpenAI Chat -> OpenAI Response (通过其他转换器)
            ("openai_chat", "openai_response") => {
                // 这里需要使用ChatCompletionsToOpenResponsesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }

            // Claude Messages -> OpenAI Response (通过其他转换器)
            ("claude_messages", "openai_response") => {
                // 这里需要使用AnthropicMessagesToOpenResponsesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }

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
        let converter = OpenAIResponseStreamConverter::new();
        let event = SseEvent {
            event: Some("response.created".to_string()),
            data: Some(r#"{"type":"response.created","response":{"id":"resp_123","object":"response","created_at":1234567890,"status":"in_progress","model":"gpt-4","output":[]}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "openai_response", "openai_response")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_convert_event_unsupported() {
        let converter = OpenAIResponseStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some("{}".to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "unsupported", "openai_response")
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = OpenAIResponseStreamConverter::new();
        assert_eq!(converter.name(), "openai_response_stream");
    }

    #[test]
    fn test_converter_supports() {
        let converter = OpenAIResponseStreamConverter::new();
        assert!(converter.supports("openai_response", "openai_response"));
        assert!(converter.supports("openai_response", "openai_chat"));
        assert!(converter.supports("openai_response", "claude_messages"));
        assert!(!converter.supports("unsupported", "openai_response"));
    }
}
