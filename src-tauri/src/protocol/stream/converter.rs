//! SSE流式转换抽象层
//!
//! 定义SSE事件和流式转换器的核心接口。
//! 所有流式转换器必须是无状态的，仅处理SSE事件流。

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// SSE事件
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// 事件类型（event: xxx）
    pub event: Option<String>,
    /// 数据字段（data: xxx）
    pub data: Option<String>,
    /// 事件ID
    pub id: Option<String>,
    /// 重试间隔（毫秒）
    pub retry: Option<u64>,
    /// 注释（:keep-alive等）
    pub comment: Option<String>,
}

impl SseEvent {
    /// 序列化为SSE格式文本
    pub fn serialize(&self) -> String {
        let mut output = String::new();

        if let Some(ref comment) = self.comment {
            output.push_str(&format!(": {comment}\n"));
        }
        if let Some(ref id) = self.id {
            output.push_str(&format!("id: {id}\n"));
        }
        if let Some(ref event) = self.event {
            output.push_str(&format!("event: {event}\n"));
        }
        if let Some(ref retry) = self.retry {
            output.push_str(&format!("retry: {retry}\n"));
        }
        if let Some(ref data) = self.data {
            for line in data.lines() {
                output.push_str(&format!("data: {line}\n"));
            }
        }

        if !output.is_empty() {
            output.push('\n');
        }
        output
    }

    /// 是否为流结束标记
    pub fn is_end(&self) -> bool {
        self.data.as_deref() == Some("[DONE]")
    }
}

/// SSE解析器：将字节流解析为SseEvent
pub struct SseParser {
    buffer: bytes::BytesMut,
}

impl SseParser {
    /// 创建新的解析器
    pub fn new() -> Self {
        Self {
            buffer: bytes::BytesMut::new(),
        }
    }

    /// 喂入数据块，返回解析出的事件
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);

        // 防止buffer无限增长：超过1MB时截断
        const MAX_BUFFER_SIZE: usize = 1024 * 1024;
        if self.buffer.len() > MAX_BUFFER_SIZE {
            let split_idx = self.buffer.len() - MAX_BUFFER_SIZE / 2;
            let _ = self.buffer.split_to(split_idx);
        }

        let mut events = Vec::new();

        while let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            let raw_bytes = self.buffer.split_to(pos);
            let _ = self.buffer.split_to(2); // Skip the \n\n

            let raw_str = String::from_utf8_lossy(&raw_bytes);
            if let Some(event) = Self::parse_event(&raw_str) {
                events.push(event);
            }
        }

        events
    }

    fn parse_event(raw: &str) -> Option<SseEvent> {
        let mut event = SseEvent {
            event: None,
            data: None,
            id: None,
            retry: None,
            comment: None,
        };

        let mut has_data = false;

        for line in raw.lines() {
            if line.starts_with(':') {
                // SSE规范：注释以:开头，可选空格
                let comment = line.strip_prefix(": ").or_else(|| line.strip_prefix(':'));
                event.comment = comment.map(|s| s.to_string());
            } else if let Some(rest) = line
                .strip_prefix("event:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                event.event = Some(rest.to_string());
                has_data = true;
            } else if let Some(rest) = line
                .strip_prefix("data:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE规范：多个data:字段用\n拼接
                event.data = match event.data {
                    Some(existing) => Some(format!("{existing}\n{rest}")),
                    None => Some(rest.to_string()),
                };
                has_data = true;
            } else if let Some(rest) = line
                .strip_prefix("id:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE规范：id字段包含null字符时应忽略
                if !rest.contains('\0') {
                    event.id = Some(rest.to_string());
                    has_data = true;
                }
            } else if let Some(rest) = line
                .strip_prefix("retry:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                event.retry = rest.parse().ok();
                has_data = true;
            }
        }

        if has_data {
            Some(event)
        } else {
            None
        }
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 流式转换错误
#[derive(Error, Debug)]
pub enum StreamConversionError {
    #[error("解析失败: {0}")]
    ParseError(String),

    #[error("转换失败: {0}")]
    TransformError(String),

    #[error("序列化失败: {0}")]
    SerializationError(String),

    #[error("不支持的协议: {0}")]
    UnsupportedProtocol(String),

    #[error("流超时")]
    Timeout,

    #[error("内部错误: {0}")]
    InternalError(String),
}

/// 流式协议转换器trait
///
/// 所有流式转换器必须实现此接口。
/// 转换器是无状态的，每次转换都是独立的。
#[async_trait]
pub trait StreamConverter: Send + Sync {
    /// 转换器名称
    fn name(&self) -> &str;

    /// 支持的源协议
    fn source_protocols(&self) -> Vec<&str>;

    /// 支持的目标协议
    fn target_protocols(&self) -> Vec<&str>;

    /// 检查是否支持指定的协议转换
    fn supports(&self, from: &str, to: &str) -> bool {
        self.source_protocols().contains(&from) && self.target_protocols().contains(&to)
    }

    /// 转换SSE事件
    ///
    /// # Arguments
    /// * `event` - 原始SSE事件
    /// * `from_protocol` - 源协议名称
    /// * `to_protocol` - 目标协议名称
    ///
    /// # Returns
    /// 转换后的SSE事件列表（一个事件可能转换为多个事件）
    async fn convert_event(
        &self,
        event: &SseEvent,
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Vec<SseEvent>, StreamConversionError>;

    /// 转换SSE事件流（批量处理）
    async fn convert_events(
        &self,
        events: &[SseEvent],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Vec<SseEvent>, StreamConversionError> {
        let mut result = Vec::new();
        for event in events {
            let converted = self.convert_event(event, from_protocol, to_protocol).await?;
            result.extend(converted);
        }
        Ok(result)
    }
}

/// 流式转换器注册表
///
/// 管理所有已注册的流式转换器，支持按名称查找和按协议匹配。
pub struct StreamConverterRegistry {
    converters: HashMap<String, Arc<dyn StreamConverter>>,
}

impl StreamConverterRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            converters: HashMap::new(),
        }
    }

    /// 注册转换器
    pub fn register(&mut self, converter: Arc<dyn StreamConverter>) {
        let name = converter.name().to_string();
        self.converters.insert(name, converter);
    }

    /// 根据名称获取转换器
    pub fn get(&self, name: &str) -> Option<&Arc<dyn StreamConverter>> {
        self.converters.get(name)
    }

    /// 查找支持指定协议转换的转换器
    pub fn find_converter(&self, from: &str, to: &str) -> Option<&Arc<dyn StreamConverter>> {
        self.converters.values().find(|c| c.supports(from, to))
    }

    /// 获取所有转换器名称
    pub fn list_converters(&self) -> Vec<&str> {
        self.converters.keys().map(|s| s.as_str()).collect()
    }

    /// 获取转换器数量
    pub fn len(&self) -> usize {
        self.converters.len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.converters.is_empty()
    }
}

