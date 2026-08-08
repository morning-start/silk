# Phase 1 - 批次3：集成与错误处理

## 📋 批次概述

**批次目标**：将新的协议转换层集成到现有系统，创建用户友好的错误处理层

**预计工期**：1周（5个工作日）

**依赖关系**：批次1、批次2完成

---

## 🎯 批次目标

1. 重构`transform_request.rs`，使用新的协议转换层
2. 重构`stream_response.rs`，使用新的流式转换层
3. 创建用户友好的错误信息转换层
4. 进行集成测试

---

## 📝 任务清单

### 任务3.1：重构transform_request.rs

**文件路径**：`src-tauri/src/gateway/middleware/transform_request.rs`

**任务描述**：
将现有的协议转换逻辑替换为使用新的协议转换层

**实现步骤**：

1. 导入新的协议转换层
```rust
// src-tauri/src/gateway/middleware/transform_request.rs

use bytes::Bytes;
use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::converter::{ConversionError, ConverterRegistry, ProtocolConverter};
```

2. 创建全局转换器注册表（懒加载）
```rust
use std::sync::Arc;
use once_cell::sync::Lazy;

/// 全局协议转换器注册表
static CONVERTER_REGISTRY: Lazy<Arc<ConverterRegistry>> = Lazy::new(|| {
    let mut registry = ConverterRegistry::new();
    
    // 注册所有转换器
    registry.register(Arc::new(crate::protocol::converters::openai_chat::OpenAIChatConverter::new()));
    registry.register(Arc::new(crate::protocol::converters::claude_messages::ClaudeMessagesConverter::new()));
    registry.register(Arc::new(crate::protocol::converters::openai_response::OpenAIResponseConverter::new()));
    
    Arc::new(registry)
});
```

3. 重构run函数
```rust
/// 请求转换中间件
///
/// 选择出站协议对应的适配器，处理跨协议请求体格式转换，
/// 将原始请求体（JSON）转为上游请求格式，更新ctx.request_body。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    let inbound = ctx
        .inbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_chat".to_string());
    let outbound = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| inbound.clone());
    
    let target_protocol = &outbound;
    
    // 注入stream:true，使所有请求走流式SSE路径
    if let Some(body) = ctx.get_parsed_body() {
        let mut json = body.clone();
        if !json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false) {
            json["stream"] = serde_json::Value::Bool(true);
            json["stream_options"] = serde_json::json!({"include_usage": true});
            ctx.update_body(json).map_err(|e| {
                StageError::new(
                    ctx.clone(),
                    GatewayError::BadRequest(format!("注入stream字段失败: {e}")),
                )
            })?;
        }
    }
    
    // 注入默认max_tokens（Claude协议必需字段）
    if target_protocol == "claude_messages" {
        if let Some(body) = ctx.get_parsed_body() {
            if body.get("max_tokens").is_none() {
                let mut json = body.clone();
                json["max_tokens"] = serde_json::Value::Number(1024.into());
                ctx.update_body(json).map_err(|e| {
                    StageError::new(
                        ctx.clone(),
                        GatewayError::BadRequest(format!("注入max_tokens字段失败: {e}")),
                    )
                })?;
            }
        }
    }
    
    // 使用新的协议转换层进行跨协议转换
    let request_bytes = if inbound != outbound {
        tracing::debug!(
            "跨协议格式转换: inbound={}, outbound={}",
            inbound,
            outbound
        );
        
        let converter = CONVERTER_REGISTRY.find_converter(&inbound, &outbound)
            .ok_or_else(|| {
                StageError::new(
                    ctx.clone(),
                    GatewayError::Transform(format!("不支持的协议转换: {inbound} -> {outbound}")),
                )
            })?;
        
        converter.convert_request(&ctx.request_body, &inbound, &outbound)
            .await
            .map_err(|e| {
                StageError::new(
                    ctx.clone(),
                    GatewayError::Transform(e.to_string()),
                )
            })?
    } else {
        ctx.request_body.to_vec()
    };
    
    // 获取outbound适配器（生成正确的URL、认证头、Content-Type）
    let adapter = ctx
        .adapter_registry
        .as_ref()
        .and_then(|reg| reg.get(target_protocol))
        .or_else(|| {
            ctx.adapter_registry
                .as_ref()
                .and_then(|reg| reg.get("openai_chat"))
        })
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {target_protocol}")),
            )
        })?;
    
    let provider = ctx.provider.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少provider".to_string()),
        )
    })?;
    
    let selected_api_key = ctx.selected_api_key.as_deref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少已选中的上游Key".to_string()),
        )
    })?;
    
    // 调用outbound适配器转换（验证并生成请求头/URL/序列化body）
    let upstream_req = adapter
        .transform_request(&request_bytes, provider, selected_api_key)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;
    
    let new_body = serde_json::to_vec(&upstream_req.body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;
    
    ctx.request_body = Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);
    ctx.upstream_url = Some(upstream_req.url);
    ctx.upstream_method = Some(upstream_req.method);
    
    Ok(ctx)
}
```

