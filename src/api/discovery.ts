import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 发现类接口：预置渠道模板、本地已安装 AI 应用检测
// 对应后端 `commands::discovery`
// ---------------------------------------------------------------------------

/** 预置渠道模板中的一个模型 */
export interface PresetModel {
  id: string;
  name: string;
  description: string;
}

/** 预置渠道模板（内置 `data/preset_providers.json`，用于快速填充渠道表单） */
export interface PresetProvider {
  id: string;
  name: string;
  description: string;
  /** 与渠道表单的协议值一致：openai / messages / responses / gemini */
  protocols: string[];
  models: PresetModel[];
  api_base_url: string;
  /** 申请 API Key 的官方页面 */
  api_key_url: string;
  api_key_placeholder: string;
  color: string;
}

/** 本地已安装 AI 应用检测结果 */
export interface InstalledAiApp {
  name: string;
  description: string;
  installed: boolean;
  config_path: string | null;
  icon: string;
  color: string;
}

/** `omp --list-models` 探测结果：按 provider 分组的模型列表 */
export interface OmpModelGroup {
  provider: string;
  models: Array<{
    id: string;
    context_window?: number;
    max_tokens?: number;
    reasoning?: boolean;
    input_types?: string[];
  }>;
}

export const discoveryApi = {
  /** 获取全部预置渠道模板 */
  getPresetProviders: (): Promise<PresetProvider[]> =>
    invoke<PresetProvider[]>("get_preset_providers"),

  /** 按 id 获取单个预置渠道模板 */
  getPresetProviderById: (id: string): Promise<PresetProvider | null> =>
    invoke<PresetProvider | null>("get_preset_provider_by_id", { id }),

  /** 检测本机已安装的 AI 应用（扫描其配置文件是否存在） */
  detectInstalledAiApps: (): Promise<InstalledAiApp[]> =>
    invoke<InstalledAiApp[]>("detect_installed_ai_apps"),

  /** 探测本机 OMP 的模型列表（`omp --list-models`，3s 超时；未安装降级为空） */
  listOmpModels: (): Promise<OmpModelGroup[]> =>
    invoke<OmpModelGroup[]>("list_omp_models"),
};
