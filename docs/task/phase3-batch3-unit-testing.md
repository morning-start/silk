# Phase 3 - 批次3：单元测试补充

## 📋 批次概述

**批次目标**：为协议转换层、Gateway层、Application层、前端组件补充单元测试

**预计工期**：1周（5个工作日）

**依赖关系**：Phase 1、Phase 2 完成

---

## 🎯 批次目标

1. 协议转换层单元测试
2. Gateway层单元测试
3. Application层单元测试
4. 前端组件单元测试

---

## 📝 任务清单

### 任务3.1：协议转换层单元测试

**文件路径**：`src-tauri/src/protocol/tests/`

**任务描述**：
为所有转换器添加单元测试

**实现步骤**：

1. 创建协议转换层测试目录
```bash
mkdir -p src-tauri/src/protocol/tests
```

2. 创建转换器测试
```rust
// src-tauri/src/protocol/tests/converter_test.rs

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use crate::protocol::converter::{ConverterRegistry, ProtocolConverter, ConversionError};
    use crate::protocol::converters::*;
    
    fn create_registry() -> ConverterRegistry {
        let mut registry = ConverterRegistry::new();
        registry.register(std::sync::Arc::new(openai_chat::OpenAIChatConverter::new()));
        registry.register(std::sync::Arc::new(claude_messages::ClaudeMessagesConverter::new()));
        registry.register(std::sync::Arc::new(openai_response::OpenAIResponseConverter::new()));
        registry
    }
    
    #[test]
    fn test_registry_creation() {
        let registry = create_registry();
        assert_eq!(registry.list_converters().len(), 3);
    }
    
    #[test]
    fn test_find_converter() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages");
        assert!(converter.is_some());
    }
    
    #[test]
    fn test_find_unsupported_converter() {
        let registry = create_registry();
        let converter = registry.find_converter("unsupported", "openai_chat");
        assert!(converter.is_none());
    }
    
    #[tokio::test]
    async fn test_openai_chat_convert_request_same_protocol() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "openai_chat").unwrap();
        
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
    async fn test_openai_chat_convert_request_to_claude() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages").unwrap();
        
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "openai_chat",
            "claude_messages",
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_claude_convert_request_to_openai() {
        let registry = create_registry();
        let converter = registry.find_converter("claude_messages", "openai_chat").unwrap();
        
        let body = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "claude_messages",
            "openai_chat",
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_unsupported_protocol_conversion() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages").unwrap();
        
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let result = converter.convert_request(
            &serde_json::to_vec(&body).unwrap(),
            "unsupported",
            "openai_chat",
        ).await;
        
        assert!(result.is_err());
    }
}
```

3. 创建流式转换器测试
```rust
// src-tauri/src/protocol/tests/stream_converter_test.rs

#[cfg(test)]
mod tests {
    use crate::protocol::stream::{
        SseEvent, StreamConverterRegistry, StreamConverter, StreamConversionError,
    };
    use crate::protocol::stream::converters::*;
    
    fn create_registry() -> StreamConverterRegistry {
        let mut registry = StreamConverterRegistry::new();
        registry.register(std::sync::Arc::new(openai_chat::OpenAIChatStreamConverter::new()));
        registry.register(std::sync::Arc::new(claude_messages::ClaudeMessagesStreamConverter::new()));
        registry.register(std::sync::Arc::new(openai_response::OpenAIResponseStreamConverter::new()));
        registry
    }
    
    #[test]
    fn test_stream_registry_creation() {
        let registry = create_registry();
        assert_eq!(registry.list_converters().len(), 3);
    }
    
    #[test]
    fn test_find_stream_converter() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages");
        assert!(converter.is_some());
    }
    
    #[tokio::test]
    async fn test_openai_chat_stream_convert_same_protocol() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "openai_chat").unwrap();
        
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
    async fn test_openai_chat_stream_convert_to_claude() {
        let registry = create_registry();
        let converter = registry.find_converter("openai_chat", "claude_messages").unwrap();
        
        let event = SseEvent {
            event: None,
            data: Some(r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        
        let result = converter.convert_event(&event, "openai_chat", "claude_messages").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_sse_event_serialization() {
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
}
```

**验收标准**：
- [ ] 所有转换器测试覆盖
- [ ] 测试用例通过
- [ ] 测试覆盖率达到80%以上

---

### 任务3.2：Gateway层单元测试

**文件路径**：`src-tauri/src/gateway/tests/`

**任务描述**：
为中间件、管道、错误处理添加单元测试

**实现步骤**：

1. 创建Gateway层测试目录
```bash
mkdir -p src-tauri/src/gateway/tests
```

