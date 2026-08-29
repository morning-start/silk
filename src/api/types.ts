// ---------------------------------------------------------------------------
// 网关相关类型
// ---------------------------------------------------------------------------

export interface GatewaySettings {
  bind_host: string;
  bind_port: number;
  allow_remote: boolean;
  log_retention_days: number;
  launch_at_startup: boolean;
  minimize_to_tray: boolean;
  close_to_tray: boolean;
  auto_start_gateway: boolean;
  default_provider_id: string | null;
  rate_limit_enabled: boolean;
  rate_limit_max_requests_per_minute: number;
  rate_limit_max_tokens_per_minute: number;
}

export interface GatewayStatus {
  running: boolean;
  address: string;
  settings: GatewaySettings;
}

// ---------------------------------------------------------------------------
// Provider 相关类型
// ---------------------------------------------------------------------------

export interface ProviderHeaderEntry {
  name: string;
  value: string;
  enabled: boolean;
}

export interface Provider {
  id: string;
  name: string;
  protocols: string[];
  models: string[];
  key_count: number;
  keys: { name: string; value: string; enabled: boolean; weight: number }[];
  key_strategy: string;
  api_base_url: string;
  proxy_url: string | null;
  timeout_seconds: number;
  max_retries: number;
  status: string;
  health_status: string | null;
  custom_headers: ProviderHeaderEntry[];
  models_passthrough: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProviderTestResponse {
  status_code: number;
  response_time_ms: number;
  health_status: string;
  error: string | null;
}

/** 从上游 API 获取的模型元信息 */
export interface ProviderModelInfo {
  id: string;
  object: string | null;
  created: number | null;
  owned_by: string | null;
  supported_endpoint_types: string[];
}

// ---------------------------------------------------------------------------
// 日志相关类型
// ---------------------------------------------------------------------------

export interface RequestLog {
  id: string;
  request_id: string;
  timestamp: string;
  method: string;
  path: string;
  inbound_protocol: string | null;
  outbound_protocol: string | null;
  response_status: number | null;
  resp_ms: number | null;
  total_duration_ms: number | null;
  provider_id: string | null;
  provider_name: string | null;
  error_message: string | null;
  error_code: string | null;
  model_id: string | null;
  model_name: string | null;
  retry_count: number;
  stream_enabled: boolean;
  cache_hit: boolean;
  request_size_bytes: number | null;
  response_size_bytes: number | null;
  tokens_input: number | null;
  tokens_output: number | null;
  tokens_sent: number | null;
  auth_key_name: string | null;
  channel_key_name: string | null;
}

// ---------------------------------------------------------------------------
// 统计相关类型
// ---------------------------------------------------------------------------

export interface DashboardStats {
  today_requests: number;
  today_success: number;
  today_avg_duration_ms: number;
  today_tokens: number;
  active_providers: number;
  total_requests: number;
  yesterday_requests: number;
}

export interface ProviderStats {
  provider_name: string | null;
  request_count: number;
  avg_duration_ms: number;
  total_tokens: number;
}

export interface HourlyStats {
  hour: string;
  request_count: number;
  avg_duration_ms: number;
  total_tokens: number;
}

// ---------------------------------------------------------------------------
// 模型映射相关类型
// ---------------------------------------------------------------------------

export interface MappingChannelInfo {
  id: string;
  mapping_id: string;
  provider_id: string;
  provider_name: string;
  provider_protocols: string[];
  provider_models: string[];
  provider_models_count: number;
  provider_health: string | null;
  selected_models: string[];
  enabled: boolean;
}

export interface NewMappingChannel {
  provider_id: string;
  selected_models?: string[];
  enabled?: boolean;
}

export interface ModelMapping {
  id: string;
  model_name: string;
  strategy: string;
  max_input_tokens: number | null;
  max_context_tokens: number | null;
  max_output_tokens: number | null;
  input_price_per_1m: number | null;
  output_price_per_1m: number | null;
  capabilities: string[];
  description: string;
  enabled: boolean;
  channels: MappingChannelInfo[];
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Gateway Key 类型
// ---------------------------------------------------------------------------

export interface GatewayKey {
  id: string;
  name: string;
  plain_key: string;
  enabled: boolean;
  expires_at: string | null;
  max_concurrent: number;
  is_expired: boolean;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Profile 类型
// ---------------------------------------------------------------------------

export type AgentType = "claude_code" | "codex" | "gemini_cli" | "opencode" | "hermes";

export interface Profile {
  id: string;
  name: string;
  agent_type: AgentType;
  config_json: string;
  is_active: boolean;
  sort_index?: number;
  created_at: string;
  updated_at: string;
}

export interface CreateProfilePayload {
  agent_type: AgentType;
  name: string;
  config_json: string;
  sort_index?: number;
}

export interface UpdateProfilePayload {
  name?: string;
  config_json?: string;
  sort_index?: number;
}

export interface SwitchResult {
  success: boolean;
  warnings: string[];
  requires_restart: boolean;
}

// ---------------------------------------------------------------------------
// 其他类型
// ---------------------------------------------------------------------------

export interface ModelListingItem {
  id: string;
  object: string;
  created: number;
  owned_by: string;
  model_mapping_id: string | null;
}

export interface FileOperationResponse {
  file_path: string;
}