4. 删除旧的`convert_request_body`函数（已移至协议转换层）

5. 更新单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ... 保留现有测试，确保它们仍然通过
}
```

**验收标准**：
- [ ] 使用新的协议转换层
- [ ] 旧的转换逻辑已移除
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务3.2：重构stream_response.rs

**文件路径**：`src-tauri/src/gateway/middleware/stream_response.rs`

**任务描述**：
将现有的SSE转换逻辑替换为使用新的流式转换层

**实现步骤**：

1. 导入新的流式转换层
```rust
// src-tauri/src/gateway/middleware/stream_response.rs

use bytes::Bytes;
use crate::protocol::stream::{
    SseEvent, SseParser, StreamConversionError, StreamConverter, StreamConverterRegistry,
};
```

2. 创建全局流式转换器注册表（懒加载）
```rust
use std::sync::Arc;
use once_cell::sync::Lazy;

/// 全局流式转换器注册表
static STREAM_CONVERTER_REGISTRY: Lazy<Arc<StreamConverterRegistry>> = Lazy::new(|| {
    let mut registry = StreamConverterRegistry::new();
    
    // 注册所有流式转换器
    registry.register(Arc::new(crate::protocol::stream::openai_chat::OpenAIChatStreamConverter::new()));
    registry.register(Arc::new(crate::protocol::stream::claude_messages::ClaudeMessagesStreamConverter::new()));
    registry.register(Arc::new(crate::protocol::stream::openai_response::OpenAIResponseStreamConverter::new()));
    
    Arc::new(registry)
});
```

3. 重构SseConverter结构体
```rust
/// 流式SSE事件协议转换器（使用新的流式转换层）
pub struct SseConverter {
    /// 流式转换器
    converter: Option<Arc<dyn StreamConverter>>,
    /// 源协议
    from_protocol: String,
    /// 目标协议
    to_protocol: String,
}

impl SseConverter {
    pub fn new(inbound: &str, outbound: &str) -> Self {
        if inbound == outbound || inbound.is_empty() || outbound.is_empty() {
            return Self {
                converter: None,
                from_protocol: inbound.to_string(),
                to_protocol: outbound.to_string(),
            };
        }
        
        let converter = STREAM_CONVERTER_REGISTRY.find_converter(outbound, inbound);
        
        Self {
            converter,
            from_protocol: outbound.to_string(),
            to_protocol: inbound.to_string(),
        }
    }
    
