import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Provider {
  id: string;
  name: string;
  protocols: string[];
  models: string[];
  key_count: number;
  keys: { name: string; value: string; enabled: boolean; weight: number }[];
  api_base_url: string;
  proxy_url: string | null;
  timeout_seconds: number;
  max_retries: number;
  status: string;
  health_status: string | null;
  created_at: string;
  updated_at: string;
}

export interface RoutingRule {
  id: string;
  name: string;
  match_host: string | null;
  match_path: string;
  match_method: string;
  match_content_type: string | null;
  target_provider_id: string;
  target_group_id: string | null;
  inbound_protocol: string | null;
  outbound_protocol: string | null;
  protocol_conversion: boolean;
  model_name_override: string | null;
  priority: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface RequestLog {
  id: string;
  request_id: string;
  timestamp: string;
  method: string;
  path: string;
  route_id: string | null;
  inbound_protocol: string | null;
  outbound_protocol: string | null;
  response_status: number | null;
  /** 响应时间（毫秒） */
  resp_ms: number | null;
  /** 总耗时（毫秒），流式场景末字节 */
  total_duration_ms: number | null;
  provider_id: string | null;
  provider_name: string | null;
  error_message: string | null;
  error_code: string | null;
  /** 实际使用的模型 ID */
  model_id: string | null;
  /** 模型池名称 */
  model_name: string | null;
  retry_count: number;
  stream_enabled: boolean;
  cache_hit: boolean;
  request_size_bytes: number | null;
  response_size_bytes: number | null;
  tokens_input: number | null;
  tokens_output: number | null;
  tokens_sent: number | null;
  cost: number | null;
  auth_key_name: string | null;
  channel_key_name: string | null;
}

export interface GatewaySettings {
  bind_host: string;
  bind_port: number;
  allow_remote: boolean;
  log_retention_days: number;
  default_provider_id: string | null;
  default_route_id: string | null;
  rate_limit_enabled: boolean;
  rate_limit_max_requests_per_minute: number;
  rate_limit_max_tokens_per_minute: number;
}

export interface GatewayStatus {
  running: boolean;
  address: string;
  settings: GatewaySettings;
}

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

/** 从上游 API 获取的模型元信息 */
export interface ProviderModelInfo {
  id: string;
  object: string | null;
  created: number | null;
  owned_by: string | null;
  supported_endpoint_types: string[];
}

export interface ProviderTestResponse {
  status_code: number;
  response_time_ms: number;
  health_status: string;
  error: string | null;
}

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

export interface GatewayKey {
  id: string;
  name: string;
  key_prefix: string;
  enabled: boolean;
  expires_at: string | null;
  max_concurrent: number;
  is_expired: boolean;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// API Client
// ---------------------------------------------------------------------------

export const api = {
  // Gateway
  gatewayStatus: () => invoke<GatewayStatus>("gateway_status"),
  gatewayStart: () => invoke<{ success: boolean; address: string }>("gateway_start"),
  gatewayStop: () => invoke<{ success: boolean; message: string }>("gateway_stop"),
  gatewayRestart: () => invoke<{ success: boolean; address: string }>("gateway_restart"),

  // Providers
  listProviders: () => invoke<Provider[]>("list_providers"),
  getProvider: (id: string) => invoke<Provider>("get_provider", { id }),
  createProvider: (data: Partial<Provider> & { api_key: string }) =>
    invoke<Provider>("create_provider", { payload: data }),
  updateProvider: (id: string, data: Partial<Provider>) =>
    invoke<Provider>("update_provider", { id, payload: data }),
  deleteProvider: (id: string) => invoke<boolean>("delete_provider", { id }),
  testProvider: (id: string) => invoke<ProviderTestResponse>("test_provider", { id }),


  // Routing Rules
  listRoutingRules: () => invoke<RoutingRule[]>("list_routing_rules"),
  getRoutingRule: (id: string) => invoke<RoutingRule>("get_routing_rule", { id }),
  createRoutingRule: (data: Partial<RoutingRule>) =>
    invoke<RoutingRule>("create_routing_rule", { payload: data }),
  updateRoutingRule: (id: string, data: Partial<RoutingRule>) =>
    invoke<RoutingRule>("update_routing_rule", { id, payload: data }),
  deleteRoutingRule: (id: string) => invoke<boolean>("delete_routing_rule", { id }),

  // Logs
  listLogs: (limit = 50, offset = 0) =>
    invoke<{ logs: RequestLog[]; total: number; limit: number; offset: number }>(
      "list_logs",
      { payload: { limit, offset } }
    ),
  cleanupLogs: (beforeDays: number) => invoke<number>("cleanup_logs", { payload: { beforeDays } }),
  clearAllLogs: () => invoke<number>("clear_all_logs"),

  // Dashboard Stats
  dashboardStats: () => invoke<DashboardStats>("dashboard_stats"),
  recentRequests: (limit = 20) => invoke<RequestLog[]>("recent_requests", { limit }),
  statsByProvider: (limit = 10) => invoke<ProviderStats[]>("stats_by_provider", { limit }),
  hourlyStats: (hours = 24) => invoke<HourlyStats[]>("hourly_stats", { hours }),

  // Fetch Models from Provider API
  fetchProviderModels: (data: { api_base_url: string; api_key: string; proxy_url?: string; timeout_seconds?: number }) =>
    invoke<ProviderModelInfo[]>("fetch_provider_models", { payload: data }),

  // Model Mappings
  listModelMappings: () => invoke<ModelMapping[]>("list_model_mappings"),
  getModelMapping: (id: string) => invoke<ModelMapping>("get_model_mapping", { id }),
  findModelMappingByName: (name: string) => invoke<ModelMapping | null>("find_model_mapping_by_name", { model_name: name }),
  createModelMapping: (data: {
    model_name: string;
    strategy?: string;
    max_input_tokens?: number | null;
    max_context_tokens?: number | null;
    max_output_tokens?: number | null;
    input_price_per_1m?: number | null;
    output_price_per_1m?: number | null;
    capabilities?: string[];
    description?: string;
    enabled?: boolean;
    channels?: NewMappingChannel[];
  }) => invoke<ModelMapping>("create_model_mapping", { payload: data }),
  updateModelMapping: (id: string, data: {
    model_name?: string;
    strategy?: string;
    max_input_tokens?: number | null;
    max_context_tokens?: number | null;
    max_output_tokens?: number | null;
    input_price_per_1m?: number | null;
    output_price_per_1m?: number | null;
    capabilities?: string[];
    description?: string;
    enabled?: boolean;
    channels?: NewMappingChannel[];
  }) => invoke<ModelMapping>("update_model_mapping", { id, payload: data }),
  deleteModelMapping: (id: string) => invoke<boolean>("delete_model_mapping", { id }),

  // Gateway Keys
  listGatewayKeys: () => invoke<GatewayKey[]>("list_gateway_keys"),
  createGatewayKey: (data: { name: string; key_value: string; enabled?: boolean; max_concurrent?: number }) =>
    invoke<{ key: GatewayKey; plain_key: string }>("create_gateway_key", { payload: data }),
  updateGatewayKey: (id: string, data: { name?: string; enabled?: boolean; max_concurrent?: number }) =>
    invoke<GatewayKey>("update_gateway_key", { id, payload: data }),
  deleteGatewayKey: (id: string) => invoke<boolean>("delete_gateway_key", { id }),

  // Logs CSV Export
  exportLogsCsv: (data: { provider_id?: string; limit?: number; file_path?: string }) =>
    invoke<{ file_path: string; exported_count: number }>("export_logs_csv", { payload: data }),

  // Settings
  updateGatewaySettings: (data: Partial<GatewaySettings>) =>
    invoke<GatewaySettings>("update_gateway_settings", { payload: data }),
};
