import { invoke } from "@tauri-apps/api/core";
import type {
  Provider,
  ProviderTestResponse,
  ProviderModelInfo,
} from "./types";

export const providersApi = {
  list: (): Promise<Provider[]> => invoke<Provider[]>("list_providers"),

  get: (id: string): Promise<Provider> => invoke<Provider>("get_provider", { id }),

  create: (data: Partial<Provider> & { api_key: string }): Promise<Provider> =>
    invoke<Provider>("create_provider", { payload: data }),

  update: (id: string, data: Partial<Provider>): Promise<Provider> =>
    invoke<Provider>("update_provider", { id, payload: data }),

  remove: (id: string): Promise<boolean> => invoke<boolean>("delete_provider", { id }),

  test: (id: string): Promise<ProviderTestResponse> =>
    invoke<ProviderTestResponse>("test_provider", { id }),

  fetchModels: (data: {
    api_base_url: string;
    api_key: string;
    proxy_url?: string;
    timeout_seconds?: number;
  }): Promise<ProviderModelInfo[]> =>
    invoke<ProviderModelInfo[]>("fetch_provider_models", { payload: data }),
};