    pub fn convert(&mut self, event: &SseEvent) -> Result<Bytes, String> {
        match &self.converter {
            Some(converter) => {
                let runtime = tokio::runtime::Handle::current();
                let events = runtime.block_on(async {
                    converter.convert_event(event, &self.from_protocol, &self.to_protocol).await
                }).map_err(|e| e.to_string())?;
                
                let mut bytes = Vec::new();
                for converted_event in events {
                    bytes.extend_from_slice(converted_event.serialize().as_bytes());
                }
                Ok(Bytes::from(bytes))
            }
            None => {
                // 无转换器，直接返回原始事件
                Ok(Bytes::from(event.serialize()))
            }
        }
    }
}
```

4. 保留现有的SseParser、SseEvent等工具类（已在新层中定义，可复用）

5. 更新单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ... 保留现有测试，确保它们仍然通过
}
```

**验收标准**：
- [ ] 使用新的流式转换层
- [ ] 旧的转换逻辑已移除
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务3.3：创建错误信息转换层

**文件路径**：`src-tauri/src/error/user_friendly.rs`

**任务描述**：
创建统一的用户友好错误信息转换层

**实现步骤**：

1. 定义用户友好错误类型
```rust
// src-tauri/src/error/user_friendly.rs

use serde::{Deserialize, Serialize};

/// 用户友好的错误信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFriendlyError {
    /// 错误标题
    pub title: String,
    /// 错误消息（用户友好）
    pub message: String,
    /// 建议操作
    pub suggestion: Option<String>,
    /// 错误类型（用于UI展示）
    pub error_type: UserFriendlyErrorType,
    /// 原始错误（仅用于日志，不展示给用户）
    #[serde(skip)]
    pub original_error: Option<String>,
}

/// 用户友好错误类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserFriendlyErrorType {
    /// 认证错误
    Authentication,
    /// 请求频率限制
    RateLimit,
    /// 服务不可用
    ServiceUnavailable,
    /// 请求格式错误
    BadRequest,
    /// 网络错误
    Network,
    /// 超时
    Timeout,
    /// 未知错误
    Unknown,
}
```

2. 实现错误转换函数
```rust
use crate::gateway::error::GatewayError;

/// 将技术错误转换为用户友好信息
pub fn convert_error(error: &GatewayError) -> UserFriendlyError {
    match error {
        GatewayError::Upstream(e) => {
            convert_upstream_error(e.status, &e.message)
        }
        GatewayError::Transform(e) => UserFriendlyError {
            title: "请求格式错误".to_string(),
            message: "请求格式转换失败，请检查输入内容".to_string(),
            suggestion: Some("请确保输入符合AI服务的格式要求".to_string()),
            error_type: UserFriendlyErrorType::BadRequest,
            original_error: Some(e.clone()),
        },
        GatewayError::Serialization(e) => UserFriendlyError {
            title: "数据格式错误".to_string(),
            message: "数据序列化失败，请稍后再试".to_string(),
            suggestion: Some("如问题持续，请联系技术支持".to_string()),
            error_type: UserFriendlyErrorType::Unknown,
            original_error: Some(e.clone()),
        },
        GatewayError::BadRequest(e) => UserFriendlyError {
            title: "请求错误".to_string(),
            message: e.clone(),
            suggestion: None,
            error_type: UserFriendlyErrorType::BadRequest,
            original_error: Some(e.clone()),
        },
        GatewayError::Internal(e) => UserFriendlyError {
            title: "系统错误".to_string(),
            message: "系统出现问题，请稍后再试".to_string(),
            suggestion: Some("如问题持续，请联系技术支持".to_string()),
            error_type: UserFriendlyErrorType::Unknown,
            original_error: Some(e.clone()),
        },
        GatewayError::Timeout => UserFriendlyError {
            title: "请求超时".to_string(),
            message: "请求处理超时，请稍后再试".to_string(),
            suggestion: Some("请检查网络连接，或稍后重试".to_string()),
            error_type: UserFriendlyErrorType::Timeout,
            original_error: None,
        },
        GatewayError::RateLimitExceeded => UserFriendlyError {
            title: "请求过于频繁".to_string(),
            message: "AI服务请求过于频繁，请稍后再试".to_string(),
            suggestion: Some("您可以尝试减少请求频率或等待一段时间".to_string()),
            error_type: UserFriendlyErrorType::RateLimit,
            original_error: None,
        },
    }
}

/// 转换上游错误
fn convert_upstream_error(status: u16, message: &str) -> UserFriendlyError {
    match status {
        401 => UserFriendlyError {
            title: "认证失败".to_string(),
            message: "AI服务认证失败，请检查您的API密钥是否正确".to_string(),
            suggestion: Some("请在设置中重新配置API密钥".to_string()),
            error_type: UserFriendlyErrorType::Authentication,
            original_error: Some(message.to_string()),
        },
        403 => UserFriendlyError {
            title: "访问被拒绝".to_string(),
            message: "AI服务访问被拒绝，请检查您的权限".to_string(),
            suggestion: Some("请确认您的账户有访问该服务的权限".to_string()),
            error_type: UserFriendlyErrorType::Authentication,
            original_error: Some(message.to_string()),
        },
        429 => UserFriendlyError {
            title: "请求过于频繁".to_string(),
            message: "AI服务请求过于频繁，请稍后再试".to_string(),
            suggestion: Some("您可以尝试减少请求频率或等待一段时间".to_string()),
            error_type: UserFriendlyErrorType::RateLimit,
            original_error: Some(message.to_string()),
        },
        500..=599 => UserFriendlyError {
            title: "服务暂时不可用".to_string(),
            message: "AI服务暂时不可用，请稍后再试".to_string(),
            suggestion: Some("请稍后重试，或联系服务提供商".to_string()),
            error_type: UserFriendlyErrorType::ServiceUnavailable,
            original_error: Some(message.to_string()),
        },
        _ => UserFriendlyError {
            title: "请求失败".to_string(),
            message: "AI服务出现问题，请稍后再试".to_string(),
            suggestion: None,
            error_type: UserFriendlyErrorType::Unknown,
            original_error: Some(format!("HTTP {status}: {message}")),
        },
    }
}
```