impl Default for StreamConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockStreamConverter {
        name: String,
    }

    impl MockStreamConverter {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl StreamConverter for MockStreamConverter {
        fn name(&self) -> &str {
            &self.name
        }

        fn source_protocols(&self) -> Vec<&str> {
            vec!["openai_chat"]
        }

        fn target_protocols(&self) -> Vec<&str> {
            vec!["claude_messages"]
        }

        async fn convert_event(
            &self,
            event: &SseEvent,
            _from: &str,
            _to: &str,
        ) -> Result<Vec<SseEvent>, StreamConversionError> {
            Ok(vec![event.clone()])
        }
    }

    #[test]
    fn test_sse_event_serialize() {
        let event = SseEvent {
            event: Some("message".to_string()),
            data: Some("hello world".to_string()),
            id: Some("123".to_string()),
            retry: Some(3000),
            comment: None,
        };
        let serialized = event.serialize();
        assert!(serialized.contains("event: message"));
        assert!(serialized.contains("data: hello world"));
        assert!(serialized.contains("id: 123"));
        assert!(serialized.contains("retry: 3000"));
    }

    #[test]
    fn test_sse_event_is_end() {
        let end_event = SseEvent {
            event: None,
            data: Some("[DONE]".to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert!(end_event.is_end());

        let normal_event = SseEvent {
            event: None,
            data: Some("hello".to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert!(!normal_event.is_end());
    }

    #[test]
    fn test_sse_parser_basic() {
        let mut parser = SseParser::new();
        let input = "event: message\ndata: hello\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("message".to_string()));
        assert_eq!(events[0].data, Some("hello".to_string()));
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();
        let input = "data: line1\ndata: line2\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, Some("line1\nline2".to_string()));
    }

    #[test]
    fn test_sse_parser_multiple_events() {
        let mut parser = SseParser::new();
        let input = "data: first\n\ndata: second\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, Some("first".to_string()));
        assert_eq!(events[1].data, Some("second".to_string()));
    }

    #[test]
    fn test_sse_parser_incremental() {
        let mut parser = SseParser::new();
        let events1 = parser.feed(b"data: hello");
        assert_eq!(events1.len(), 0);
        let events2 = parser.feed(b"\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, Some("hello".to_string()));
    }

    #[test]
    fn test_stream_converter_registry_creation() {
        let registry = StreamConverterRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_stream_converter_registry_register() {
        let mut registry = StreamConverterRegistry::new();
        let converter = Arc::new(MockStreamConverter::new("test"));
        registry.register(converter);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_stream_converter_registry_get() {
        let mut registry = StreamConverterRegistry::new();
        let converter = Arc::new(MockStreamConverter::new("test"));
        registry.register(converter);

        let found = registry.get("test");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "test");

        let not_found = registry.get("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_stream_converter_registry_find_converter() {
        let mut registry = StreamConverterRegistry::new();
        let converter = Arc::new(MockStreamConverter::new("test"));
        registry.register(converter);

        let found = registry.find_converter("openai_chat", "claude_messages");
        assert!(found.is_some());

        let not_found = registry.find_converter("unsupported", "openai_chat");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_stream_converter_convert_event() {
        let converter = MockStreamConverter::new("test");
        let event = SseEvent {
            event: None,
            data: Some("hello".to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let result = converter
            .convert_event(&event, "openai_chat", "claude_messages")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
