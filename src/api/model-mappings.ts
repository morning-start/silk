import { invoke } from "@tauri-apps/api/core";
import type {
  ModelMapping,
  NewMappingChannel,
  ModelListingItem,
} from "./types";

export interface CreateModelMappingPayload {
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
}

export interface UpdateModelMappingPayload {
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
}

export const modelMappingsApi = {
  list: (): Promise<ModelMapping[]> => invoke<ModelMapping[]>("list_model_mappings"),

  get: (id: string): Promise<ModelMapping> =>
    invoke<ModelMapping>("get_model_mapping", { id }),

  findByName: (name: string): Promise<ModelMapping | null> =>
    invoke<ModelMapping | null>("find_model_mapping_by_name", {
      model_name: name,
    }),

  create: (data: CreateModelMappingPayload): Promise<ModelMapping> =>
    invoke<ModelMapping>("create_model_mapping", { payload: data }),

  update: (id: string, data: UpdateModelMappingPayload): Promise<ModelMapping> =>
    invoke<ModelMapping>("update_model_mapping", { id, payload: data }),

  remove: (id: string): Promise<boolean> =>
    invoke<boolean>("delete_model_mapping", { id }),

  listAllModels: (): Promise<ModelListingItem[]> =>
    invoke<ModelListingItem[]>("list_all_models"),
};
