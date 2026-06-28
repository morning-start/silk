---
doc:
  type: api
  version: "1.0.0"
  status: draft
  scope: local-gateway
---

# Silk 中转站 API 文档

## 1. 概述

Silk 的对外 HTTP 能力是一个本地网关，监听 `http://127.0.0.1:{bind_port}`，将客户端请求按路由规则和模型映射转发到上游 AI Provider。

当前实现对外暴露的 HTTP 接口很少：

- `GET /health` 用于健康检查
- `ANY /*` 作为网关兜底入口，由中间件管线处理所有其它请求

其中，`/v1/models` 是网关内置的特殊路径，不是独立服务，而是由路由阶段直接返回本地模型列表。

## 2. 基础信息

- 协议：HTTP
- Base URL：`http://127.0.0.1:{bind_port}`
- 数据格式：`application/json`
- 认证范围：仅 `/v1/*` 请求需要 `Authorization` 头
- 部署形态：纯本地桌面进程，不经过云端中转

## 3. 认证

### 3.1 认证方式

`/v1/*` 请求必须携带：

```http
Authorization: Bearer <gateway-key>
```

认证逻辑会对 Bearer Token 做哈希后，在本地数据库的 `gateway_keys` 中查找。

### 3.2 认证失败

认证失败时返回 `401`，响应体为：

```json
{
  "message": "未授权: 缺少 Key"
}
```

或：

```json
{
  "message": "未授权: Key 错误"
}
```

## 4. 接口总览

| 方法 | 路径 | 是否认证 | 说明 |
|---|---|---:|---|
| GET | `/health` | 否 | 健康检查 |
| ANY | `/*` | 视路径而定 | 进入网关管线 |
| GET | `/v1/models` | 是 | 返回本地启用模型列表 |
| GET/POST/... | `/v1/*` | 是 | OpenAI / Claude 兼容请求入口 |
| GET/POST/... | 其它路径 | 否 | 仅在路由规则命中时转发 |

## 5. 健康检查

### GET `/health`

用于进程存活和端口可达性检查。

#### 响应

```json
{
  "status": "ok",
  "service": "silk-gateway"
}
```

## 6. 模型列表

### GET `/v1/models`

返回本地数据库中启用的模型映射列表。该接口不会请求上游 Provider。

#### 请求头

```http
Authorization: Bearer <gateway-key>
```

#### 响应示例

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1710000000,
      "owned_by": "openai"
    }
  ]
}
```

#### 字段说明

| 字段 | 说明 |
|---|---|
| `id` | 本地模型名 |
| `object` | 固定为 `model` |
| `created` | 本地记录创建时间戳 |
| `owned_by` | 供应商名称；未配置时回退为 `silk` |

## 7. 网关代理入口

### ANY `/*`

所有非 `GET /health` 的请求都会进入网关管线。真正是否转发、转发到哪个 Provider、采用什么协议，由本地配置决定。

### 7.1 路由优先级

当前路由决策顺序如下：

1. 先尝试读取请求体里的 `model` 字段
2. 如果命中本地模型映射，按模型映射和渠道负载均衡选 Provider
3. 如果没有命中模型映射，再按路由规则匹配 `host + path + method + content-type`
4. 路由成功后，根据 Provider 支持的协议和路由配置确定上游目标

### 7.2 支持的入站协议

网关当前支持三类兼容协议：

- OpenAI Chat Completions
- Claude Messages
- OpenAI Responses

入站协议是通过请求体结构自动识别的：

| 请求体顶层字段 | 识别结果 |
|---|---|
| `input` | `openai_response` |
| `messages` | `openai_chat` |
| 其它 | 默认 `openai_chat` |

### 7.3 支持的上游路径

协议适配器会把请求转发到 Provider 的 `api_base_url` 下的固定路径：

| 适配器 | 上游路径 |
|---|---|
| `openai_chat` | `/v1/chat/completions` |
| `claude_messages` | `/v1/messages` |
| `openai_response` | `/v1/responses` |

### 7.4 请求体限制

网关读取请求体时的上限是 `2 MiB`。超过该限制会返回 `400 Bad Request`。

### 7.5 头部转发规则

网关不会原样转发所有请求头，只会保留少量必要头部并注入上游所需的认证头。实现上会特别保留：

- `user-agent`
- `accept`
- `x-request-id`
- `x-trace-id`

## 8. 流式响应

网关支持 SSE 流式转发，并提供：

- 自动心跳保活
- `Last-Event-ID` 续传
- 流超时处理

注意：

- 流式场景下不会做 chunk 级协议转换
- 同协议流转可透传
- 跨协议流式转换当前不做增量级变换

## 9. 错误响应

### 9.1 通用错误格式

非上游原样透传错误时，响应体统一为：

```json
{
  "message": "错误描述"
}
```

### 9.2 状态码与错误码

| HTTP 状态码 | 错误码 | 说明 |
|---|---|---|
| 400 | `bad_request` | 请求体读取失败、协议或方法不合法 |
| 400 | `transform_error` | 协议转换失败 |
| 401 | `unauthorized` | 缺少或错误的 Gateway Key |
| 404 | `not_found` | 路由、模型或 Provider 未命中 |
| 429 | `too_many_requests` | 触发限流 |
| 500 | `database_error` | 数据库访问失败 |
| 500 | `internal_error` | 内部错误 |
| 500 | `serialization_error` | 序列化失败 |
| 502 | `upstream_error` | 请求上游失败 |
| 504 | `timeout` | SSE 或上游请求超时 |

### 9.3 上游错误透传

当上游返回明确的 HTTP 错误时，网关会尽量保留上游状态码，并返回错误体：

```json
{
  "error": {
    "message": "upstream error message"
  }
}
```

## 10. 示例

### 10.1 健康检查

```bash
curl http://127.0.0.1:1234/health
```

### 10.2 获取模型列表

```bash
curl http://127.0.0.1:1234/v1/models \
  -H "Authorization: Bearer sk-gw-xxxx"
```

### 10.3 OpenAI Chat 请求

```bash
curl http://127.0.0.1:1234/v1/chat/completions \
  -H "Authorization: Bearer sk-gw-xxxx" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      { "role": "user", "content": "Hello" }
    ]
  }'
```

### 10.4 Claude Messages 请求

```bash
curl http://127.0.0.1:1234/v1/messages \
  -H "Authorization: Bearer sk-gw-xxxx" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-opus",
    "messages": [
      { "role": "user", "content": "Hello" }
    ]
  }'
```

### 10.5 OpenAI Responses 请求

```bash
curl http://127.0.0.1:1234/v1/responses \
  -H "Authorization: Bearer sk-gw-xxxx" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "input": "Hello"
  }'
```

## 11. 不属于 HTTP 对外 API 的内容

以下能力存在于本地进程内部，但不属于网关 HTTP 对外接口：

- Tauri IPC 管理命令
- Provider / RoutingRule / GatewayKey 的本地增删改查
- SQLite 持久化与日志写入
- GUI 设置页操作

## 12. 备注

- 当前网关是本地代理，不是多租户公网服务
- `/v1/*` 是否最终转发，取决于本地模型映射和路由规则
- 本文档以当前代码实现为准

