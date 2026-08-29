import { invoke } from "@tauri-apps/api/core";
import type { FileOperationResponse, GatewayKey } from "./types";

export const configApi = {
  // Gateway Keys
  getBuiltinGatewayKey: (): Promise<GatewayKey> =>
    invoke<GatewayKey>("get_builtin_gateway_key"),

  resetBuiltinGatewayKey: (): Promise<GatewayKey> =>
    invoke<GatewayKey>("reset_builtin_gateway_key"),

  // Config & Data
  exportConfig: (data?: { file_path?: string }): Promise<FileOperationResponse> =>
    invoke<FileOperationResponse>("export_app_config", { payload: data ?? {} }),

  importConfig: (data: { file_path: string }): Promise<FileOperationResponse> =>
    invoke<FileOperationResponse>("import_app_config", { payload: data }),

  backupDatabase: (data?: { file_path?: string }): Promise<FileOperationResponse> =>
    invoke<FileOperationResponse>("backup_database", { payload: data ?? {} }),

  restoreDatabase: (data: { file_path: string }): Promise<FileOperationResponse> =>
    invoke<FileOperationResponse>("restore_database", { payload: data }),
};
