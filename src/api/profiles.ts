import { invoke } from "@tauri-apps/api/core";
import type {
  Profile,
  AgentType,
  CreateProfilePayload,
  UpdateProfilePayload,
  SwitchResult,
} from "./types";

export const profilesApi = {
  list: (agentType: AgentType): Promise<Profile[]> =>
    invoke<Profile[]>("list_profiles", { agentType }),

  get: (profileId: string): Promise<Profile> =>
    invoke<Profile>("get_profile", { profileId }),

  create: (payload: CreateProfilePayload): Promise<Profile> =>
    invoke<Profile>("create_profile", { payload }),

  update: (profileId: string, payload: UpdateProfilePayload): Promise<Profile> =>
    invoke<Profile>("update_profile", { profileId, payload }),

  remove: (profileId: string): Promise<boolean> =>
    invoke<boolean>("delete_profile", { profileId }),

  switch: (agentType: AgentType, profileId: string): Promise<SwitchResult> =>
    invoke<SwitchResult>("switch_profile", { agentType, profileId }),

  getCommonSnippet: (agentType: AgentType): Promise<string | null> =>
    invoke<string | null>("get_common_snippet", { agentType }),

  setCommonSnippet: (agentType: AgentType, content: string): Promise<void> =>
    invoke<void>("set_common_snippet", { agentType, content }),
};
