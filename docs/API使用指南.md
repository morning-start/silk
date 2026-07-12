# Silk API 使用指南

## 概述

Silk（丝路）是一个纯本地桌面 AI 多模型网关客户端。提供统一本地 HTTP 端点 `http://127.0.0.1:{port}`，桥接 OpenAI Chat、Claude Messages、OpenAI Response 三种 LLM 协议范式。所有数据停留在本地，无云端组件。

**路由决策顺序**（由高到低）：
1. 模型映射（`model_mappings`）— 按请求体 `model` 字段匹配，支持多渠道负载均衡与自动失败回退
2. 路由规则（`routing_rules`）— 按 Host / Path / Method / ContentType 匹配，直接转发到目标 Provider，兼容历史 `target_group_id`
3. 路径兜底 — 按请求路径推断协议，选取任意启用 Provider

更完整的 HTTP 接口说明见 [网关 API 文档](./网关API文档.md)。


---

## 目录

1. [Tauri IPC 命令总览](#1-tauri-ipc-命令总览)
2. [Gateway 控制](#2-gateway-控制)
3. [Provider（渠道）管理](#3-provider渠道管理)
4. [路由规则管理](#4-路由规则管理)
5. [历史兼容说明](#5-历史兼容说明)
6. [网关设置](#6-网关设置)
7. [网关 API Key 管理](#7-网关-api-key-管理)
8. [模型映射管理](#8-模型映射管理)
9. [日志管理](#9-日志管理)
10. [仪表盘统计](#10-仪表盘统计)
11. [网关 HTTP 端点](#11-网关-http-端点)
12. [协议适配与转换](#12-协议适配与转换)
13. [业务概念详解](#13-业务概念详解)
14. [完整配置示例](#14-完整配置示例)

---

## 1. Tauri IPC 命令总览

所有命令通过 `@tauri-apps/api/core` 的 `invoke` 调用。

```typescript
import { invoke } from '@tauri-apps/api/core';
```

命令参数名为 Rust 函数参数名（snake_case），通过对象字面量传入。

### 命令分类速查

| 分类 | 命令名 |
|------|--------|
| Gateway 控制 | `gateway_status`, `gateway_start`, `gateway_stop`, `gateway_restart` |
| Provider 管理 | `list_providers`, `get_provider`, `create_provider`, `update_provider`, `test_provider`, `delete_provider`, `fetch_provider_models` |
| 路由规则 | `list_routing_rules`, `get_routing_rule`, `create_routing_rule`, `update_routing_rule`, `delete_routing_rule` |
| 网关设置 | `get_gateway_settings`, `update_gateway_settings` |
| 日志管理 | `list_logs`, `cleanup_logs`, `clear_all_logs`, `export_logs_csv` |
| 仪表盘统计 | `dashboard_stats`, `recent_requests`, `stats_by_provider`, `hourly_stats` |
| 网关 Key | `list_gateway_keys`, `get_gateway_key`, `create_gateway_key`, `update_gateway_key`, `delete_gateway_key` |
| 模型映射 | `list_model_mappings`, `get_model_mapping`, `find_model_mapping_by_name`, `create_model_mapping`, `update_model_mapping`, `delete_model_mapping` |

---

## 2. Gateway 控制

### 启动网关

```typescript
const result = await invoke('gateway_start');
console.log(result);
// { success: true, message: "网关已启动", bind_address: "127.0.0.1:1877" }
```

### 停止网关

```typescript
const result = await invoke('gateway_stop');
// { success: true, message: "网关已停止" }
```

### 重启网关

```typescript
const result = await invoke('gateway_restart');
// { success: true, message: "网关已重启", bind_address: "127.0.0.1:1877" }
```

### 查询网关状态

```typescript
const status = await invoke('gateway_status');
// {
//   running: true,
//   bind_address: "127.0.0.1:1877",
//   uptime_seconds: 3600,
//   total_requests: 42,
//   active_providers: 3
// }
```

---

## 3. Provider（渠道）管理

### Provider 字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | String | 是 | 渠道显示名称 |
| `protocols` | String[] | 是 | 支持的接口协议，如 `["openai_chat"]`、`["claude_messages"]`、`["openai_response"]` |
| `api_base_url` | String | 是 | API 基础 URL（如 `https://api.openai.com`），会自动去除尾部 `/v1` |
| `models` | String[] | 是 | 模型列表，如 `["gpt-4", "gpt-3.5-turbo"]` |
| `keys` | ProviderKeyEntry[] | 是 | API Key 条目数组（详情见下方） |
| `key_strategy` | String | 否 | 密钥选择策略，默认 `round_robin` |
| `proxy_url` | String | 否 | HTTP 代理地址 |
| `timeout_seconds` | Number | 否 | 请求超时（秒），默认 30 |
| `max_retries` | Number | 否 | 最大重试次数，默认 3 |
| `status` | String | 否 | `"enabled"` 或 `"disabled"`，默认 `"enabled"` |

### ProviderKeyEntry 结构

```typescript
interface ProviderKeyEntry {
  name: string;    // 密钥名称，如 "主密钥"
  value: string;   // API Key 明文值，如 "sk-xxx"
  enabled: boolean; // 是否启用
  weight: number;  // 权重（仅 key_strategy=weighted 时有效，默认 1）
}
```

### API Key 选择策略

| 策略值 | 说明 | 适用场景 |
|--------|------|----------|
| `round_robin`（默认） | 轮询所有启用的密钥 | 多 Key 负载均衡 |
| `weighted` | 按权重比例选择，权重越高被选中概率越大 | 不同速率限制的 Key 混用 |
| `failover` | 按顺序选第一个可用 Key，失败时切换到下一个 | 主备 Key |

### 创建 Provider

```typescript
const provider = await invoke('create_provider', {
  payload: {
    name: '我的 OpenAI',
    protocols: ['openai_chat'],
    api_base_url: 'https://api.openai.com',
    models: ['gpt-4', 'gpt-3.5-turbo'],
    keys: [
      { name: '主密钥', value: 'sk-xxxxx', enabled: true, weight: 1 }
    ],
    key_strategy: 'round_robin',
    timeout_seconds: 60,
    max_retries: 3,
    status: 'enabled',
  }
});
```

### 更新 Provider

```typescript
const updated = await invoke('update_provider', {
  id: 'provider-uuid',
  payload: {
    name: 'OpenAI 新版',
    protocols: ['openai_chat', 'openai_response'],
    // 仅传入需要更新的字段，不传的字段保持不变
    timeout_seconds: 120,
  }
});
```

### 获取 Provider 列表

```typescript
const providers = await invoke('list_providers');
// [
//   {
//     id: "uuid",
//     name: "我的 OpenAI",
//     protocols: ["openai_chat"],
//     models: ["gpt-4", "gpt-3.5-turbo"],
//     keys: [...],          // 已解密的密钥列表
//     key_count: 1,
//     api_base_url: "https://api.openai.com",
//     proxy_url: null,
//     timeout_seconds: 60,
//     max_retries: 3,
//     status: "enabled",
//     health_status: "healthy",
//     created_at: "2026-06-29T12:00:00",
//     updated_at: "2026-06-29T12:00:00",
//   }
// ]
```

### 获取单个 Provider

```typescript
const provider = await invoke('get_provider', { id: 'provider-uuid' });
```

### 测试 Provider 连通性

```typescript
const result = await invoke('test_provider', { id: 'provider-uuid' });
// {
//   status_code: 200,
//   response_time_ms: 320,
//   health_status: "healthy",
//   error: null
// }
```

### 删除 Provider

```typescript
const deleted = await invoke('delete_provider', { id: 'provider-uuid' });
// true
```

### 拉取 Provider 的远程模型列表

```typescript
const models = await invoke('fetch_provider_models', {
  payload: {
    api_base_url: 'https://api.openai.com',
    api_key: 'sk-xxxxx',  // 用于认证
    proxy_url: null,       // 可选
    timeout_seconds: 10,   // 可选，默认 10
  }
});
// [
//   { id: "gpt-4", object: "model", created: 1687882411, owned_by: "openai" },
//   { id: "gpt-4o", object: "model", created: ... },
// ]
```

---

## 4. 路由规则管理

### 路由规则字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | String | 是 | 规则名称 |
| `match_path` | String | 是 | 请求路径匹配（支持 `*` 通配符前缀匹配） |
| `match_method` | String | 否 | HTTP 方法，默认 `*`（匹配所有） |
| `match_content_type` | String | 否 | Content-Type 包含匹配，默认空（不检查） |
| `target_provider_id` | String | 是 | 目标 Provider ID |
| `target_group_id` | String | 否 | 历史兼容字段；当前不提供独立分组管理入口 |
| `inbound_protocol` | String | 否 | 入站协议标识，如 `"openai_chat"` |
| `outbound_protocol` | String | 否 | 出站协议标识，如 `"claude_messages"` |
| `protocol_conversion` | Boolean | 否 | 是否启用协议转换，默认 false |
| `model_name_override` | String | 否 | 覆盖请求中的模型名 |
| `priority` | Number | 否 | 优先级（数字越小优先级越高），默认 100 |
| `enabled` | Boolean | 否 | 是否启用，默认 true |

### 匹配逻辑

1. 只匹配 `enabled = true` 的规则
2. 按 `priority` 升序排序
3. 依次检查：
   - `match_path` — 支持 `*` 通配符前缀匹配（如 `/v1/chat/completions*`）
   - `match_method` — 精确匹配，`*` 表示匹配所有方法
   - `match_content_type` — 包含匹配（请求 Content-Type 包含此字符串）
4. 返回第一个完全匹配的规则

### 创建路由规则

```typescript
const rule = await invoke('create_routing_rule', {
  payload: {
    name: 'OpenAI Chat 转发',
    match_path: '/v1/chat/completions',
    match_method: 'POST',
    match_content_type: 'application/json',
    target_provider_id: 'provider-uuid',
    inbound_protocol: 'openai_chat',
    outbound_protocol: 'openai_chat',
    priority: 100,
    enabled: true,
  }
});
```

### 协议转换路由

```typescript
// Claude 格式请求 → 转发给 OpenAI Chat Provider
const rule = await invoke('create_routing_rule', {
  payload: {
    name: 'Claude → OpenAI 转换',
    match_path: '/v1/messages',
    match_method: 'POST',
    target_provider_id: 'openai-provider-uuid',
    inbound_protocol: 'claude_messages',
    outbound_protocol: 'openai_chat',
    protocol_conversion: true,
    enabled: true,
    priority: 100,
  }
});
```

### 模型名称覆盖

```typescript
// 将所有请求的模型名统一替换为 "gpt-4o"
await invoke('create_routing_rule', {
  payload: {
    name: '统一模型名',
    match_path: '/v1/chat/completions',
    target_provider_id: 'provider-uuid',
    model_name_override: 'gpt-4o',
  }
});
```

### 更新/删除

```typescript
await invoke('update_routing_rule', { id: 'rule-uuid', payload: { ... } });
await invoke('delete_routing_rule', { id: 'rule-uuid' });
```

---

## 5. 历史兼容说明

当前实现不再提供独立的 Provider 分组管理命令或页面。`routing_rules.target_group_id` 仅作为历史兼容字段保留。

---

## 6. 网关设置

### 设置字段

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| `bind_host` | String | 监听地址 | `127.0.0.1` |
| `bind_port` | Number | 监听端口 | `1877` |
| `allow_remote` | Boolean | 是否允许远程连接 | `false` |
| `log_retention_days` | Number | 日志保留天数 | `30` |
| `default_provider_id` | String | 默认 Provider ID | `null` |
| `default_route_id` | String | 默认路由规则 ID | `null` |
| `rate_limit_enabled` | Boolean | 是否启用限流 | `false` |
| `rate_limit_max_requests_per_minute` | Number | 每分钟最大请求数 | `1000` |
| `rate_limit_max_tokens_per_minute` | Number | 每分钟最大 Token 数 | `500000` |

### 获取设置

```typescript
const settings = await invoke('get_gateway_settings');
```

### 更新设置

```typescript
await invoke('update_gateway_settings', {
  payload: {
    bind_port: 8080,
    allow_remote: false,
    rate_limit_enabled: true,
    rate_limit_max_requests_per_minute: 200,
  }
});
```

---

## 7. 网关 API Key 管理

网关 Key 用于保护网关 HTTP 端点，客户端请求时需要携带 `Authorization: Bearer <gateway-key>` 头。

### 创建网关 Key

```typescript
const result = await invoke('create_gateway_key', {
  payload: {
    name: '我的客户端',
    key_value: '',            // 传空则自动生成
    enabled: true,
    expires_at: null,          // 可选过期时间
    max_concurrent: 10,       // 最大并发数，默认 10
  }
});
// {
//   key: { id, name, key_prefix: "sk-gw-", enabled, ... },
//   plain_key: "sk-gw-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" // 仅创建时返回一次！
// }
```

### 列出网关 Key

```typescript
const keys = await invoke('list_gateway_keys');
// 每个 key 只返回 key_prefix（如 "sk-gw-a1b2"），不返回完整密钥
```

### 更新/删除

```typescript
await invoke('update_gateway_key', { id: 'key-uuid', payload: { enabled: false } });
await invoke('delete_gateway_key', { id: 'key-uuid' });
```

---

## 8. 模型映射管理

模型映射（模型池）将模型名与一组 Provider 渠道关联，支持能力标签、价格管理、负载均衡。

### 创建模型映射

```typescript
const mapping = await invoke('create_model_mapping', {
  payload: {
    model_name: 'gpt-4o',
    max_input_tokens: 128000,
    max_context_tokens: 128000,
    max_output_tokens: 16384,
    input_price_per_1m: 2.5,
    output_price_per_1m: 10.0,
    capabilities: ['chat', 'vision', 'function_calling'],
    description: 'OpenAI 最新旗舰模型',
    strategy: 'round_robin',
    enabled: true,
    channels: [
      { provider_id: 'provider-uuid-1', selected_models: ['gpt-4o'], enabled: true },
      { provider_id: 'provider-uuid-2', selected_models: ['gpt-4o-2024-08-06'], enabled: true },
    ],
  }
});
```

### 查询模型映射

```typescript
// 列表
const mappings = await invoke('list_model_mappings');

// 按 ID
const mapping = await invoke('get_model_mapping', { id: 'mapping-uuid' });

// 按模型名
const mapping = await invoke('find_model_mapping_by_name', { model_name: 'gpt-4o' });
```

### 渠道信息结构

```typescript
interface MappingChannelInfo {
  id: string;
  mapping_id: string;
  provider_id: string;
  provider_name: string;       // 渠道显示名称
  provider_protocols: string[]; // 渠道支持的协议
  provider_models: string[];    // 渠道的全部模型
  provider_models_count: number;
  provider_health: string | null; // "healthy" | "unhealthy" | null
  selected_models: string[];    // 该渠道选中的远程模型
  enabled: boolean;
}
```

---

## 9. 日志管理

### 查询日志

```typescript
const result = await invoke('list_logs', {
  payload: {
    limit: 50,   // 可选，默认 50，最大 500
    offset: 0,   // 可选，默认 0
  }
});
// {
//   logs: [{ id, request_id, timestamp, method, path, response_status, duration_ms, provider_id, provider_name, error_message, ... }],
//   total: 1000,
//   limit: 50,
//   offset: 0,
// }
```

### 日志字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | String | 日志 ID |
| `request_id` | String | 请求 ID |
| `timestamp` | String | 请求时间 |
| `method` | String | HTTP 方法 |
| `path` | String | 请求路径 |
| `route_id` | String | 匹配的路由规则 ID |
| `response_status` | Number | 响应状态码 |
| `duration_ms` | Number | 响应耗时（毫秒） |
| `provider_id` | String | 目标 Provider ID |
| `provider_name` | String | 目标 Provider 名称（从缓存解析） |
| `error_message` | String | 错误信息 |
| `error_code` | String | 错误码 |
| `model_used` | String | 使用的模型名 |
| `retry_count` | Number | 重试次数 |
| `stream_enabled` | Boolean | 是否流式响应 |
| `cache_hit` | Boolean | 是否缓存命中 |
| `tokens_input` | Number | 输入 Token 数 |
| `tokens_output` | Number | 输出 Token 数 |
| `cost` | Number | 估算费用 |
| `auth_key_name` | String | 认证 Key 名称 |

### 清理日志

```typescript
// 清理指定天数之前的日志
const deleted = await invoke('cleanup_logs', {
  payload: { before_days: 30 }
});

// 清空所有日志
const all = await invoke('clear_all_logs');
```

### 导出日志为 CSV

```typescript
const result = await invoke('export_logs_csv', {
  payload: {
    provider_id: null,   // 可选，按 Provider 筛选
    limit: 10000,        // 可选
    offset: 0,           // 可选
    file_path: null,     // 可选，输出文件路径，默认自动命名
  }
});
// { file_path: "silk_logs_20260629_120000.csv", exported_count: 500 }
```

---

## 10. 仪表盘统计

### 仪表盘总览

```typescript
const stats = await invoke('dashboard_stats');
// {
//   today_requests: 120,
//   today_success: 115,
//   today_avg_duration_ms: 450.5,
//   today_tokens: 250000,
//   active_providers: 3,
//   total_requests: 5000,
//   yesterday_requests: 100,
// }
// 可通过 stats.success_rate() 和 stats.growth_rate() 计算百分比
```

### 最近请求

```typescript
const requests = await invoke('recent_requests', { limit: 20 });
// LogResponse[]
```

### 按 Provider 统计

```typescript
const byProvider = await invoke('stats_by_provider', { limit: 10 });
// [{ provider_name: "OpenAI", request_count: 50, avg_duration_ms: 300, total_tokens: 100000 }, ...]
```

### 时序统计（按小时）

```typescript
const hourly = await invoke('hourly_stats', { hours: 24 });
// [{ hour: "2026-06-29T08:00:00", request_count: 10, avg_duration_ms: 400, total_tokens: 20000 }, ...]
```

---

## 11. 网关 HTTP 端点

网关启动后在 `http://127.0.0.1:{port}` 监听 HTTP 请求。

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

所有匹配路由规则的请求（任意路径、任意方法）会被转发到上游 Provider。

```bash
# 示例：直接使用 OpenAI Chat 格式
curl http://127.0.0.1:1877/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-gw-xxxx" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# 示例：使用 Claude Messages 格式
curl http://127.0.0.1:1877/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-gw-xxxx" \
  -d '{
    "model": "claude-3-opus",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### 认证方式

如果配置了网关 API Key，请求需要携带认证头：

| 头 | 说明 |
|----|------|
| `Authorization: Bearer <gateway-key>` | OpenAI 风格 |
| `x-api-key: <gateway-key>` | Claude 风格 |

未配置网关 Key 则不需要认证。

---

## 12. 协议适配与转换

### 支持的协议

| 协议标识 | 说明 |
|----------|------|
| `openai_chat` | OpenAI Chat Completions API（`/v1/chat/completions`） |
| `claude_messages` | Anthropic Claude Messages API（`/v1/messages`） |
| `openai_response` | OpenAI Response API（`/v1/responses`） |

### 协议转换

协议转换由路由规则中的 `inbound_protocol` 和 `outbound_protocol` 控制。适配器直接操作 JSON，不经过中间格式。

```
入站请求 (openai_chat / claude_messages / openai_response)
    ↓
适配器直接转换 JSON
    ↓
上游请求 (目标协议)
```

支持的转换组合（在路由规则中设置 `protocol_conversion: true`）：

| 入站协议 | 出站协议 | 说明 |
|----------|----------|------|
| `claude_messages` | `openai_chat` | Claude 格式 → OpenAI Chat |
| `openai_chat` | `claude_messages` | OpenAI Chat → Claude 格式 |
| 其他组合 | — | 直接透传（仅 URL 代理，不修改请求体） |

### 流式响应（SSE）

Silk 完整支持 SSE 流式响应：

- 每 15 秒发送 `: keep-alive\n\n` 心跳
- 30 秒无数据自动超时
- 自动携带 `Last-Event-ID` 头进行断线重连

---

## 13. 业务概念详解

### Provider 与 API Key 的关系

每个 Provider 可以配置多个 API Key。Key 的选择策略由 `key_strategy` 控制：

- **轮询（round_robin）**：依次循环使用，适合多个相同速率限制的 Key
- **加权（weighted）**：按 weight 比例随机选择，weight 越高概率越大
- **主备（failover）**：始终使用第一个启用的 Key，请求失败后切换到下一个

### 路由规则匹配优先级

1. 仅匹配 `enabled = true` 的规则
2. 按 `priority` 升序排序（数字越小越优先）
3. 规则匹配的严格程度不影响优先级，只决定是否匹配
4. 返回第一条完全匹配的规则

### 模型映射与路由规则的边界

模型映射负责把一个模型名映射到多个 Provider 渠道，并携带能力标签、价格、Token 上限等信息。

路由规则负责按 Host / Path / Method / ContentType 选择目标 Provider，并可选开启协议转换。

### 命令行 vs IPC 命令名

命令行通过 `cargo run` 或 `bun run tauri dev` 启动。IPC 命令名全部使用 snake_case，通过 `invoke("command_name", { ... })` 调用。

---

## 14. 完整配置示例

### 示例：配置 OpenAI 透传

#### 1. 创建 Provider

```typescript
await invoke('create_provider', {
  payload: {
    name: 'OpenAI',
    protocols: ['openai_chat'],
    api_base_url: 'https://api.openai.com',
    models: ['gpt-4', 'gpt-4o', 'gpt-3.5-turbo'],
    keys: [
      { name: '主密钥', value: 'sk-your-key', enabled: true, weight: 1 }
    ],
    key_strategy: 'round_robin',
    timeout_seconds: 60,
    max_retries: 3,
    status: 'enabled',
  }
});
```

#### 2. 创建路由规则

```typescript
await invoke('create_routing_rule', {
  payload: {
    name: 'OpenAI 透传',
    match_path: '/v1/chat/completions',
    match_method: 'POST',
    match_content_type: 'application/json',
    target_provider_id: 'provider-uuid',
    inbound_protocol: 'openai_chat',
    outbound_protocol: 'openai_chat',
    priority: 100,
    enabled: true,
  }
});
```

#### 3. 测试请求

```bash
curl http://127.0.0.1:1877/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### 示例：Claude → OpenAI 协议转换

#### 1. 创建 OpenAI Provider（同上）

#### 2. 创建转换路由

```typescript
await invoke('create_routing_rule', {
  payload: {
    name: 'Claude → OpenAI 转换',
    match_path: '/v1/messages',
    match_method: 'POST',
    target_provider_id: 'openai-provider-uuid',
    inbound_protocol: 'claude_messages',
    outbound_protocol: 'openai_chat',
    protocol_conversion: true,
    priority: 100,
    enabled: true,
  }
});
```

#### 3. 测试

客户端发送 Claude 格式请求，Silk 自动转换为 OpenAI Chat 格式转发到上游。

```bash
curl http://127.0.0.1:1877/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-ant-xxxx" \
  -d '{
    "model": "claude-3-opus",
    "max_tokens": 1024,
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### 示例：多 Key 轮询 + 模型映射回退

```typescript
// 1. 创建两个 Provider
const p1 = await invoke('create_provider', {
  payload: { name: 'API-1', protocols: ['openai_chat'], api_base_url: 'https://api-1.example.com', models: ['gpt-4'], keys: [{ name: 'k1', value: 'sk-1', enabled: true, weight: 1 }], key_strategy: 'round_robin' }
});
const p2 = await invoke('create_provider', {
  payload: { name: 'API-2', protocols: ['openai_chat'], api_base_url: 'https://api-2.example.com', models: ['gpt-4'], keys: [{ name: 'k2', value: 'sk-2', enabled: true, weight: 1 }], key_strategy: 'round_robin' }
});

// 2. 创建模型映射，关联两个渠道
await invoke('create_model_mapping', {
  payload: {
    model_name: 'gpt-4',
    strategy: 'round_robin',
    channels: [
      { provider_id: p1.id, selected_models: ['gpt-4'], enabled: true },
      { provider_id: p2.id, selected_models: ['gpt-4'], enabled: true },
    ],
  }
});

// 3. 路由规则只负责把请求送到这个模型名
await invoke('create_routing_rule', {
  payload: {
    name: 'GPT-4 路由',
    match_path: '/v1/chat/completions',
    target_provider_id: p1.id,
    inbound_protocol: 'openai_chat',
    outbound_protocol: 'openai_chat',
  }
});
```

---

## 附录

### 错误处理

所有 IPC 命令返回 `Result<T, String>`，失败时返回错误描述字符串。

网关 HTTP 代理端的错误响应格式：

```json
{
  "message": "错误描述"
}
```

| 状态码 | 错误码 | 说明 |
|--------|--------|------|
| 400 | `bad_request` | 请求格式错误 |
| 400 | `transform_error` | 协议转换失败 |
| 404 | `not_found` | 路由不存在 |
| 502 | `upstream_error` | 上游请求失败 |
| 504 | `timeout` | 请求超时 |
| 500 | `internal_error` | 内部错误 |
