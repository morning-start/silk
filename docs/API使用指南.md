# Silk API 使用指南

## 概述

Silk 是一个本地 AI 网关，监听 `http://127.0.0.1:{port}`，将请求转发到上游 AI 服务。

更完整、以当前代码为准的 HTTP 接口说明见 [网关 API 文档](./网关API文档.md)。

## 启动网关

### 通过 Tauri GUI

1. 打开 Silk 应用
2. 进入「设置」页面配置网关参数
3. 点击「启动网关」按钮

### 通过 Tauri IPC

```typescript
import { invoke } from '@tauri-apps/api/core';

// 启动网关
await invoke('start_gateway');

// 停止网关
await invoke('stop_gateway');

// 获取网关状态
const status = await invoke('get_gateway_status');
```

## API 端点

### 健康检查

```http
GET /health
```

**响应：**
```json
{
  "status": "ok",
  "service": "silk-gateway"
}
```

### 代理请求

```http
POST /*
```

所有匹配路由规则的请求会被转发到上游 Provider。

## 路由配置

路由规则决定请求如何转发到上游 Provider。

### 路由匹配字段

| 字段 | 类型 | 说明 | 示例 |
|------|------|------|------|
| `host` | String | 请求 Host 头 | `api.openai.com` |
| `path` | String | 请求路径（支持前缀匹配） | `/v1/chat/completions` |
| `method` | String | HTTP 方法 | `POST` |
| `content_type` | String | Content-Type 头 | `application/json` |
| `priority` | Number | 优先级（数字越小优先级越高） | `100` |

### 创建路由规则

```typescript
const rule = {
  name: 'OpenAI Chat 转发',
  host: '127.0.0.1',
  path: '/v1/chat/completions',
  method: 'POST',
  content_type: 'application/json',
  inbound_protocol: 'openai_chat',
  outbound_protocol: 'openai_chat',
  target_provider_id: 'provider-uuid',
  target_group_id: null,  // 或指定分组
  enabled: true,
  priority: 100,
};

await invoke('create_routing_rule', { rule });
```

### 路由匹配逻辑

1. 按 `priority` 升序排序
2. 依次检查每个规则：
   - `host` 精确匹配（可选，为空则跳过）
   - `path` 前缀匹配（可选，为空则跳过）
   - `method` 精确匹配（可选，为空则跳过）
   - `content_type` 包含匹配（可选，为空则跳过）
3. 返回第一个匹配的规则

## Provider 管理

### Provider 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | String | 显示名称 |
| `provider_type` | String | 提供商类型（openai_chat / claude / openai_response） |
| `api_base_url` | String | API 基础 URL |
| `api_key` | String | API 密钥（加密存储） |
| `model_name` | String | 模型名称（可选） |
| `timeout_seconds` | Number | 超时时间（秒） |
| `max_retries` | Number | 最大重试次数 |
| `status` | String | 状态（enabled / disabled） |

### 创建 Provider

```typescript
const provider = {
  name: 'OpenAI GPT-4',
  provider_type: 'openai_chat',
  api_base_url: 'https://api.openai.com',
  api_key: 'sk-xxx',
  model_name: 'gpt-4',
  timeout_seconds: 60,
  max_retries: 3,
  status: 'enabled',
};

await invoke('create_provider', { provider });
```

## 协议转换

### 支持的协议

| 协议 | 类型标识 | 说明 |
|------|----------|------|
| OpenAI Chat | `openai_chat` | OpenAI Chat Completions API |
| Claude Messages | `claude` | Anthropic Messages API |
| OpenAI Response | `openai_response` | OpenAI Response API |

### 协议转换流程

```
入站请求 (任意协议)
    ↓
Inbound Adapter → CanonicalRequest (中间格式)
    ↓
Outbound Adapter → 上游请求 (目标协议)
    ↓
上游响应
    ↓
Outbound Adapter → CanonicalResponse (中间格式)
    ↓
Inbound Adapter → 入站响应 (原始协议)
```

### 配置协议转换

在路由规则中设置：

```typescript
const rule = {
  // Claude 请求 → OpenAI Chat
  inbound_protocol: 'claude',      // 入站协议
  outbound_protocol: 'openai_chat', // 出站协议
  // ...
};
```

## SSE 流式响应

Silk 完整支持 SSE 流式响应，无需特殊配置。

### 心跳保活

- 每 15 秒发送 `: keep-alive\n\n`
- 30 秒无数据自动超时

### 断线重连

Silk 自动携带 `Last-Event-ID` 头进行断线重连。

## 错误处理

### 错误响应格式

```json
{
  "message": "错误描述"
}
```

### 错误码

| 状态码 | 错误码 | 说明 |
|--------|--------|------|
| 400 | `bad_request` | 请求格式错误 |
| 400 | `transform_error` | 协议转换失败 |
| 404 | `not_found` | 路由不存在 |
| 502 | `upstream_error` | 上游请求失败 |
| 504 | `timeout` | 请求超时 |
| 500 | `internal_error` | 内部错误 |

## 示例：配置 OpenAI 透传

### 1. 创建 Provider

```typescript
await invoke('create_provider', {
  provider: {
    name: 'OpenAI',
    provider_type: 'openai_chat',
    api_base_url: 'https://api.openai.com',
    api_key: 'sk-your-key',
    model_name: 'gpt-4',
    timeout_seconds: 60,
    max_retries: 3,
    status: 'enabled',
  }
});
```

### 2. 创建路由规则

```typescript
await invoke('create_routing_rule', {
  rule: {
    name: 'OpenAI 透传',
    host: '127.0.0.1',
    path: '/v1/chat/completions',
    method: 'POST',
    inbound_protocol: 'openai_chat',
    outbound_protocol: 'openai_chat',
    target_provider_id: 'provider-uuid',
    enabled: true,
    priority: 100,
  }
});
```

### 3. 测试请求

```bash
curl http://127.0.0.1:1234/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## 示例：Claude → OpenAI 转换

### 1. 创建 Claude Provider

```typescript
await invoke('create_provider', {
  provider: {
    name: 'Claude',
    provider_type: 'claude',
    api_base_url: 'https://api.anthropic.com',
    api_key: 'sk-ant-xxx',
    model_name: 'claude-3-opus',
    timeout_seconds: 120,
    max_retries: 2,
    status: 'enabled',
  }
});
```

### 2. 创建转换路由

```typescript
await invoke('create_routing_rule', {
  rule: {
    name: 'Claude → OpenAI 转换',
    path: '/v1/messages',
    inbound_protocol: 'claude',
    outbound_protocol: 'openai_chat',
    target_provider_id: 'openai-provider-uuid',
    enabled: true,
    priority: 100,
  }
});
```

### 3. 测试

客户端发送 Claude 格式请求，Silk 自动转换为 OpenAI Chat 格式转发。
