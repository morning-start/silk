import { invoke } from "@tauri-apps/api/core";
import type { GatewaySettings, GatewayStatus } from "./types";

export interface GatewayStartResponse {
  success: boolean;
  address: string;
}

export interface GatewayStopResponse {
  success: boolean;
  message: string;
}

export const gatewayApi = {
  status: (): Promise<GatewayStatus> => invoke<GatewayStatus>("gateway_status"),

  start: (): Promise<GatewayStartResponse> =>
    invoke<GatewayStartResponse>("gateway_start"),

  stop: (): Promise<GatewayStopResponse> =>
    invoke<GatewayStopResponse>("gateway_stop"),

  restart: (): Promise<GatewayStartResponse> =>
    invoke<GatewayStartResponse>("gateway_restart"),

  getSettings: (): Promise<GatewaySettings> =>
    invoke<GatewaySettings>("get_gateway_settings"),

  updateSettings: (data: Partial<GatewaySettings>): Promise<GatewaySettings> =>
    invoke<GatewaySettings>("update_gateway_settings", { payload: data }),
};
