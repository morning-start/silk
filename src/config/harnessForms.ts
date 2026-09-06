// ---------------------------------------------------------------------------
// Harness 预设结构化表单契约。
// settings_config 仍保存各 CLI 的原生配置片段；表单只负责可编辑字段与
// 原生配置之间的无损编解码，不引入供应商目录或 OAuth 专用模型。
// ---------------------------------------------------------------------------

import type { Preset } from "../api";

export type FieldType =
  | "text"
  | "secret"
  | "number"
  | "select"
  | "model-roles"
  | "model-select"
  | "key-value"
  | "model-list"
  | "model-map"
  | "json";
export interface HarnessOption {
  label: string;
  value: string;
  disabled?: boolean;
}

export interface HarnessField {
  key: string;
  label: string;
  type: FieldType;
  placeholder?: string;
  hint?: string;
  roles?: string[];
  options?: HarnessOption[];
  min?: number;
}

export interface HarnessFormSpec {
  agentType: string;
  label: string;
  fields: HarnessField[];
  toSettings(form: Record<string, unknown>): Record<string, unknown>;
  fromSettings(config: Record<string, unknown>): Record<string, unknown>;
}

type JsonObject = Record<string, unknown>;

function object(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? { ...(value as JsonObject) }
    : {};
}

function string(value: unknown): string {
  return typeof value === "string" ? value : value == null ? "" : String(value);
}

function cleanKeyValue(value: unknown): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, raw] of Object.entries(object(value))) {
    const trimmed = key.trim();
    if (trimmed) result[trimmed] = string(raw);
  }
  return result;
}

function cleanModels(value: unknown): Array<Record<string, unknown>> {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => object(item))
    .filter((item) => string(item.id).trim())
    .map((item) => ({
      id: string(item.id).trim(),
      ...(string(item.name).trim() ? { name: string(item.name).trim() } : {}),
      ...(Number(item.context_length) > 0
        ? { context_length: Number(item.context_length) }
        : {}),
    }));
}

function cleanModelMap(value: unknown): Array<Record<string, unknown>> {
  if (Array.isArray(value)) {
    return value.map((item) => object(item)).filter((item) => string(item.id).trim()).map((item) => ({ ...item, id: string(item.id).trim() }));
  }
  if (!value || typeof value !== "object") return [];
  return Object.entries(value as JsonObject).map(([id, metadata]) => ({
    id,
    ...(metadata && typeof metadata === "object" && !Array.isArray(metadata) ? metadata as JsonObject : {}),
  }));
}

function modelMap(value: unknown): JsonObject {
  const result: JsonObject = {};
  for (const item of cleanModelMap(value)) {
    const id = string(item.id).trim();
    if (!id) continue;
    const metadata = { ...item };
    delete metadata.id;
    result[id] = metadata;
  }
  return result;
}

function withRaw(config: JsonObject, values: JsonObject): JsonObject {
  return { ...values, __raw: config };
}

function rawConfig(form: JsonObject): JsonObject {
  return object(form.__raw);
}

function setNested(root: JsonObject, key: string, value: unknown) {
  if (value === undefined || value === "") delete root[key];
  else root[key] = value;
}

const codexReasoningOptions = [
  "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
].map((value) => ({ label: value, value }));

const hermesApiModeOptions = [
  { label: "Chat Completions", value: "chat_completions" },
  { label: "Anthropic Messages", value: "anthropic_messages" },
  { label: "Codex Responses", value: "codex_responses" },
  // bedrock 不在 silk 网关 4 协议转换集内：仅直连第三方端点可用（对齐 AgentType 能力表）
  { label: "Bedrock Converse（仅直连）", value: "bedrock_converse" },
];

const openCodeSdkOptions = [
  { label: "OpenAI Responses", value: "@ai-sdk/openai" },
  { label: "OpenAI Compatible", value: "@ai-sdk/openai-compatible" },
  { label: "Anthropic", value: "@ai-sdk/anthropic" },
  // bedrock 不在 silk 网关 4 协议转换集内：仅直连第三方端点可用（对齐 AgentType 能力表）
  { label: "Amazon Bedrock（仅直连）", value: "@ai-sdk/amazon-bedrock" },
  { label: "Google Gemini", value: "@ai-sdk/google" },
];


