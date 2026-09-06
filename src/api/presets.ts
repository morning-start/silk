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

  /** OpenCode 累加模式独立启停：active=true 加入 opencode.json，false 移出；不清除其他激活项 */
  setActive: (agentType: string, presetId: string, active: boolean): Promise<PresetSwitchResult> =>
    invoke<PresetSwitchResult>("set_preset_active", { agentType, presetId, active }),

  /** 官方直连行一键恢复默认（清空凭据，回到官方锚定形态） */
  resetOfficial: (presetId: string): Promise<Preset> =>
    invoke<Preset>("reset_official_preset", { presetId }),

  listAgentTypes: (): Promise<AgentTypeInfo[]> =>
    invoke<AgentTypeInfo[]>("list_agent_types"),

  getDefaults: (agentType: string): Promise<PresetDefaults> =>
    invoke<PresetDefaults>("get_preset_defaults", { agentType }),

  reorder: (agentType: string, orderedIds: string[]): Promise<void> =>
    invoke<void>("update_preset_order", { agentType, orderedIds }),
};