2. 创建中间件测试
```rust
// src-tauri/src/gateway/tests/middleware_test.rs

#[cfg(test)]
mod tests {
    use crate::gateway::context::RequestContext;
    use crate::gateway::error::GatewayError;
    use crate::gateway::middleware::*;
    use axum::http::{HeaderMap, Method};
    use std::time::Instant;
    
    fn create_test_context() -> RequestContext {
        RequestContext::new(
            "req-1".to_string(),
            Instant::now(),
            Method::POST,
            "/v1/chat/completions".parse().expect("valid uri"),
            HeaderMap::new(),
        )
    }
    
    #[test]
    fn test_request_context_creation() {
        let ctx = create_test_context();
        assert_eq!(ctx.request_id, "req-1");
        assert_eq!(ctx.method, Method::POST);
    }
    
    #[test]
    fn test_gateway_error_status_codes() {
        let error = GatewayError::BadRequest("test".to_string());
        assert_eq!(error.status_code(), 400);
        
        let error = GatewayError::Internal("test".to_string());
        assert_eq!(error.status_code(), 500);
        
        let error = GatewayError::Timeout;
        assert_eq!(error.status_code(), 504);
        
        let error = GatewayError::RateLimitExceeded;
        assert_eq!(error.status_code(), 429);
    }
    
    #[test]
    fn test_gateway_error_messages() {
        let error = GatewayError::BadRequest("invalid input".to_string());
        assert!(error.to_string().contains("invalid input"));
        
        let error = GatewayError::Internal("server error".to_string());
        assert!(error.to_string().contains("server error"));
    }
}
```

3. 创建管道测试
```rust
// src-tauri/src/gateway/tests/pipeline_test.rs

#[cfg(test)]
mod tests {
    use crate::gateway::pipeline::GatewayPipeline;
    use crate::gateway::context::GatewayContext;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    #[test]
    fn test_pipeline_creation() {
        // 测试管道创建
        // 注意：这需要完整的GatewayContext，可能需要mock
    }
}
```

**验收标准**：
- [ ] 中间件测试覆盖
- [ ] 管道测试覆盖
- [ ] 错误处理测试覆盖
- [ ] 测试用例通过

---

### 任务3.3：Application层单元测试

**文件路径**：`src-tauri/src/application/tests/`

**任务描述**：
为服务层、业务逻辑添加单元测试

**实现步骤**：

1. 创建Application层测试目录
```bash
mkdir -p src-tauri/src/application/tests
```

2. 创建服务测试
```rust
// src-tauri/src/application/tests/service_test.rs

#[cfg(test)]
mod tests {
    use crate::application::auto_detect::AiAppDetector;
    use crate::application::preset_providers::PresetProviderService;
    use crate::application::smart_defaults::SmartDefaultsService;
    
    #[test]
    fn test_ai_app_detector() {
        let apps = AiAppDetector::detect_all();
        assert!(!apps.is_empty());
        
        // 验证每个应用都有必要的字段
        for app in apps {
            assert!(!app.name.is_empty());
            assert!(!app.description.is_empty());
        }
    }
    
    #[test]
    fn test_preset_providers() {
        let providers = PresetProviderService::get_all();
        assert!(!providers.is_empty());
        
        // 验证每个Provider都有必要的字段
        for provider in providers {
            assert!(!provider.id.is_empty());
            assert!(!provider.name.is_empty());
            assert!(!provider.api_base_url.is_empty());
        }
    }
    
    #[test]
    fn test_preset_provider_by_id() {
        let provider = PresetProviderService::get_by_id("openai");
        assert!(provider.is_some());
        
        let provider = provider.unwrap();
        assert_eq!(provider.id, "openai");
        assert_eq!(provider.name, "OpenAI");
    }
    
    #[test]
    fn test_preset_provider_not_found() {
        let provider = PresetProviderService::get_by_id("nonexistent");
        assert!(provider.is_none());
    }
    
    #[test]
    fn test_smart_defaults() {
        let recommendations = SmartDefaultsService::get_recommendations();
        // 可能为空，取决于系统环境
        // 但不应该panic
        for rec in recommendations {
            assert!(!rec.provider_id.is_empty());
            assert!(!rec.provider_name.is_empty());
            assert!(!rec.reason.is_empty());
            assert!(rec.confidence >= 0.0 && rec.confidence <= 1.0);
        }
    }
}
```

**验收标准**：
- [ ] 服务层测试覆盖
- [ ] 业务逻辑测试覆盖
- [ ] 测试用例通过

---

### 任务3.4：前端组件单元测试

**文件路径**：`src/components/__tests__/`

**任务描述**：
为Vue组件添加单元测试