3. 创建错误转换服务（供前端调用）
```rust
/// 错误转换服务
pub struct ErrorConverterService;

impl ErrorConverterService {
    /// 转换错误为用户友好信息
    pub fn convert(error: &GatewayError) -> UserFriendlyError {
        convert_error(error)
    }
    
    /// 转换错误为JSON格式（供前端使用）
    pub fn convert_to_json(error: &GatewayError) -> serde_json::Value {
        let user_error = convert_error(error);
        serde_json::to_value(user_error).unwrap_or_else(|_| {
            serde_json::json!({
                "title": "系统错误",
                "message": "系统出现问题，请稍后再试",
                "error_type": "Unknown"
            })
        })
    }
}
```

4. 更新`src-tauri/src/error/mod.rs`，导出新模块
```rust
// src-tauri/src/error/mod.rs

pub mod user_friendly;

pub use user_friendly::{UserFriendlyError, UserFriendlyErrorType, ErrorConverterService};
```

5. 添加单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convert_authentication_error() {
        let error = GatewayError::Upstream(crate::gateway::error::UpstreamError {
            status: 401,
            message: "Invalid API key".to_string(),
        });
        
        let user_error = convert_error(&error);
        assert_eq!(user_error.title, "认证失败");
        assert_eq!(user_error.error_type, UserFriendlyErrorType::Authentication);
    }
    
    #[test]
    fn test_convert_rate_limit_error() {
        let error = GatewayError::RateLimitExceeded;
        
        let user_error = convert_error(&error);
        assert_eq!(user_error.title, "请求过于频繁");
        assert_eq!(user_error.error_type, UserFriendlyErrorType::RateLimit);
    }
}
```

**验收标准**：
- [ ] UserFriendlyError类型定义完整
- [ ] 错误转换函数覆盖所有错误类型
- [ ] 单元测试通过
- [ ] 代码通过cargo clippy检查

---

### 任务3.4：集成测试

**文件路径**：`src-tauri/src/protocol/tests/`

**任务描述**：
为所有转换器添加集成测试，确保端到端流程正常

**实现步骤**：

1. 创建集成测试目录
```bash
mkdir -p src-tauri/src/protocol/tests
```

2. 创建协议转换集成测试
```rust
// src-tauri/src/protocol/tests/integration_test.rs

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use crate::protocol::converter::{ConverterRegistry, ProtocolConverter};
    use crate::protocol::converters::*;
    
    fn create_registry() -> ConverterRegistry {
        let mut registry = ConverterRegistry::new();
        registry.register(std::sync::Arc::new(openai_chat::OpenAIChatConverter::new()));
        registry.register(std::sync::Arc::new(claude_messages::ClaudeMessagesConverter::new()));
        registry.register(std::sync::Arc::new(openai_response::OpenAIResponseConverter::new()));
        registry
    }
    
    #[tokio::test]
    async fn test_openai_chat_to_claude_messages() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages").unwrap();
        
        let request_body = serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "temperature": 0.7
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&request_body).unwrap(),
            "openai_chat",
            "claude_messages",
        ).await;
        
        assert!(result.is_ok());
        let converted: serde_json::Value = serde_json::from_slice(&result.unwrap()).unwrap();
        assert!(converted.get("messages").is_some());
    }
    
    #[tokio::test]
    async fn test_claude_messages_to_openai_chat() {
        let registry = create_registry();
        let converter = registry.find_converter("claude_messages", "openai_chat").unwrap();
        
        let request_body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&request_body).unwrap(),
            "claude_messages",
            "openai_chat",
        ).await;
        
        assert!(result.is_ok());
    }
}
```

3. 创建流式转换集成测试
```rust
// src-tauri/src/protocol/tests/stream_integration_test.rs

