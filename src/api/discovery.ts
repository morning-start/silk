import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 发现类接口：渠道模板目录、本地已安装 AI 应用检测
// 对应后端 `commands::discovery`
// ---------------------------------------------------------------------------

/** 渠道模板中的一个预置模型项 */
export interface ChannelTemplateModel {
  id: string;
  name: string;
  description: string;
}

/**
 * 渠道模板（内置 `data/channel_templates.json`，用于快速填充「添加渠道」表单）。
 * 注意与 `Provider`（数据库中用户实际配置的渠道）区分：模板只是填写素材。
 */
export interface ChannelTemplate {
  id: string;
  name: string;
  description: string;
  /** 与渠道表单的协议值一致：openai / messages / responses / gemini */
  protocols: string[];
  models: ChannelTemplateModel[];
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

export const discoveryApi = {
  /** 获取全部渠道模板 */
  getChannelTemplates: (): Promise<ChannelTemplate[]> =>
    invoke<ChannelTemplate[]>("get_channel_templates"),

  /** 按 id 获取单个渠道模板 */
  getChannelTemplateById: (id: string): Promise<ChannelTemplate | null> =>
    invoke<ChannelTemplate | null>("get_channel_template_by_id", { id }),

  /** 检测本机已安装的 AI 应用（扫描其配置文件是否存在） */
  detectInstalledAiApps: (): Promise<InstalledAiApp[]> =>
    invoke<InstalledAiApp[]>("detect_installed_ai_apps"),
};
