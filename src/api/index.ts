// ---------------------------------------------------------------------------
// 领域 API 模块
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 类型重导出
// ---------------------------------------------------------------------------

export type {
  GatewaySettings,
  GatewayStatus,
  Provider,
  ProviderHeaderEntry,
  ProviderTestResponse,
  ProviderModelInfo,
  RequestLog,
  DashboardStats,
  ProviderStats,
  HourlyStats,
  MappingChannelInfo,
  NewMappingChannel,
  ModelMapping,
  GatewayKey,
  ModelListingItem,
  AgentTypeInfo,
  Preset,
  NewPresetPayload,
  UpdatePresetPayload,
  PresetSwitchResult,
  PresetDefaults,
  FileOperationResponse,
} from "./types";

// ---------------------------------------------------------------------------
// 领域 API 独立导出（新代码推荐按需导入）
// ---------------------------------------------------------------------------

export { gatewayApi } from "./gateway";
export { providersApi } from "./providers";
export { logsApi } from "./logs";
export { statsApi } from "./stats";
export { modelMappingsApi } from "./model-mappings";
export { presetsApi } from "./presets";
export { configApi } from "./config";