#[cfg(test)]
mod tests {
    use crate::protocol::stream::{
        SseEvent, StreamConverterRegistry, StreamConverter,
    };
    use crate::protocol::stream::converters::*;
    
    fn create_registry() -> StreamConverterRegistry {
        let mut registry = StreamConverterRegistry::new();
        registry.register(std::sync::Arc::new(openai_chat::OpenAIChatStreamConverter::new()));
        registry.register(std::sync::Arc::new(claude_messages::ClaudeMessagesStreamConverter::new()));
        registry.register(std::sync::Arc::new(openai_response::OpenAIResponseStreamConverter::new()));
        registry
    }
    
    #[tokio::test]
    async fn test_openai_chat_stream_to_openai_response() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "openai_response").unwrap();
        
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

4. 更新`src-tauri/src/protocol/mod.rs`，添加测试模块
```rust
// src-tauri/src/protocol/mod.rs

pub mod adapter;
pub mod adapters;
pub mod builtin_adapters;
pub mod converter;
pub mod registry;
pub mod stream;
pub mod tests;  // 新增

// ... 其他导出
```

**验收标准**：
- [ ] 集成测试覆盖所有协议转换组合
- [ ] 集成测试覆盖所有流式转换组合
- [ ] 所有集成测试通过
- [ ] 测试覆盖率达到80%以上

---

## 📦 批次交付物

1. 重构的`src-tauri/src/gateway/middleware/transform_request.rs`
2. 重构的`src-tauri/src/gateway/middleware/stream_response.rs`
3. `src-tauri/src/error/user_friendly.rs` - 错误信息转换层
4. `src-tauri/src/protocol/tests/` - 集成测试
5. 更新的`src-tauri/src/error/mod.rs`

---

## ✅ 批次验收标准

- [ ] 所有重构完成
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
- [ ] 代码通过cargo clippy检查
- [ ] 无编译错误
- [ ] 向后兼容性验证通过

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务3.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务3.2 | 1.5天 | 第2天下午 | 第3天 |
| 任务3.3 | 1天 | 第4天 | 第4天 |
| 任务3.4 | 1天 | 第5天 | 第5天 |
