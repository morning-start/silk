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

从本地数据库 `model_mappings` 表中查询所有启用的模型映射记录，以 OpenAI `/v1/models` 响应格式返回本地模型池。该接口不会请求上游 Provider。

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
| `id` | 模型映射名（`model_mappings.model_name`） |
| `object` | 固定为 `model` |
| `created` | `model_mappings.created_at` 的时间戳 |
| `owned_by` | 模型映射的 `vendor` 字段；未配置时回退为 `silk` |

## 7. 网关代理入口

### ANY `/*`

所有非 `GET /health` 的请求都会进入网关管线。真正是否转发、转发到哪个 Provider、采用什么协议，由本地配置决定。

### 7.1 路由优先级

`resolve_route` 阶段的路由决策顺序如下：

1. **特殊路径短路**：如果请求路径为 `/v1/models`，直接返回本地模型池数据，不进入后续路由和上游转发。

2. **模型映射优先**：先读取请求体 JSON 中的 `model` 字段，在 `model_mappings` 表中查找启用映射。
   - 如果命中模型映射，通过关联渠道表加载所有可用的 Provider 渠道，按映射配置的负载均衡策略（如 `round_robin`）选中一个 Provider。
   - 同时根据 Provider 的 `protocols` 字段确定出站协议，根据请求路径和体结构确定入站协议。
   - 如果渠道后续请求失败，支持自动回退到下一个可用渠道。

3. **路由规则降级**：如果模型映射未命中，再按 `RoutingRule` 匹配（匹配维度：Host + Path + Method + ContentType）。
   - 如果路由规则命中且指定了 `target_provider_id`，直接使用该 Provider。
    - 如果路由规则指定了 `target_group_id`，该字段仅作为历史兼容保留，不再进入独立分组负载均衡流程。
   - 路由规则也决定了入站/出站协议映射。

4. **最终结果**：路由成功后，`resolve_route` 阶段设置 `ctx.provider`、`inbound_protocol`、`outbound_protocol` 和 `adapter_registry`，后续阶段使用这些信息进行协议转换和上游转发。

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

### 7.3 上游目标路径

适配器在 `transform_request` 阶段将目标路径拼接到 Provider 的 `api_base_url` 上，构造完整上游 URL。不同适配器对应不同的目标路径：

| 适配器 | 目标路径 | 请求方法 |
|---|---|---|
| `openai_chat` | `/v1/chat/completions` | POST |
| `claude_messages` | `/v1/messages` | POST |
| `openai_response` | `/v1/responses` | POST |

例如，Provider 的 `api_base_url` 为 `https://api.openai.com` 且使用 `openai_chat` 适配器时，最终上游 URL 为 `https://api.openai.com/v1/chat/completions`。

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

**silk 自身产生的错误**（网关鉴权、路由、协议转换、连不通上游、超时、限流等）
统一带 `【silk】` 标记，并同时给出顶层 `message` 与标准 `error` 对象（适配
OpenAI 系 SDK、Claude Code、Codex 等按标准形状取错的客户端）：

```json
{
  "message": "【silk】未授权: Key 错误",
  "error": {
    "message": "【silk】未授权: Key 错误",
    "type": "unauthorized",
    "origin": "silk"
  }
}
```

> **判定规则**：响应里带 `【silk】` ⇒ 问题出在 silk 侧；**不带** ⇒ 是上游的原话
> （见 [9.3](#93-上游错误原样透传)）。

历史教训：上游 403「模型不在当前套餐内」曾被统一文案成「认证失败 / API 密钥错误」，
把用户引向改 Key 的错误方向 —— 实际上密钥有效，改 Key 无用。

### 9.2 状态码与错误码

**silk 侧错误**（`origin: silk`）：

| HTTP 状态码 | 错误码 | 说明 |
|---|---|---|
| 400 | `bad_request` | 请求体读取失败、协议或方法不合法 |
| 400 | `transform_error` | 协议转换失败 |
| 401 | `unauthorized` | 缺少或错误的 Gateway Key |
| 404 | `not_found` | 路由、模型或 Provider 未命中 |
| 429 | `too_many_requests` | 触发本地限流 |
| 500 | `database_error` | 数据库访问失败 |
| 500 | `internal_error` | 内部错误；回退耗尽且上游从未返回过错误时使用 |
| 500 | `serialization_error` | 序列化/反序列化失败 |
| 502 | `upstream_unreachable` | **连不上**上游（DNS/连接/TLS/读超时）—— 上游没答话，是 silk 侧的报告 |
| 504 | `timeout` | SSE 流超时或回退总超时 |

**上游侧错误**（`origin: upstream`，状态码与错误体均由上游决定）：

| 错误码 | 说明 |
|---|---|
| `upstream_error` | 上游**确实返回了** 4xx/5xx 响应，网关原样透传。状态码不固定映射到本表，取决于上游 |

> 注：`upstream_unreachable`（silk 连不上）与 `upstream_error`（上游返回了错误）
> 必须区分 —— 前者不是上游说过的话，不能伪装成上游错误体。

### 9.3 上游错误原样透传

当上游返回明确的 HTTP 错误时，网关**不做任何加工**：

- **状态码**：保留上游原始状态码；
- **响应体**：返回上游**原始字节**，不解析、不重包装、不截断。上游的错误体不一定是
  JSON —— nginx 的 HTML 502、网关的纯文本 401 都很常见，强行解析失败后再包一层
  `{"error":{"message":…}}` 等于篡改了上游的原话，还会丢掉原始信息；
- **Content-Type**：原样转发，与上游保持一致。

因此客户端收到什么形状，取决于上游返回什么形状：

```json
// 上游是 OpenAI 系时（原样透传，无 【silk】 标记）
{
  "error": {
    "message": "model is not available in the current token plan",
    "type": "permission_denied_error",
    "code": "7"
  }
}
```

**回退场景**：若一次请求在多个渠道间回退且最终全部失败，网关返回**最后一次**
上游错误（原样透传），而不是 silk 自造的「所有渠道和 Key 均已失败」——
后者掩盖了上游的真实原因。仅当上游全程从未返回过任何错误响应时，
才回落到 `internal_error`。

**流式场景**：流式响应一旦发出 200 响应头就无法再改状态码。此时网关会把错误作为
一条 SSE `error` 事件写进流里再正常收尾（上游中途断流、流超时均如此），
否则 hyper 会直接中止 body，客户端只能看到「连接被重置」而拿不到原因。

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
- `/v1/*` 路径的请求需要认证。网关同时支持 `Authorization: Bearer <key>`（OpenAI 风格）和 `x-api-key: <key>`（Anthropic 风格）两种认证方式，认证令牌会做哈希后在本地 `gateway_keys` 表中校验
- 本文档以当前代码实现为准
