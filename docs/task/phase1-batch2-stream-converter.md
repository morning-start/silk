# Phase 1 - 批次2：流式处理层独立化

## 📋 批次概述

**批次目标**：将SSE流式转换逻辑从`stream_response.rs`剥离，创建独立的流式转换层

**预计工期**：1周（5个工作日）

**依赖关系**：批次1完成（需要协议转换抽象层）

---

## 🎯 批次目标

1. 创建SSE流式转换抽象接口
2. 实现OpenAI Chat流式转换器
3. 实现Claude Messages流式转换器
4. 实现OpenAI Response流式转换器

---

## 📝 任务清单

### 任务2.1：创建SSE流式转换抽象层

**文件路径**：`src-tauri/src/protocol/stream/converter.rs`

**任务描述**：
定义SSE流式转换器的核心接口，支持流式字节流处理

**实现步骤**：

1. 创建SSE事件类型
```rust
// src-tauri/src/protocol/stream/converter.rs

use bytes::Bytes;
use std::time::Duration;

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
```

2. 创建SSE解析器
```rust
/// SSE解析器：将字节流解析为SseEvent
pub struct SseParser {
    buffer: bytes::BytesMut,
}

impl SseParser {
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
            } else if let Some(rest) = line.strip_prefix("event:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                event.event = Some(rest.to_string());
                has_data = true;
            } else if let Some(rest) = line.strip_prefix("data:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE规范：多个data:字段用\n拼接
                event.data = match event.data {
                    Some(existing) => Some(format!("{existing}\n{rest}")),
                    None => Some(rest.to_string()),
                };
                has_data = true;
            } else if let Some(rest) = line.strip_prefix("id:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE规范：id字段包含null字符时应忽略
                if !rest.contains('\0') {
                    event.id = Some(rest.to_string());
                    has_data = true;
                }
            } else if let Some(rest) = line.strip_prefix("retry:")
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
```

3. 定义流式转换错误类型
```rust
use thiserror::Error;

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
```

4. 定义流式转换器trait
```rust
use async_trait::async_trait;

/// 流式协议转换器trait
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
        self.source_protocols().contains(&from) && 
        self.target_protocols().contains(&to)
    }
    
    /// 转换SSE事件
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
```

5. 创建流式转换器注册表
```rust
use std::collections::HashMap;
use std::sync::Arc;

/// 流式转换器注册表
pub struct StreamConverterRegistry {
    converters: HashMap<String, Arc<dyn StreamConverter>>,
}

impl StreamConverterRegistry {
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
    
    /// 获取转换器
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
}

impl Default for StreamConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

6. 更新`src-tauri/src/protocol/mod.rs`，添加stream模块
```rust
// src-tauri/src/protocol/mod.rs

pub mod adapter;
pub mod adapters;
pub mod builtin_adapters;
pub mod converter;
pub mod registry;
pub mod stream;  // 新增

pub use adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
pub use converter::{ConversionError, ConverterRegistry, ProtocolConverter};
pub use registry::AdapterRegistry;
pub use stream::{
    SseEvent, SseParser, StreamConversionError, StreamConverter, StreamConverterRegistry,
};
```

**验收标准**：
- [ ] SseEvent类型定义完整
- [ ] SseParser功能正常
- [ ] StreamConverter trait定义清晰
- [ ] StreamConverterRegistry功能完整
- [ ] 代码通过cargo clippy检查
- [ ] 单元测试通过

---

### 任务2.2：实现OpenAI Chat流式转换器

**文件路径**：`src-tauri/src/protocol/stream/openai_chat.rs`

**任务描述**：
从`stream_response.rs`剥离OpenAI Chat SSE转换逻辑

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/stream/openai_chat.rs

use async_trait::async_trait;
use linguafranca::chat_completions_openai::convert::stream::{
    ChatCompletionsToOpenResponsesStream, OpenResponsesToChatCompletionsStream,
};
use linguafranca::chat_completions_openai::stream::ChatCompletionsStreamChunk;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;

use crate::protocol::stream::{
    SseEvent, StreamConversionError, StreamConverter,
};

pub struct OpenAIChatStreamConverter {
    chat_to_hub: ChatCompletionsToOpenResponsesStream,
    hub_to_chat: OpenResponsesToChatCompletionsStream,
}

impl OpenAIChatStreamConverter {
    pub fn new() -> Self {
        Self {
            chat_to_hub: ChatCompletionsToOpenResponsesStream::new(),
            hub_to_chat: OpenResponsesToChatCompletionsStream::new(),
        }
    }
}
```

