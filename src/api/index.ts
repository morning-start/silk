import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Provider {
  id: string;
  name: string;
  provider_type: string;
  api_base_url: string;
  model_name: string | null;
  proxy_url: string | null;
  timeout_seconds: number;
  max_retries: number;
  status: string;
  health_status: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProviderGroup {
  id: string;
  name: string;
  model_name: string;
  strategy: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface GroupMember {
  id: string;
  group_id: string;
  provider_id: string;
  weight: number;
  enabled: boolean;
}

export interface GroupWithMembers {
  group: ProviderGroup;
  members: GroupMemberInfo[];
}

export interface GroupMemberInfo {
  id: string;
  group_id: string;
  provider_id: string;
  provider_name: string;
  weight: number;
  enabled: boolean;
}

export interface RoutingRule {
  id: string;
  name: string;
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
  duration_ms: number | null;
  provider_id: string | null;
  error_message: string | null;
  model_used: string | null;
  retry_count: number;
  stream_enabled: boolean;
  request_size_bytes: number | null;
  response_size_bytes: number | null;
  tokens_input: number | null;
  tokens_output: number | null;
}

export interface GatewaySettings {
  id: string;
  bind_host: string;
  bind_port: number;
  allow_remote: boolean;
  auth_token_hash: string | null;
  log_retention_days: number;
  default_provider_id: string | null;
  default_route_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface GatewayStatus {
  running: boolean;
  address: string;
  settings: GatewaySettings;
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

  // Groups
  listGroups: () => invoke<ProviderGroup[]>("list_groups"),
  findGroupsByModel: (modelName: string) =>
    invoke<ProviderGroup[]>("find_groups_by_model", { modelName }),
  getGroup: (id: string) => invoke<GroupWithMembers>("get_group", { id }),
  createGroup: (data: { name: string; model_name: string; strategy?: string; enabled?: boolean }) =>
    invoke<ProviderGroup>("create_group", { payload: data }),
  updateGroup: (id: string, data: Partial<ProviderGroup>) =>
    invoke<ProviderGroup>("update_group", { id, payload: data }),
  deleteGroup: (id: string) => invoke<boolean>("delete_group", { id }),
  addGroupMember: (groupId: string, data: { provider_id: string; weight?: number }) =>
    invoke<GroupMember>("add_group_member", { groupId, payload: data }),
  updateGroupMember: (id: string, data: { weight?: number; enabled?: boolean }) =>
    invoke<GroupMember>("update_group_member", { id, payload: data }),
  removeGroupMember: (id: string) => invoke<boolean>("remove_group_member", { id }),

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
  logsByProvider: (providerId: string, limit = 50) =>
    invoke<RequestLog[]>("logs_by_provider", { providerId, limit }),
  logsByRequestId: (requestId: string) =>
    invoke<RequestLog[]>("logs_by_request_id", { requestId }),
  countLogs: () => invoke<number>("count_logs"),
  cleanupLogs: (beforeDays: number) => invoke<number>("cleanup_logs", { payload: { beforeDays } }),
  clearAllLogs: () => invoke<number>("clear_all_logs"),

  // Settings
  getGatewaySettings: () => invoke<GatewaySettings>("get_gateway_settings"),
  updateGatewaySettings: (data: Partial<GatewaySettings>) =>
    invoke<GatewaySettings>("update_gateway_settings", { payload: data }),
};
