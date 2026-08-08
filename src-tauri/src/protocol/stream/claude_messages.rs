//! Claude Messages流式转换器
//!
//! 支持Claude Messages协议的SSE流式转换。

use async_trait::async_trait;
use linguafranca::anthropic::convert::stream::{
    AnthropicMessagesToOpenResponsesStream, OpenResponsesToAnthropicMessagesStream,
};
use linguafranca::anthropic::stream::AnthropicStreamEvent;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;
use std::sync::Mutex;

use super::converter::{SseEvent, StreamConversionError, StreamConverter};

/// Claude Messages流式转换器
pub struct ClaudeMessagesStreamConverter {
    claude_to_hub: Mutex<AnthropicMessagesToOpenResponsesStream>,
    hub_to_claude: Mutex<OpenResponsesToAnthropicMessagesStream>,
}

impl ClaudeMessagesStreamConverter {
    /// 创建新的转换器实例
    pub fn new() -> Self {
        Self {
            claude_to_hub: Mutex::new(AnthropicMessagesToOpenResponsesStream::new()),
            hub_to_claude: Mutex::new(OpenResponsesToAnthropicMessagesStream::new()),
        }
    }
}

impl Default for ClaudeMessagesStreamConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamConverter for ClaudeMessagesStreamConverter {
    fn name(&self) -> &str {
        "claude_messages_stream"
    }

    fn source_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_response"]
    }

    fn target_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_response"]
    }

    async fn convert_event(
        &self,
        event: &SseEvent,
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Vec<SseEvent>, StreamConversionError> {
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
            ("claude_messages", "claude_messages") => Ok(vec![event.clone()]),

            // Claude Messages -> OpenAI Response (hub)
            ("claude_messages", "openai_response") => {
                let claude_event: AnthropicStreamEvent = serde_json::from_str(data).map_err(
                    |e| StreamConversionError::ParseError(format!("解析Claude event失败: {e}")),
                )?;

                let hub_events = self.claude_to_hub.lock().unwrap().transform(claude_event).map_err(|e| {
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

            // OpenAI Response (hub) -> Claude Messages
            ("openai_response", "claude_messages") => {
                let hub_event: OpenResponsesStreamEvent = serde_json::from_str(data).map_err(
                    |e| {
                        StreamConversionError::ParseError(format!(
                            "解析OpenResponses event失败: {e}"
                        ))
                    },
                )?;

                let claude_events = self.hub_to_claude.lock().unwrap().transform(hub_event).map_err(|e| {
                    StreamConversionError::TransformError(format!("转换失败: {e}"))
                })?;

                let mut result = Vec::new();
                for claude_event in claude_events {
                    let json_str = serde_json::to_string(&claude_event).map_err(|e| {
                        StreamConversionError::SerializationError(format!("序列化失败: {e}"))
                    })?;

                    let et = serde_json::to_value(&claude_event)
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
        let converter = ClaudeMessagesStreamConverter::new();
        let event = SseEvent {
            event: Some("message_start".to_string()),
            data: Some(r#"{"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-3-opus-20240229","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":25,"output_tokens":1}}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "claude_messages", "claude_messages")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_convert_event_to_openai_response() {
        let converter = ClaudeMessagesStreamConverter::new();
        let event = SseEvent {
            event: Some("message_start".to_string()),
            data: Some(r#"{"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-3-opus-20240229","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":25,"output_tokens":1}}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "claude_messages", "openai_response")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_convert_event_unsupported() {
        let converter = ClaudeMessagesStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some("{}".to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "unsupported", "claude_messages")
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_converter_name() {
        let converter = ClaudeMessagesStreamConverter::new();
        assert_eq!(converter.name(), "claude_messages_stream");
    }

    #[test]
    fn test_converter_supports() {
        let converter = ClaudeMessagesStreamConverter::new();
        assert!(converter.supports("claude_messages", "claude_messages"));
        assert!(converter.supports("claude_messages", "openai_response"));
        assert!(!converter.supports("unsupported", "claude_messages"));
    }
}