2. 实现StreamConverter trait
```rust
#[async_trait]
impl StreamConverter for OpenAIChatStreamConverter {
    fn name(&self) -> &str {
        "openai_chat_stream"
    }
    
    fn source_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "openai_response"]
    }
    
    fn target_protocols(&self) -> Vec<&str> {
        vec!["openai_chat", "claude_messages", "openai_response"]
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
            ("openai_chat", "openai_chat") => {
                // 同协议，直接返回
                Ok(vec![event.clone()])
            }
            ("openai_chat", "openai_response") => {
                // OpenAI Chat -> OpenAI Response (hub)
                let chunk: ChatCompletionsStreamChunk = serde_json::from_str(data)
                    .map_err(|e| StreamConversionError::ParseError(format!("解析Chat chunk失败: {e}")))?;
                
                let hub_events = self.chat_to_hub.transform(chunk)
                    .map_err(|e| StreamConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let mut result = Vec::new();
                for hub_event in hub_events {
                    let json_str = serde_json::to_string(&hub_event)
                        .map_err(|e| StreamConversionError::SerializationError(format!("序列化失败: {e}")))?;
                    
                    let et = serde_json::to_value(&hub_event)
                        .ok()
                        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()));
                    
                    let mut sse_event = SseEvent {
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
            ("openai_response", "openai_chat") => {
                // OpenAI Response (hub) -> OpenAI Chat
                let hub_event: OpenResponsesStreamEvent = serde_json::from_str(data)
                    .map_err(|e| StreamConversionError::ParseError(format!("解析OpenResponses event失败: {e}")))?;
                
                let chat_chunks = self.hub_to_chat.transform(hub_event)
                    .map_err(|e| StreamConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let mut result = Vec::new();
                for chunk in chat_chunks {
                    let json_str = serde_json::to_string(&chunk)
                        .map_err(|e| StreamConversionError::SerializationError(format!("序列化失败: {e}")))?;
                    
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
            ("openai_response", "openai_response") => {
                // 同协议，直接返回
                Ok(vec![event.clone()])
            }
            _ => Err(StreamConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
}
```

3. 添加单元测试
```rust
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
        
        let result = converter.convert_event(&event, "openai_chat", "openai_chat").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
    
    #[tokio::test]
    async fn test_convert_event_to_openai_response() {
        let converter = OpenAIChatStreamConverter::new();
        let event = SseEvent {
            event: None,
            data: Some(r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        
        let result = converter.convert_event(&event, "openai_chat", "openai_response").await;
        assert!(result.is_ok());
    }
}
```

**验收标准**：
- [ ] OpenAIChatStreamConverter实现完整
- [ ] 支持OpenAI Chat -> OpenAI Response流式转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务2.3：实现Claude Messages流式转换器

**文件路径**：`src-tauri/src/protocol/stream/claude_messages.rs`

**任务描述**：
实现Claude Messages协议的流式转换器

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/stream/claude_messages.rs

use async_trait::async_trait;
use linguafranca::anthropic::convert::stream::{
    AnthropicMessagesToOpenResponsesStream, OpenResponsesToAnthropicMessagesStream,
};
use linguafranca::anthropic::stream::AnthropicStreamEvent;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;

use crate::protocol::stream::{
    SseEvent, StreamConversionError, StreamConverter,
};

pub struct ClaudeMessagesStreamConverter {
    claude_to_hub: AnthropicMessagesToOpenResponsesStream,
    hub_to_claude: OpenResponsesToAnthropicMessagesStream,
}

impl ClaudeMessagesStreamConverter {
    pub fn new() -> Self {
        Self {
            claude_to_hub: AnthropicMessagesToOpenResponsesStream::new(),
            hub_to_claude: OpenResponsesToAnthropicMessagesStream::new(),
        }
    }
}
```

2. 实现StreamConverter trait
```rust
#[async_trait]
impl StreamConverter for ClaudeMessagesStreamConverter {
    fn name(&self) -> &str {
        "claude_messages_stream"
    }
    