**实现步骤**：

1. 创建前端测试目录
```bash
mkdir -p src/components/__tests__
```

2. 创建组件测试
```typescript
// src/components/__tests__/UserFriendlyError.test.ts

import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import UserFriendlyError from '../UserFriendlyError.vue';

describe('UserFriendlyError', () => {
  it('renders error message correctly', () => {
    const error = {
      title: '认证失败',
      message: 'AI服务认证失败，请检查您的API密钥是否正确',
      suggestion: '请在设置中重新配置API密钥',
      error_type: 'Authentication',
    };
    
    const wrapper = mount(UserFriendlyError, {
      props: { error },
    });
    
    expect(wrapper.text()).toContain('认证失败');
    expect(wrapper.text()).toContain('AI服务认证失败');
  });
  
  it('renders suggestion when provided', () => {
    const error = {
      title: '认证失败',
      message: 'AI服务认证失败',
      suggestion: '请在设置中重新配置API密钥',
      error_type: 'Authentication',
    };
    
    const wrapper = mount(UserFriendlyError, {
      props: { error },
    });
    
    expect(wrapper.text()).toContain('请在设置中重新配置API密钥');
  });
  
  it('does not render suggestion when not provided', () => {
    const error = {
      title: '认证失败',
      message: 'AI服务认证失败',
      error_type: 'Authentication',
    };
    
    const wrapper = mount(UserFriendlyError, {
      props: { error },
    });
    
    expect(wrapper.text()).not.toContain('请在设置中重新配置API密钥');
  });
  
  it('emits close event when close button clicked', async () => {
    const error = {
      title: '认证失败',
      message: 'AI服务认证失败',
      error_type: 'Authentication',
    };
    
    const wrapper = mount(UserFriendlyError, {
      props: { error },
    });
    
    await wrapper.find('.close-button').trigger('click');
    expect(wrapper.emitted('close')).toBeTruthy();
  });
  
  it('emits retry event when retry button clicked', async () => {
    const error = {
      title: '认证失败',
      message: 'AI服务认证失败',
      error_type: 'Authentication',
    };
    
    const wrapper = mount(UserFriendlyError, {
      props: { error },
    });
    
    await wrapper.find('.retry-button').trigger('click');
    expect(wrapper.emitted('retry')).toBeTruthy();
  });
});
```

3. 创建状态指示器测试
```typescript
// src/components/__tests__/StatusIndicator.test.ts

import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import StatusIndicator from '../StatusIndicator.vue';

describe('StatusIndicator', () => {
  it('renders loading state correctly', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'loading', message: '加载中...' },
    });
    
    expect(wrapper.find('.status-loading').exists()).toBe(true);
    expect(wrapper.text()).toContain('加载中...');
  });
  
  it('renders success state correctly', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'success', message: '操作成功' },
    });
    
    expect(wrapper.find('.status-success').exists()).toBe(true);
    expect(wrapper.text()).toContain('操作成功');
  });
  
  it('renders error state correctly', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'error', message: '操作失败' },
    });
    
    expect(wrapper.find('.status-error').exists()).toBe(true);
    expect(wrapper.text()).toContain('操作失败');
  });
  
  it('renders warning state correctly', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'warning', message: '警告信息' },
    });
    
    expect(wrapper.find('.status-warning').exists()).toBe(true);
    expect(wrapper.text()).toContain('警告信息');
  });
  
  it('renders info state correctly', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'info', message: '提示信息' },
    });
    
    expect(wrapper.find('.status-info').exists()).toBe(true);
    expect(wrapper.text()).toContain('提示信息');
  });
  
  it('does not render when status is idle', () => {
    const wrapper = mount(StatusIndicator, {
      props: { status: 'idle' },
    });
    
    expect(wrapper.find('.status-indicator').exists()).toBe(false);
  });
});
```

**验收标准**：
- [ ] 组件测试覆盖
- [ ] 测试用例通过
- [ ] 测试覆盖率达到80%以上

---

## 📦 批次交付物

1. `src-tauri/src/protocol/tests/` - 协议转换层测试
2. `src-tauri/src/gateway/tests/` - Gateway层测试
3. `src-tauri/src/application/tests/` - Application层测试
4. `src/components/__tests__/` - 前端组件测试

---

## ✅ 批次验收标准

- [ ] 所有测试用例通过
- [ ] 测试覆盖率达到80%以上
- [ ] 测试代码质量良好
- [ ] 测试文档完整

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务3.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务3.2 | 1天 | 第2天下午 | 第3天 |
| 任务3.3 | 1天 | 第4天 | 第4天 |
| 任务3.4 | 1天 | 第5天 | 第5天 |
