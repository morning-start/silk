# Phase 1 - 批次1：协议转换核心模块创建

## 📋 批次概述

**批次目标**：创建协议转换抽象层和三个核心转换器

**预计工期**：1周（5个工作日）

**依赖关系**：无前置依赖

---

## 🎯 批次目标

1. 创建协议转换器抽象接口
2. 实现OpenAI Chat转换器
3. 实现Claude Messages转换器
4. 实现OpenAI Response转换器

---

## 📝 任务清单

### 任务1.1：创建协议转换抽象层

**文件路径**：`src-tauri/src/protocol/converter.rs`

**任务描述**：
定义协议转换器的核心接口，支持流式和非流式转换

**实现步骤**：

1. 创建转换错误类型
```rust
// src-tauri/src/protocol/converter.rs

use bytes::Bytes;
use thiserror::Error;

/// 协议转换错误
#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("解析失败: {0}")]
    ParseError(String),
    
    #[error("转换失败: {0}")]
    TransformError(String),
    
    #[error("序列化失败: {0}")]
    SerializationError(String),
    
    #[error("不支持的协议: {0}")]
    UnsupportedProtocol(String),
    
    #[error("内部错误: {0}")]
    InternalError(String),
}
```

2. 定义转换器trait
```rust
use async_trait::async_trait;

/// 协议转换器trait
#[async_trait]
pub trait ProtocolConverter: Send + Sync {
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
    
    /// 转换请求体
    async fn convert_request(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError>;
    
    /// 转换响应体
    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError>;
}
```

3. 定义转换器注册表
```rust
use std::collections::HashMap;
use std::sync::Arc;

/// 协议转换器注册表
pub struct ConverterRegistry {
    converters: HashMap<String, Arc<dyn ProtocolConverter>>,
}

impl ConverterRegistry {
    pub fn new() -> Self {
        Self {
            converters: HashMap::new(),
        }
    }
    
    /// 注册转换器
    pub fn register(&mut self, converter: Arc<dyn ProtocolConverter>) {
        let name = converter.name().to_string();
        self.converters.insert(name, converter);
    }
    
    /// 获取转换器
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ProtocolConverter>> {
        self.converters.get(name)
    }
    
    /// 查找支持指定协议转换的转换器
    pub fn find_converter(&self, from: &str, to: &str) -> Option<&Arc<dyn ProtocolConverter>> {
        self.converters.values().find(|c| c.supports(from, to))
    }
    
    /// 获取所有转换器名称
    pub fn list_converters(&self) -> Vec<&str> {
        self.converters.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

4. 更新`src-tauri/src/protocol/mod.rs`，添加converter模块
```rust
// src-tauri/src/protocol/mod.rs

pub mod adapter;
pub mod adapters;
pub mod builtin_adapters;
pub mod converter;  // 新增
pub mod registry;

pub use adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
pub use converter::{ConversionError, ConverterRegistry, ProtocolConverter};
pub use registry::AdapterRegistry;
```

**验收标准**：
- [ ] ConversionError类型定义完整
- [ ] ProtocolConverter trait定义清晰
- [ ] ConverterRegistry功能完整
- [ ] 代码通过cargo clippy检查
- [ ] 单元测试通过

---

### 任务1.2：实现OpenAI Chat转换器

**文件路径**：`src-tauri/src/protocol/converters/openai_chat.rs`

**任务描述**：
从`transform_request.rs`剥离OpenAI Chat协议转换逻辑，实现独立的转换器

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/converters/openai_chat.rs

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

pub struct OpenAIChatConverter;

impl OpenAIChatConverter {
    pub fn new() -> Self {
        Self
    }
}
```

2. 实现ProtocolConverter trait
```rust
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
            ("openai_chat", "openai_chat") => {
                // 同协议，直接返回
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_chat", "claude_messages") => {
                // OpenAI Chat -> Claude Messages
                let chat_req: ChatCompletionsOpenAiRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Chat请求失败: {e}")))?;
                
                let open_responses = chat_req.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                // 这里需要调用linguafranca的Claude转换
                // 暂时返回原始请求，后续完善
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_chat", "openai_response") => {
                // OpenAI Chat -> OpenAI Response
                let chat_req: ChatCompletionsOpenAiRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Chat请求失败: {e}")))?;
                
                let open_responses = chat_req.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&open_responses.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
    
    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            ("openai_chat", "openai_chat") => {
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_response", "openai_chat") => {
                // OpenAI Response -> OpenAI Chat
                let open_responses: linguafranca::open_responses::response::OpenResponsesResponse = 
                    serde_json::from_slice(body)
                        .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Response失败: {e}")))?;
                
                let chat_resp = ChatCompletionsOpenAiResponse::from_open_responses(open_responses, None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&chat_resp.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
}

impl Default for OpenAIChatConverter {
    fn default() -> Self {
        Self::new()
    }
}
```

3. 添加单元测试
```rust
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
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "openai_chat",
            "openai_chat",
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_convert_request_to_openai_response() {
        let converter = OpenAIChatConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "openai_chat",
            "openai_response",
        ).await;
        
        assert!(result.is_ok());
    }
}
```

**验收标准**：
- [ ] OpenAIChatConverter实现完整
- [ ] 支持OpenAI Chat -> OpenAI Response转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务1.3：实现Claude Messages转换器

**文件路径**：`src-tauri/src/protocol/converters/claude_messages.rs`

