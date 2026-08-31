// ---------------------------------------------------------------------------
// 5 个 harness 的结构化表单字段配置（对齐 cc-switch 前端 ProviderForm）
//
// 每个 harness 定义一组字段，用于预设的新建/编辑弹窗（替代手填 JSON）：
// - fields: 表单字段描述（label + key + 类型）
// - toSettings(form): 表单值 → settings_config（后端 writer 注入网关信息后写 live）
// - fromSettings(config): settings_config → 表单值（编辑回显）
// ---------------------------------------------------------------------------

import type { Preset } from "../api";

export type FieldType = "text" | "secret" | "model-roles" | "model-select";

export interface HarnessField {
  key: string;
  label: string;
  type: FieldType;
  placeholder?: string;
  /** model-roles 类型：角色列表（对齐 cc-switch Claude 4 角色） */
  roles?: string[];
}

export interface HarnessFormSpec {
  agentType: string;
  label: string;
  fields: HarnessField[];
  /** 表单值 → settings_config */
  toSettings(form: Record<string, unknown>): Record<string, unknown>;
  /** settings_config → 表单值（编辑回显） */
  fromSettings(config: Record<string, unknown>): Record<string, unknown>;
}

// Claude Code：env 子对象（4 角色模型 + 端点 + key）
const claudeSpec: HarnessFormSpec = {
  agentType: "claude_code",
  label: "Claude Code",
  fields: [
    { key: "ANTHROPIC_BASE_URL", label: "API 端点", type: "text", placeholder: "https://api.example.com/anthropic" },
    { key: "ANTHROPIC_AUTH_TOKEN", label: "API Key", type: "secret", placeholder: "sk-..." },
    { key: "roles", label: "角色模型", type: "model-roles", roles: ["sonnet", "opus", "fable", "haiku"] },
  ],
  toSettings(form) {
    const env: Record<string, string> = {};
    if (form.ANTHROPIC_BASE_URL) env.ANTHROPIC_BASE_URL = String(form.ANTHROPIC_BASE_URL);
    if (form.ANTHROPIC_AUTH_TOKEN) env.ANTHROPIC_AUTH_TOKEN = String(form.ANTHROPIC_AUTH_TOKEN);
    // 角色模型 → ANTHROPIC_DEFAULT_*_MODEL（对齐 cc-switch 预设结构）
    const roles = (form.roles as Record<string, string>) || {};
    for (const role of ["sonnet", "opus", "fable", "haiku"]) {
      const v = roles[role];
      if (v) env[`ANTHROPIC_DEFAULT_${role.toUpperCase()}_MODEL`] = String(v);
    }
    return Object.keys(env).length > 0 ? { env } : {};
  },
  fromSettings(config) {
    const env = (config.env as Record<string, string>) || {};
    const roles: Record<string, string> = {};
    for (const role of ["sonnet", "opus", "fable", "haiku"]) {
      roles[role] = env[`ANTHROPIC_DEFAULT_${role.toUpperCase()}_MODEL`] || "";
    }
    return {
      ANTHROPIC_BASE_URL: env.ANTHROPIC_BASE_URL || "",
      ANTHROPIC_AUTH_TOKEN: env.ANTHROPIC_AUTH_TOKEN || "",
      roles,
    };
  },
};

// Codex：顶层 model_provider / model / wire_api + 端点/Key（可选，默认 silk）
const codexSpec: HarnessFormSpec = {
  agentType: "codex",
  label: "Codex",
  fields: [
    { key: "model_provider", label: "Model Provider", type: "text", placeholder: "custom（内置 ollama/lmstudio 禁止覆盖）" },
    { key: "model", label: "模型", type: "model-select", placeholder: "选择或输入模型 id" },
    { key: "wire_api", label: "Wire API", type: "text", placeholder: "responses" },
    { key: "base_url", label: "API 端点", type: "text", placeholder: "http://127.0.0.1:1877/v1" },
    { key: "api_key", label: "API Key", type: "secret", placeholder: "sk-silk-..." },
  ],
  toSettings(form) {
    const out: Record<string, unknown> = {};
    if (form.model_provider) out.model_provider = String(form.model_provider);
    if (form.model) out.model = String(form.model);
    if (form.wire_api) out.wire_api = String(form.wire_api);
    if (form.base_url) out.base_url = String(form.base_url);
    if (form.api_key) out.api_key = String(form.api_key);
    return out;
  },
  fromSettings(config) {
    return {
      model_provider: (config.model_provider as string) || "",
      model: (config.model as string) || "",
      wire_api: (config.wire_api as string) || "responses",
      base_url: (config.base_url as string) || "",
      api_key: (config.api_key as string) || "",
    };
  },
};

