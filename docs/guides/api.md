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

### 1.8 协议内核（prism.wasm）

协议转换内核的检查、下载与更新。内核发布仓库为 `morning-start/prism`，
每个 `v*` 发布提供 `prism.wasm`、`prism.wasm.sha256`、`prism.release.json`。

#### `get_kernel_status`
读取当前运行内核的版本、ABI 与来源位置。

**参数**：无

**返回值**：
```typescript
interface KernelStatus {
  version: string | null;      // 仅数据目录来源可反查
  abi: string | null;          // 内核不支持 ABI 探测时为 null
  ir_schema: string | null;
  source: string;              // exe_dir / data_dir / embedded
  path: string | null;         // 内嵌来源为 null
  supported_abi: string;       // 宿主兼容的 ABI
  updatable: boolean;          // 程序目录抢占时为 false
  updatable_reason: string | null;
}
```

#### `check_kernel_update`
检查内核更新。沿用「设置 → 请求与限流」的全局代理。

**参数**：无

**返回值**：
```typescript
interface KernelInfo {
  current_version: string | null;
  latest_version: string | null;
  available: boolean;
  name: string | null;
  notes: string | null;
  published_at: string | null;
  release_url: string | null;
  asset_size: number | null;
  can_install: boolean;        // 程序目录抢占或发布缺资产时为 false
  blocked_reason: string | null;
}
```

#### `install_kernel_update`
下载并安装最新内核。**两道校验关**：

1. **SHA-256**：与发布方的 `.sha256` 比对，证明字节未被篡改；
2. **ABI 探测**：用 wasmtime 独立实例化并调用 `wasm_abi_version()`，
   证明内核可运行且导出签名与宿主兼容（`abi` 必须等于 `1`）。

任一步失败都在落盘前中止，旧内核保持原样。通过后备份现有内核为
`prism.wasm.bak`，原子替换 `prism.wasm`，并写入 `prism.release.json` 构建清单。

**参数**：无

**返回值**：
```typescript
interface KernelInstallResult {
  version: string | null;
  abi: string;
  source: string;              // 安装后恒为 data_dir
  requires_restart: boolean;   // 内核是进程级单例，恒为 true
}
```

**错误码**：
- `kernel_externally_managed` — 程序目录下的 prism.wasm 抢占加载权
- `kernel_asset_missing` — 发布缺少必需资产
- `kernel_probe_failed` — 内核无法实例化
- `kernel_abi_mismatch` — ABI 与宿主不兼容

#### `restart_app`
重启应用（内核更新后生效用）。直接使用 Tauri 核心 `AppHandle::restart()`，
无需额外插件。

**参数**：无

**返回值**：无

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