const claudeSpec: HarnessFormSpec = {
  agentType: "claude_code",
  label: "Claude Code",
  fields: [
    { key: "ANTHROPIC_BASE_URL", label: "API 端点", type: "text", placeholder: "http://127.0.0.1:1877（自动补 /v1/messages，勿带 /v1）" },
    { key: "ANTHROPIC_AUTH_TOKEN", label: "API Key", type: "secret", placeholder: "sk-..." },
    { key: "ANTHROPIC_MODEL", label: "主模型", type: "model-select", placeholder: "选择或输入模型 id" },
    { key: "CLAUDE_CODE_SUBAGENT_MODEL", label: "子代理模型", type: "model-select", placeholder: "可选" },
    { key: "roles", label: "角色模型", type: "model-roles", roles: ["sonnet", "opus", "fable", "haiku"] },
    { key: "env_extra", label: "额外环境变量", type: "key-value", placeholder: "{\n  \"ANTHROPIC_CUSTOM_HEADERS\": \"...\"\n}" },
  ],
  toSettings(form) {
    const out = rawConfig(form);
    const env = object(out.env);
    const controlled = new Set(["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL"]);
    for (const key of controlled) delete env[key];
    Object.assign(env, cleanKeyValue(form.env_extra));
    // Claude Code 会自动追加 /v1/messages：保存时自动去除端点尾部 /v1（与后端 normalize_api_base_url 一致）
    setNested(env, "ANTHROPIC_BASE_URL", string(form.ANTHROPIC_BASE_URL).trim().replace(/\/+$/, "").replace(/\/v1$/, ""));
    for (const key of ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL"]) setNested(env, key, string(form[key]).trim());
    const roles = object(form.roles);
    for (const role of ["sonnet", "opus", "fable", "haiku"]) setNested(env, `ANTHROPIC_DEFAULT_${role.toUpperCase()}_MODEL`, string(roles[role]).trim());
    out.env = env;
    return out;
  },
  fromSettings(config) {
    const env = object(config.env);
    const roles: JsonObject = {};
    for (const role of ["sonnet", "opus", "fable", "haiku"]) roles[role] = string(env[`ANTHROPIC_DEFAULT_${role.toUpperCase()}_MODEL`]);
    const extra = { ...env };
    for (const key of ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL"]) delete extra[key];
    return withRaw(config, { ANTHROPIC_BASE_URL: string(env.ANTHROPIC_BASE_URL), ANTHROPIC_AUTH_TOKEN: string(env.ANTHROPIC_AUTH_TOKEN), ANTHROPIC_MODEL: string(env.ANTHROPIC_MODEL), CLAUDE_CODE_SUBAGENT_MODEL: string(env.CLAUDE_CODE_SUBAGENT_MODEL), roles, env_extra: extra });
  },
};

const codexSpec: HarnessFormSpec = {
  agentType: "codex",
  label: "Codex",
  fields: [
    { key: "model_provider", label: "Model Provider", type: "text", placeholder: "custom（ollama/lmstudio 禁止覆盖）" },
    { key: "model", label: "模型", type: "model-select", placeholder: "请先获取模型列表" },
    // 无 Wire API 字段：Codex 0.149+ 仅支持 Responses wire（chat wire 已移除），
    // 其余协议由 silk 网关 prism 转换（writer 侧恒写 wire_api="responses"）
    { key: "base_url", label: "API 端点", type: "text", placeholder: "http://127.0.0.1:1877/v1" },
    { key: "api_key", label: "API Key", type: "secret", placeholder: "sk-silk-..." },
    { key: "model_reasoning_effort", label: "推理强度", type: "select", options: codexReasoningOptions },
    { key: "model_context_window", label: "上下文窗口", type: "number", min: 1, placeholder: "可选" },
    { key: "model_auto_compact_token_limit", label: "自动压缩阈值", type: "number", min: 1, placeholder: "可选" },
    { key: "http_headers", label: "请求头", type: "key-value", placeholder: "{\n  \"X-Provider\": \"silk\"\n}" },
    { key: "advanced", label: "其他原生配置", type: "json", placeholder: "JSON 对象（可选）" },
  ],
  toSettings(form) {
    const out = { ...rawConfig(form), ...object(form.advanced) };
    for (const key of ["model_provider", "model", "base_url", "api_key", "model_reasoning_effort"]) setNested(out, key, string(form[key]).trim());
    for (const key of ["model_context_window", "model_auto_compact_token_limit"]) {
      const n = Number(form[key]);
      if (Number.isInteger(n) && n > 0) out[key] = n;
      else delete out[key];
    }
    const headers = cleanKeyValue(form.http_headers);
    if (Object.keys(headers).length) out.http_headers = headers;
    else delete out.http_headers;
    return out;
  },
  fromSettings(config) {
    const advanced = { ...config };
    for (const key of ["model_provider", "model", "wire_api", "base_url", "api_key", "model_reasoning_effort", "model_context_window", "model_auto_compact_token_limit", "http_headers"]) delete advanced[key];
    return withRaw(config, { model_provider: string(config.model_provider), model: string(config.model), base_url: string(config.base_url), api_key: string(config.api_key), model_reasoning_effort: string(config.model_reasoning_effort), model_context_window: config.model_context_window ?? "", model_auto_compact_token_limit: config.model_auto_compact_token_limit ?? "", http_headers: cleanKeyValue(config.http_headers), advanced });
  },
};

const opencodeSpec: HarnessFormSpec = {
  agentType: "opencode",
  label: "OpenCode",
  fields: [
    { key: "id", label: "Provider ID", type: "text", placeholder: "silk" },
    { key: "npm", label: "SDK", type: "select", options: openCodeSdkOptions },
    { key: "baseURL", label: "API 端点", type: "text", placeholder: "https://api.example.com/v1" },
    { key: "apiKey", label: "API Key", type: "secret", placeholder: "sk-..." },
    { key: "headers", label: "请求头", type: "key-value", placeholder: "{\n  \"X-Provider\": \"silk\"\n}" },
    { key: "extra_options", label: "额外 SDK 选项", type: "json", placeholder: "JSON 对象（可选）" },
    { key: "models", label: "模型定义", type: "model-map", placeholder: "请先获取模型列表" },
  ],
  toSettings(form) {
    const out = rawConfig(form);
    setNested(out, "id", string(form.id).trim());
    out.npm = string(form.npm);
    const options = { ...object(out.options), ...object(form.extra_options) };
    setNested(options, "baseURL", string(form.baseURL).trim());
    setNested(options, "apiKey", string(form.apiKey).trim());
    const headers = cleanKeyValue(form.headers);
    if (Object.keys(headers).length) options.headers = headers;
    else delete options.headers;
    out.options = options;
    const models = modelMap(form.models);
    if (Object.keys(models).length) out.models = models;
    else delete out.models;
    return out;
  },
  fromSettings(config) {
    const options = object(config.options);
    const extra = { ...options };
    for (const key of ["baseURL", "apiKey", "headers"]) delete extra[key];
    return withRaw(config, { id: string(config.id), npm: string(config.npm), baseURL: string(options.baseURL), apiKey: string(options.apiKey), headers: cleanKeyValue(options.headers), extra_options: extra, models: cleanModelMap(config.models) });
  },
};
const hermesSpec: HarnessFormSpec = {
  agentType: "hermes",
  label: "Hermes",
  fields: [
    { key: "name", label: "Provider 名称", type: "text", placeholder: "provider-id" },
    { key: "base_url", label: "API 端点", type: "text", placeholder: "https://api.example.com/v1" },
    { key: "api_key", label: "API Key", type: "secret", placeholder: "sk-..." },
    { key: "api_mode", label: "API 模式", type: "select", options: hermesApiModeOptions },
    { key: "rate_limit_delay", label: "限流间隔（秒）", type: "number", min: 0, placeholder: "可选" },
    { key: "models", label: "模型列表", type: "model-list", placeholder: "请先获取模型列表" },
    { key: "advanced", label: "其他原生配置", type: "json", placeholder: "JSON 对象（可选）" },
  ],
  toSettings(form) {
    const out = { ...rawConfig(form), ...object(form.advanced) };
    for (const key of ["name", "base_url", "api_key", "api_mode"]) setNested(out, key, string(form[key]).trim());
    const delay = Number(form.rate_limit_delay);
    if (Number.isFinite(delay) && delay >= 0 && String(form.rate_limit_delay).trim()) out.rate_limit_delay = delay;
    else delete out.rate_limit_delay;
    out.models = cleanModels(form.models);
    return out;
  },
  fromSettings(config) {
    const advanced = { ...config };
    for (const key of ["name", "base_url", "api_key", "api_mode", "rate_limit_delay", "models"]) delete advanced[key];
    return withRaw(config, { name: string(config.name), base_url: string(config.base_url), api_key: string(config.api_key), api_mode: string(config.api_mode) || "chat_completions", rate_limit_delay: config.rate_limit_delay ?? "", models: cleanModels(config.models), advanced });
  },
};

const geminiSpec: HarnessFormSpec = {
  agentType: "gemini_cli",
  label: "Gemini CLI",
  fields: [
    { key: "GOOGLE_GEMINI_BASE_URL", label: "API 端点", type: "text", placeholder: "https://generativelanguage.googleapis.com/v1beta" },
    { key: "GEMINI_API_KEY", label: "API Key", type: "secret", placeholder: "AIza..." },
    { key: "GEMINI_MODEL", label: "模型", type: "model-select", placeholder: "选择或输入模型 id" },
    { key: "env_extra", label: "额外环境变量", type: "key-value", placeholder: "{\n  \"GEMINI_USE_VERTEXAI\": \"true\"\n}" },
    { key: "advanced", label: "其他原生配置", type: "json", placeholder: "JSON 对象（可选）" },
  ],
  toSettings(form) {
    const out = { ...rawConfig(form), ...object(form.advanced) };
    const env = object(out.env);
    Object.assign(env, cleanKeyValue(form.env_extra));
    for (const key of ["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY", "GEMINI_MODEL"]) setNested(env, key, string(form[key]).trim());
    out.env = env;
    return out;
  },
  fromSettings(config) {
    const env = object(config.env);
    const extra = { ...env };
    for (const key of ["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY", "GEMINI_MODEL"]) delete extra[key];
    const advanced = { ...config };
    delete advanced.env;
    return withRaw(config, { GOOGLE_GEMINI_BASE_URL: string(env.GOOGLE_GEMINI_BASE_URL), GEMINI_API_KEY: string(env.GEMINI_API_KEY), GEMINI_MODEL: string(env.GEMINI_MODEL), env_extra: extra, advanced });
  },
};

export const harnessFormSpecs: Record<string, HarnessFormSpec> = {
  claude_code: claudeSpec,
  codex: codexSpec,
  opencode: opencodeSpec,
  hermes: hermesSpec,
  gemini_cli: geminiSpec,
};

/**
 * 官方直连卡（category=official）可填凭据字段：端点/模型锚定官方默认（不可改），
 * 仅凭据开放填写；留空 = 使用 CLI 官方登录/默认。与后端 sanitize_official_settings 白名单对齐。
 */
export const officialCredentialFields: Record<string, Array<{ key: string; label: string }>> = {
  claude_code: [{ key: "ANTHROPIC_AUTH_TOKEN", label: "Anthropic API Key" }],
  gemini_cli: [{ key: "GEMINI_API_KEY", label: "Google API Key" }],
};

export function formSpecFor(agentType: string): HarnessFormSpec | undefined {
  return harnessFormSpecs[agentType];
}

export function parseJsonObject(value: unknown, label: string): Record<string, unknown> {
  if (value && typeof value === "object" && !Array.isArray(value)) return value as Record<string, unknown>;
  throw new Error(`${label}必须是 JSON 对象`);
}

export type { Preset };