**任务描述**：
实现Claude Messages协议的转换器

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/converters/claude_messages.rs

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::anthropic::request::AnthropicRequest;
use linguafranca::anthropic::response::AnthropicResponse;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

pub struct ClaudeMessagesConverter;

impl ClaudeMessagesConverter {
    pub fn new() -> Self {
        Self
    }
}
```

2. 实现ProtocolConverter trait
```rust
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
            ("claude_messages", "claude_messages") => {
                Ok(Bytes::from(body.to_vec()))
            }
            ("claude_messages", "openai_chat") => {
                let claude_req: AnthropicRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析Claude请求失败: {e}")))?;
                
                let open_responses = claude_req.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&open_responses.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            ("claude_messages", "openai_response") => {
                let claude_req: AnthropicRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析Claude请求失败: {e}")))?;
                
                let open_responses = claude_req.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&open_responses.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
    
    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            ("claude_messages", "claude_messages") => {
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_response", "claude_messages") => {
                let open_responses: linguafranca::open_responses::response::OpenResponsesResponse = 
                    serde_json::from_slice(body)
                        .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Response失败: {e}")))?;
                
                let claude_resp = AnthropicResponse::from_open_responses(open_responses, None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&claude_resp.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
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
    async fn test_convert_request_same_protocol() {
        let converter = ClaudeMessagesConverter::new();
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "claude_messages",
            "claude_messages",
        ).await;
        
        assert!(result.is_ok());
    }
}
```

**验收标准**：
- [ ] ClaudeMessagesConverter实现完整
- [ ] 支持Claude Messages -> OpenAI Chat/Response转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务1.4：实现OpenAI Response转换器

**文件路径**：`src-tauri/src/protocol/converters/openai_response.rs`

**任务描述**：
实现OpenAI Response协议的转换器

**实现步骤**：

1. 创建转换器结构体
```rust
// src-tauri/src/protocol/converters/openai_response.rs

use async_trait::async_trait;
use bytes::Bytes;
use linguafranca::open_responses::request::OpenResponsesRequest;
use linguafranca::open_responses::response::OpenResponsesResponse;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::protocol::converter::{ConversionError, ProtocolConverter};

pub struct OpenAIResponseConverter;

impl OpenAIResponseConverter {
    pub fn new() -> Self {
        Self
    }
}
```

2. 实现ProtocolConverter trait
```rust
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
            ("openai_response", "openai_response") => {
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_response", "openai_chat") => {
                let open_req: OpenResponsesRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}")))?;
                
                let chat_req = linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest::from_open_responses(open_req, None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&chat_req.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            ("openai_response", "claude_messages") => {
                let open_req: OpenResponsesRequest = serde_json::from_slice(body)
                    .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Response请求失败: {e}")))?;
                
                let claude_req = linguafranca::anthropic::request::AnthropicRequest::from_open_responses(open_req, None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&claude_req.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
                format!("不支持的转换: {from_protocol} -> {to_protocol}")
            )),
        }
    }
    
    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError> {
        match (from_protocol, to_protocol) {
            ("openai_response", "openai_response") => {
                Ok(Bytes::from(body.to_vec()))
            }
            ("openai_chat", "openai_response") => {
                let chat_resp: linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse = 
                    serde_json::from_slice(body)
                        .map_err(|e| ConversionError::ParseError(format!("解析OpenAI Chat响应失败: {e}")))?;
                
                let open_resp = chat_resp.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&open_resp.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            ("claude_messages", "openai_response") => {
                let claude_resp: linguafranca::anthropic::response::AnthropicResponse = 
                    serde_json::from_slice(body)
                        .map_err(|e| ConversionError::ParseError(format!("解析Claude响应失败: {e}")))?;
                
                let open_resp = claude_resp.into_open_responses(None)
                    .map_err(|e| ConversionError::TransformError(format!("转换失败: {e}")))?;
                
                let result = serde_json::to_vec(&open_resp.value)
                    .map_err(|e| ConversionError::SerializationError(format!("序列化失败: {e}")))?;
                
                Ok(Bytes::from(result))
            }
            _ => Err(ConversionError::UnsupportedProtocol(
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
    async fn test_convert_request_same_protocol() {
        let converter = OpenAIResponseConverter::new();
        let body = serde_json::json!({
            "model": "gpt-4",
            "input": "Hello"
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "openai_response",
            "openai_response",
        ).await;
        
        assert!(result.is_ok());
    }
}
```

**验收标准**：
- [ ] OpenAIResponseConverter实现完整
- [ ] 支持OpenAI Response -> OpenAI Chat/Claude转换
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

## 📦 批次交付物

1. `src-tauri/src/protocol/converter.rs` - 协议转换抽象层
2. `src-tauri/src/protocol/converters/openai_chat.rs` - OpenAI Chat转换器
3. `src-tauri/src/protocol/converters/claude_messages.rs` - Claude Messages转换器
4. `src-tauri/src/protocol/converters/openai_response.rs` - OpenAI Response转换器
5. 更新的`src-tauri/src/protocol/mod.rs` - 模块导出

---

## ✅ 批次验收标准

- [ ] 所有转换器实现完整
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查
- [ ] 无编译错误
- [ ] 文档注释完整

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务1.1 | 1天 | 第1天 | 第1天 |
| 任务1.2 | 1.5天 | 第2天 | 第3天上午 |
| 任务1.3 | 1.5天 | 第3天下午 | 第4天 |
| 任务1.4 | 1天 | 第5天 | 第5天 |
