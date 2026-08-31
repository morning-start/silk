import { invoke } from "@tauri-apps/api/core";
import type {
  Preset,
  AgentTypeInfo,
  NewPresetPayload,
  UpdatePresetPayload,
  PresetSwitchResult,
  PresetDefaults,
} from "./types";

export const presetsApi = {
  list: (agentType: string): Promise<Preset[]> =>
    invoke<Preset[]>("list_presets", { agentType }),

  get: (presetId: string): Promise<Preset> =>
    invoke<Preset>("get_preset", { presetId }),

  create: (payload: NewPresetPayload): Promise<Preset> =>
    invoke<Preset>("create_preset", { payload }),

  update: (presetId: string, payload: UpdatePresetPayload): Promise<Preset> =>
    invoke<Preset>("update_preset", { presetId, payload }),

  remove: (presetId: string): Promise<boolean> =>
    invoke<boolean>("delete_preset", { presetId }),

  switch: (agentType: string, presetId: string): Promise<PresetSwitchResult> =>
    invoke<PresetSwitchResult>("switch_preset", { agentType, presetId }),

  listAgentTypes: (): Promise<AgentTypeInfo[]> =>
    invoke<AgentTypeInfo[]>("list_agent_types"),

  getDefaults: (agentType: string): Promise<PresetDefaults> =>
    invoke<PresetDefaults>("get_preset_defaults", { agentType }),
};