    fn source_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_response"]
    }
    
    fn target_protocols(&self) -> Vec<&str> {
        vec!["claude_messages", "openai_chat", "openai_response"]
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
        
        if event.is_end() {
            return Ok(vec![event.clone()]);
        }
        
        match (from_protocol, to_protocol) {
            ("claude_messages", "claude_messages") => {
                Ok(vec![event.clone()])
            }
            ("claude_messages", "openai_response") => {
                let claude_event: AnthropicStreamEvent = serde_json::from_str(data)
                    .map_err(|e| StreamConversionError::ParseError(format!("解析Claude event失败: {e}")))?;
                
                let hub_events = self.claude_to_hub.transform(claude_event)
                    .map_err(|e| StreamConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let mut result = Vec::new();
                for hub_event in hub_events {
                    let json_str = serde_json::to_string(&hub_event)
                        .map_err(|e| StreamConversionError::SerializationError(format!("序列化失败: {e}")))?;
                    
                    let et = serde_json::to_value(&hub_event)
                        .ok()
                        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()));
                    
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
            ("openai_response", "claude_messages") => {
                let hub_event: OpenResponsesStreamEvent = serde_json::from_str(data)
                    .map_err(|e| StreamConversionError::ParseError(format!("解析OpenResponses event失败: {e}")))?;
                
                let claude_events = self.hub_to_claude.transform(hub_event)
                    .map_err(|e| StreamConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let mut result = Vec::new();
                for claude_event in claude_events {
                    let json_str = serde_json::to_string(&claude_event)
                        .map_err(|e| StreamConversionError::SerializationError(format!("序列化失败: {e}")))?;
                    
                    let et = serde_json::to_value(&claude_event)
                        .ok()
                        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()));
                    
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
            ("openai_response", "openai_response") => {
                Ok(vec![event.clone()])
            }
            _ => Err(StreamConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
}
```

3. 添加单元测试
```rust
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
        
        let result = converter.convert_event(&event, "claude_messages", "claude_messages").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
```

**验收标准**：
- [ ] ClaudeMessagesStreamConverter实现完整
- [ ] 支持Claude Messages -> OpenAI Response流式转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务2.4：实现OpenAI Response流式转换器

**文件路径**：`src-tauri/src/protocol/stream/openai_response.rs`

**任务描述**：
实现OpenAI Response协议的流式转换器

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/stream/openai_response.rs

use async_trait::async_trait;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;

use crate::protocol::stream::{
    SseEvent, StreamConversionError, StreamConverter,
};

pub struct OpenAIResponseStreamConverter;

impl OpenAIResponseStreamConverter {
    pub fn new() -> Self {
        Self
    }
}
```

2. 实现StreamConverter trait
```rust
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
        match (from_protocol, to_protocol) {
            ("openai_response", "openai_response") => {
                Ok(vec![event.clone()])
            }
            ("openai_response", "openai_chat") => {
                // 这里需要使用OpenResponsesToChatCompletionsStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }
            ("openai_response", "claude_messages") => {
                // 这里需要使用OpenResponsesToAnthropicMessagesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }
            ("openai_chat", "openai_response") => {
                // 这里需要使用ChatCompletionsToOpenResponsesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }
            ("claude_messages", "openai_response") => {
                // 这里需要使用AnthropicMessagesToOpenResponsesStream
                // 暂时返回原始事件，后续完善
                Ok(vec![event.clone()])
            }
            _ => Err(StreamConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
}
```

3. 添加单元测试
```rust
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
        
        let result = converter.convert_event(&event, "openai_response", "openai_response").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
```

**验收标准**：
- [ ] OpenAIResponseStreamConverter实现完整
- [ ] 支持OpenAI Response协议转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

## 📦 批次交付物

1. `src-tauri/src/protocol/stream/converter.rs` - SSE流式转换抽象层
2. `src-tauri/src/protocol/stream/openai_chat.rs` - OpenAI Chat流式转换器
3. `src-tauri/src/protocol/stream/claude_messages.rs` - Claude Messages流式转换器
4. `src-tauri/src/protocol/stream/openai_response.rs` - OpenAI Response流式转换器
5. 更新的`src-tauri/src/protocol/mod.rs` - 模块导出

---

## ✅ 批次验收标准

- [ ] 所有流式转换器实现完整
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查
- [ ] 无编译错误
- [ ] 文档注释完整

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务2.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务2.2 | 1.5天 | 第2天下午 | 第3天 |
| 任务2.3 | 1天 | 第4天 | 第4天 |
| 任务2.4 | 1天 | 第5天 | 第5天 |
