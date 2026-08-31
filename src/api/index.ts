// ---------------------------------------------------------------------------
// 领域 API 模块
// ---------------------------------------------------------------------------

import { invoke } from "@tauri-apps/api/core";
import { gatewayApi } from "./gateway";
import { providersApi } from "./providers";
import { logsApi } from "./logs";
import { statsApi } from "./stats";
import { modelMappingsApi } from "./model-mappings";
import { profilesApi } from "./profiles";
import { configApi } from "./config";
import type { GatewaySettings } from "./types";

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
  Profile,
  AgentType,
  CreateProfilePayload,
  UpdateProfilePayload,
  SwitchResult,
  ModelListingItem,
  AgentLiveStatus,
  ImportableProvider,
  FileOperationResponse,
} from "./types";

// ---------------------------------------------------------------------------
// 向后兼容的聚合 API（保持旧的方法名）
// ---------------------------------------------------------------------------

export const api = {
  // Gateway
  gatewayStatus: gatewayApi.status,
  gatewayStart: gatewayApi.start,
  gatewayStop: gatewayApi.stop,
  gatewayRestart: gatewayApi.restart,
  getGatewaySettings: () => invoke<GatewaySettings>("get_gateway_settings"),
  updateGatewaySettings: gatewayApi.updateSettings,

  // Providers
  listProviders: providersApi.list,
  getProvider: providersApi.get,
  createProvider: providersApi.create,
  updateProvider: providersApi.update,
  deleteProvider: providersApi.remove,
  testProvider: providersApi.test,
  fetchProviderModels: providersApi.fetchModels,

  // Logs
  listLogs: (limit = 50, offset = 0) =>
    logsApi.list(limit, offset),
  cleanupLogs: logsApi.cleanup,
  clearAllLogs: logsApi.clearAll,
  exportLogsCsv: logsApi.exportCsv,

  // Stats
  dashboardStats: statsApi.dashboard,
  recentRequests: statsApi.recentRequests,
  statsByProvider: statsApi.byProvider,
  hourlyStats: statsApi.hourly,

  // Model Mappings
  listModelMappings: modelMappingsApi.list,
  getModelMapping: modelMappingsApi.get,
  findModelMappingByName: modelMappingsApi.findByName,
  createModelMapping: modelMappingsApi.create,
  updateModelMapping: modelMappingsApi.update,
  deleteModelMapping: modelMappingsApi.remove,
  listAllModels: modelMappingsApi.listAllModels,

  // Gateway Keys
  getBuiltinGatewayKey: configApi.getBuiltinGatewayKey,
  resetBuiltinGatewayKey: configApi.resetBuiltinGatewayKey,

  // Config & Data
  exportAppConfig: configApi.exportConfig,
  importAppConfig: configApi.importConfig,
  backupDatabase: configApi.backupDatabase,
  restoreDatabase: configApi.restoreDatabase,

  // Profiles
  listProfiles: profilesApi.list,
  getProfile: profilesApi.get,
  createProfile: profilesApi.create,
  updateProfile: profilesApi.update,
  deleteProfile: profilesApi.remove,
  switchProfile: profilesApi.switch,
  getCommonSnippet: profilesApi.getCommonSnippet,
  setCommonSnippet: profilesApi.setCommonSnippet,
  importLiveConfig: profilesApi.importLiveConfig,
  listImportableProviders: profilesApi.listImportableProviders,
  getAgentLiveStatus: profilesApi.getAgentLiveStatus,
} as const;

// ---------------------------------------------------------------------------
// 领域 API 独立导出（新代码推荐按需导入）
// ---------------------------------------------------------------------------

export { gatewayApi } from "./gateway";
export { providersApi } from "./providers";
export { logsApi } from "./logs";
export { statsApi } from "./stats";
export { modelMappingsApi } from "./model-mappings";
export { profilesApi } from "./profiles";
export { configApi } from "./config";
