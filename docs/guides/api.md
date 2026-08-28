# Silk API 文档

## 1. Tauri Commands API

Tauri Commands 是前端与后端通信的接口。

### 1.1 Gateway 控制

#### `gateway_status`
获取网关状态。

**参数**：无

**返回值**：
```typescript
interface GatewayStatusResponse {
  running: boolean;
  address: string;
  settings: GatewaySettingsInfo;
}
```

**示例**：
```typescript
const status = await invoke('gateway_status');
console.log(status.running); // true/false
console.log(status.address); // "127.0.0.1:1877"
```

#### `gateway_start`
启动网关。

**参数**：无

**返回值**：
```typescript
interface GatewayStartResponse {
  success: boolean;
  address: string;
}
```

#### `gateway_stop`
停止网关。

**参数**：无

**返回值**：
```typescript
interface GatewayStopResponse {
  success: boolean;
  message: string;
}
```

#### `gateway_restart`
重启网关。

**参数**：无

**返回值**：同 `gateway_start`

### 1.2 Provider 管理

#### `list_providers`
获取所有Provider列表。

**参数**：无

**返回值**：
```typescript
interface Provider {
  id: string;
  name: string;
  protocols: string[];
  models: string[];
  api_base_url: string;
  status: string;
}
```

#### `create_provider`
创建新的Provider。

**参数**：
```typescript
interface CreateProviderRequest {
  name: string;
  protocols: string[];
  models: string[];
  api_base_url: string;
  api_key: string;
}
```

**返回值**：`Provider`

#### `update_provider`
更新Provider。

**参数**：
```typescript
interface UpdateProviderRequest {
  id: string;
  name?: string;
  protocols?: string[];
  models?: string[];
  api_base_url?: string;
  api_key?: string;
}
```

**返回值**：`Provider`

#### `delete_provider`
删除Provider。

**参数**：
```typescript
interface DeleteProviderRequest {
  id: string;
}
```

**返回值**：`void`

### 1.3 日志管理

#### `list_logs`
获取请求日志列表。

**参数**：
```typescript
interface ListLogsRequest {
  limit?: number;
  offset?: number;
  provider_id?: string;
  status_code?: number;
  start_time?: string;
  end_time?: string;
}
```

**返回值**：
```typescript
interface RequestLog {
  request_id: string;
  provider_name: string;
  model_name: string;
  status_code: number;
  total_duration_ms: number;
  tokens_input: number;
  tokens_output: number;
  created_at: string;
}
```

### 1.4 设置管理

#### `get_gateway_settings`
获取网关设置。

**参数**：无

**返回值**：
```typescript
interface GatewaySettings {
  bind_host: string;
  bind_port: number;
  allow_remote: boolean;
  auto_start_gateway: boolean;
  launch_at_startup: boolean;
  close_to_tray: boolean;
}
```

#### `update_gateway_settings`
更新网关设置。

**参数**：
```typescript
interface UpdateGatewaySettingsRequest {
  bind_host?: string;
  bind_port?: number;
  allow_remote?: boolean;
  auto_start_gateway?: boolean;
  launch_at_startup?: boolean;
  close_to_tray?: boolean;
}
```

**返回值**：`GatewaySettings`

### 1.5 自动检测

#### `detect_installed_ai_apps`
检测已安装的AI应用。

**参数**：无

**返回值**：
```typescript
interface InstalledAiApp {
  name: string;
  description: string;
  installed: boolean;
  config_path: string | null;
  icon: string;
  color: string;
}
```

### 1.6 快速配置

#### `save_onboarding_config`
保存引导配置。

**参数**：
```typescript
interface SaveOnboardingConfigRequest {
  services: string[];
  apiKeys: Record<string, string>;
}
```

**返回值**：
```typescript
interface QuickSetupResponse {
  success: boolean;
  message: string;
  configured_services: string[];
}
```

### 1.7 预置配置

#### `get_preset_providers`
获取所有预置配置。

**参数**：无

**返回值**：
```typescript
interface PresetProvider {
  id: string;
  name: string;
  description: string;
  protocols: string[];
  models: PresetModel[];
  api_base_url: string;
  api_key_url: string;
  api_key_placeholder: string;
  color: string;
}
```

#### `get_preset_provider_by_id`
根据ID获取预置配置。

**参数**：
```typescript
interface GetPresetProviderByIdRequest {
  id: string;
}
```

**返回值**：`PresetProvider | null`

## 2. Gateway API

Gateway API 是Silk对外提供的HTTP API。

### 2.1 健康检查

#### `GET /health`
检查网关是否正常运行。

**响应**：
```json
{
  "status": "ok",
  "service": "silk-gateway"
}
```

### 2.2 Chat Completions

#### `POST /v1/chat/completions`
OpenAI Chat Completions 兼容接口。

**请求体**：
```json
{
  "model": "gpt-4",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "temperature": 0.7,
  "stream": false
}
```

**响应体**：
```json
{
  "id": "chatcmpl-123",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gpt-4",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 15,
    "total_tokens": 25
  }
}
```

### 2.3 Claude Messages

#### `POST /v1/messages`
Claude Messages 兼容接口。

**请求体**：
```json
{
  "model": "claude-3-opus-20240229",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "Hello"}
  ]
}
```

**响应体**：
```json
{
  "id": "msg_123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "Hello! How can I help you?"
    }
  ],
  "model": "claude-3-opus-20240229",
  "stop_reason": "end_turn",
  "usage": {
    "input_tokens": 10,
    "output_tokens": 15
  }
}
```

## 3. 错误码

### 3.1 Tauri Commands 错误码

| 错误码 | 描述 | 解决方案 |
|--------|------|----------|
| `DB_NOT_INITIALIZED` | 数据库未初始化 | 重启应用 |
| `PROVIDER_NOT_FOUND` | Provider不存在 | 检查Provider ID |
| `INVALID_API_KEY` | API密钥无效 | 更新API密钥 |
| `GATEWAY_ALREADY_RUNNING` | 网关已在运行 | 先停止网关 |
| `GATEWAY_NOT_RUNNING` | 网关未运行 | 先启动网关 |

### 3.2 Gateway API 错误码

| HTTP状态码 | 描述 | 解决方案 |
|------------|------|----------|
| 400 | 请求格式错误 | 检查请求体格式 |
| 401 | 认证失败 | 检查API密钥 |
| 429 | 请求过于频繁 | 降低请求频率 |
| 500 | 服务器内部错误 | 稍后重试 |
| 502 | 上游服务不可用 | 检查AI服务状态 |
| 503 | 网关未运行 | 启动网关 |