// OpenCode：provider 条目（npm + options.baseURL/apiKey + models）
const opencodeSpec: HarnessFormSpec = {
  agentType: "opencode",
  label: "OpenCode",
  fields: [
    { key: "npm", label: "SDK", type: "text", placeholder: "@ai-sdk/openai-compatible" },
    { key: "baseURL", label: "API 端点", type: "text", placeholder: "https://api.example.com/v1" },
    { key: "apiKey", label: "API Key", type: "secret", placeholder: "sk-..." },
  ],
  toSettings(form) {
    const out: Record<string, unknown> = {};
    if (form.npm) out.npm = String(form.npm);
    if (form.baseURL || form.apiKey) {
      out.options = {};
      if (form.baseURL) (out.options as Record<string, unknown>).baseURL = String(form.baseURL);
      if (form.apiKey) (out.options as Record<string, unknown>).apiKey = String(form.apiKey);
    }
    return out;
  },
  fromSettings(config) {
    const options = (config.options as Record<string, unknown>) || {};
    return {
      npm: (config.npm as string) || "",
      baseURL: (options.baseURL as string) || "",
      apiKey: (options.apiKey as string) || "",
    };
  },
};

// Hermes：custom_providers 条目（base_url + api_key + models）
const hermesSpec: HarnessFormSpec = {
  agentType: "hermes",
  label: "Hermes",
  fields: [
    { key: "base_url", label: "API 端点", type: "text", placeholder: "https://api.example.com/v1" },
    { key: "api_key", label: "API Key", type: "secret", placeholder: "sk-..." },
  ],
  toSettings(form) {
    const out: Record<string, unknown> = {};
    if (form.base_url) out.base_url = String(form.base_url);
    if (form.api_key) out.api_key = String(form.api_key);
    return out;
  },
  fromSettings(config) {
    return {
      base_url: (config.base_url as string) || "",
      api_key: (config.api_key as string) || "",
    };
  },
};

// Gemini CLI：env 子对象（GOOGLE_GEMINI_BASE_URL + GEMINI_API_KEY + GEMINI_MODEL）
const geminiSpec: HarnessFormSpec = {
  agentType: "gemini_cli",
  label: "Gemini CLI",
  fields: [
    { key: "GOOGLE_GEMINI_BASE_URL", label: "API 端点", type: "text", placeholder: "https://generativelanguage.googleapis.com/v1beta" },
    { key: "GEMINI_API_KEY", label: "API Key", type: "secret", placeholder: "AIza..." },
    { key: "GEMINI_MODEL", label: "模型", type: "model-select", placeholder: "选择或输入模型 id" },
  ],
  toSettings(form) {
    const env: Record<string, string> = {};
    if (form.GOOGLE_GEMINI_BASE_URL) env.GOOGLE_GEMINI_BASE_URL = String(form.GOOGLE_GEMINI_BASE_URL);
    if (form.GEMINI_API_KEY) env.GEMINI_API_KEY = String(form.GEMINI_API_KEY);
    if (form.GEMINI_MODEL) env.GEMINI_MODEL = String(form.GEMINI_MODEL);
    return Object.keys(env).length > 0 ? { env } : {};
  },
  fromSettings(config) {
    const env = (config.env as Record<string, string>) || {};
    return {
      GOOGLE_GEMINI_BASE_URL: env.GOOGLE_GEMINI_BASE_URL || "",
      GEMINI_API_KEY: env.GEMINI_API_KEY || "",
      GEMINI_MODEL: env.GEMINI_MODEL || "",
    };
  },
};

export const harnessFormSpecs: Record<string, HarnessFormSpec> = {
  claude_code: claudeSpec,
  codex: codexSpec,
  opencode: opencodeSpec,
  hermes: hermesSpec,
  gemini_cli: geminiSpec,
};

export function formSpecFor(agentType: string): HarnessFormSpec | undefined {
  return harnessFormSpecs[agentType];
}

export type { Preset };
