<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NButton, NInput, NInputNumber, NModal, NSelect, useMessage } from "naive-ui";
import { presetsApi } from "../api/presets";
import { providersApi } from "../api/providers";
import type { AgentTypeInfo, Preset, Provider, ProviderModelInfo } from "../api";
import { formSpecFor, officialCredentialFields, type HarnessField, type HarnessFormSpec } from "../config/harnessForms";
import ModalAdvanced from "../components/ModalAdvanced.vue";
import AppPageShell from "../components/AppPageShell.vue";
import { useConfirm } from "../utils/confirm";

const message = useMessage();
const { confirm } = useConfirm();
const agentTypes = ref<AgentTypeInfo[]>([]);
const activeTab = ref("claude_code");
const presets = ref<Preset[]>([]);
const loading = ref(false);
const showModal = ref(false);
const editingId = ref<string | null>(null);
const formName = ref("");
const formValues = ref<Record<string, unknown>>({});
const fetchedModels = ref<ProviderModelInfo[]>([]);
const modelsFetched = ref(false);
const fetchingModels = ref(false);
const draggingId = ref<string | null>(null);
// 官方直连行编辑（仅凭据可填）+ 恢复默认
const officialModal = ref(false);
const officialEditing = ref<Preset | null>(null);
const officialName = ref("");
const officialCredential = ref<Record<string, string>>({});
// 渠道快速填充（新建/编辑弹窗：选已有渠道 → 自动填端点/Key/协议选择器，直连该渠道免手填）
const channels = ref<Provider[]>([]);
const channelsLoading = ref(false);
const channelFillId = ref<string | null>(null);
const channelFillHint = ref<string | null>(null);

/** 各 harness 表单的端点/Key 字段 + 原生协议（对齐 harnessForms.ts 字段契约与 AgentType 能力表） */
const channelFillTargets: Record<string, { endpointKey: string; apiKeyField: string; nativeProtocols: string[] }> = {
  claude_code: { endpointKey: "ANTHROPIC_BASE_URL", apiKeyField: "ANTHROPIC_AUTH_TOKEN", nativeProtocols: ["messages"] },
  codex: { endpointKey: "base_url", apiKeyField: "api_key", nativeProtocols: ["responses"] },
  opencode: { endpointKey: "baseURL", apiKeyField: "apiKey", nativeProtocols: ["openai", "responses", "messages", "gemini", "bedrock"] },
  hermes: { endpointKey: "base_url", apiKeyField: "api_key", nativeProtocols: ["openai", "responses", "messages", "bedrock"] },
  gemini_cli: { endpointKey: "GOOGLE_GEMINI_BASE_URL", apiKeyField: "GEMINI_API_KEY", nativeProtocols: ["gemini"] },
};
/** 多协议 harness：渠道协议 → 表单协议选择器取值（直连该渠道时的原生协议） */
const OPENCODE_SDK_BY_PROTOCOL: Record<string, string> = {
  openai: "@ai-sdk/openai-compatible",
  responses: "@ai-sdk/openai",
  messages: "@ai-sdk/anthropic",
  gemini: "@ai-sdk/google",
  bedrock: "@ai-sdk/amazon-bedrock",
};
const HERMES_MODE_BY_PROTOCOL: Record<string, string> = {
  openai: "chat_completions",
  responses: "codex_responses",
  messages: "anthropic_messages",
  bedrock: "bedrock_converse",
};
const channelOptions = computed(() =>
  channels.value.map((channel) => ({
    label: channel.protocols.length ? `${channel.name}（${channel.protocols.join("/")}）` : channel.name,
    value: channel.id,
  })),
);

/** 页头说明：对齐原 page-header 的 subtitle 文案 */
const pageDesc = computed(() => {
  if (activeTab.value === "opencode" && openCodeActiveCount.value > 0) return `已激活 ${openCodeActiveCount.value} 个配置`;
  if (currentPreset.value) return `当前激活：${currentPreset.value.name}`;
  return "未激活任何预设（live 配置保持原状）";
});

async function loadChannels() {
  if (channelsLoading.value) return;
  channelsLoading.value = true;
  try {
    channels.value = await providersApi.list();
  } catch (error: any) {
    message.error(error?.message || "加载渠道列表失败");
  } finally {
    channelsLoading.value = false;
  }
}

function applyChannelFill(channelId: string | null) {
  channelFillId.value = channelId;
  if (!channelId) {
    channelFillHint.value = null;
    return;
  }
  const channel = channels.value.find((item) => item.id === channelId);
  if (!channel) return;
  const target = channelFillTargets[activeTab.value];
  channelFillHint.value = null;
  if (!target) {
    message.warning("该 Agent 类型不支持渠道快速填充");
    return;
  }
  // 端点：claude_code 自动去除尾部 /v1（与表单 toSettings 行为一致），其余按原样填入
  let endpoint = (channel.api_base_url || "").trim();
  if (activeTab.value === "claude_code") endpoint = endpoint.replace(/\/+$/, "").replace(/\/v1$/i, "");
  // Key：优先取启用中的第一个，其次取第一个
  const apiKey = channel.keys.find((key) => key.enabled)?.value ?? channel.keys[0]?.value ?? "";
  // 自动将渠道名填入预设名称
  formName.value = channel.name;
  formValues.value[target.endpointKey] = endpoint;
  formValues.value[target.apiKeyField] = apiKey;
  // 多协议 harness（opencode/hermes）：按渠道协议联动协议选择器
  if (activeTab.value === "opencode") {
    const sdk = channel.protocols.map((protocol) => OPENCODE_SDK_BY_PROTOCOL[protocol]).find(Boolean);
    if (sdk) formValues.value.npm = sdk;
  }
  if (activeTab.value === "hermes") {
    const mode = channel.protocols.map((protocol) => HERMES_MODE_BY_PROTOCOL[protocol]).find(Boolean);
    if (mode) formValues.value.api_mode = mode;
  }
  // 协议兼容提示：单协议 harness 若渠道协议不含原生协议，直连会失败
  const compatible = channel.protocols.length === 0 || channel.protocols.some((protocol) => target.nativeProtocols.includes(protocol));
  if (!compatible) {
    channelFillHint.value = `该渠道协议为 ${channel.protocols.join("/")}，与 ${tabLabel(activeTab.value)} 原生协议（${target.nativeProtocols.join("/")}）不匹配，直连可能失败；经 silk 网关转换则无此问题`;
  }
  message.success(`已从渠道「${channel.name}」填充名称、端点与 Key，可点击「获取模型」加载模型列表`);
  channelFillId.value = null; // 每次选择后复位，便于再次选择
}

const PROTOCOL_LABELS: Record<string, string> = {
  openai: "OpenAI Chat",
  responses: "OpenAI Responses",
  messages: "Anthropic Messages",
  gemini: "Gemini",
  bedrock: "Bedrock",
};
function protocolLabel(protocol: { name: string; convert: boolean }) {
  const label = PROTOCOL_LABELS[protocol.name] ?? protocol.name;
  return protocol.convert ? label : `${label}（仅直连）`;
}
/** 单协议 harness 的「原生协议」提示文案：协议由 CLI 固定，silk 自动转换，
 *  表单无需协议字段（多协议 harness 已在表单保留 npm/api_mode 选择器） */
function nativeProtocolHint(): string | null {
  const agent = agentTypes.value.find((item) => item.id === activeTab.value);
  if (!agent || agent.protocols.length !== 1) return null;
  return `原生协议：${protocolLabel(agent.protocols[0])}（协议固定，silk 自动转换，无需选择上游格式）`;
}
const currentPreset = computed(() => presets.value.find((preset) => preset.is_active) || null);
/** 官方直连行（category=official，锚定信息只读）：恒置顶、不可拖拽/作为拖放目标、无编辑/删除 */
function isOfficialPreset(preset: Preset) {
  return preset.category === "official";
}
/** 官方直连行可填凭据字段（端点/模型锚定官方，仅凭据开放；与后端清洗白名单对齐） */
function officialFieldsOf(agentType: string) {
  return officialCredentialFields[agentType] || [];
}
/** 官方卡说明文案：有凭据字段（claude/gemini）提示可填官方 API Key；无（codex OAuth）提示账号登录 */
function officialDesc(agentType: string) {
  return officialFieldsOf(agentType).length > 0
    ? "端点与模型锚定官方直连；可填写官方 API Key，留空则使用官方登录/默认"
    : "官方账号登录直连（OAuth），端点与模型锚定官方，不可修改";
}
/** 官方编辑弹窗提示文案 */
function officialModalTip(agentType: string) {
  return officialFieldsOf(agentType).length > 0
    ? "端点与模型已锚定官方默认，不可修改。填写官方 API Key 后，该 CLI 将用你的凭据直连官方；留空则使用官方登录/默认配置。"
    : "该应用官方模式使用官方账号登录（OAuth），无需填写 API Key；端点与模型已锚定官方，不可修改。";
}
function openOfficialEdit(preset: Preset) {
  officialEditing.value = preset;
  officialName.value = preset.name;
  const env = (preset.settings_config?.env ?? {}) as Record<string, unknown>;
  const values: Record<string, string> = {};
  for (const field of officialFieldsOf(preset.agent_type)) {
    values[field.key] = typeof env[field.key] === "string" ? (env[field.key] as string) : "";
  }
  officialCredential.value = values;
  officialModal.value = true;
}
async function saveOfficial() {
  const preset = officialEditing.value;
  if (!preset) return;
  if (!officialName.value.trim()) {
    message.warning("请输入预设名称");
    return;
  }
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(officialCredential.value)) {
    if (value && value.trim()) env[key] = value.trim();
  }
  try {
    await presetsApi.update(preset.id, { name: officialName.value.trim(), settings_config: { env } });
    message.success("已更新官方配置");
    officialModal.value = false;
    await loadPresets();
  } catch (error: any) {
    message.error(error?.message || "操作失败");
  }
}
function resetOfficial(preset: Preset) {
  confirm({
    title: "恢复官方默认",
    description: "端点与模型将重置为官方默认值。",
    targetLabel: "预设",
    target: preset.name,
    impacts: [
      "已填写的 API Key 凭据会被清空",
      "重置后需要重新填写凭据才能使用",
    ],
    positiveText: "恢复默认",
    destructive: true,
    onConfirm: async () => {
      await presetsApi.resetOfficial(preset.id);
      message.success("已恢复官方默认");
      await loadPresets();
    },
    onError: (error: any) => message.error(error?.message || "操作失败"),
  });
}
const openCodeActiveCount = computed(() => {
  if (activeTab.value !== 'opencode') return 0;
  return presets.value.filter((preset) => preset.is_active).length;
});
const spec = computed<HarnessFormSpec | undefined>(() => formSpecFor(activeTab.value));

/** 高级字段：原生环境变量 / JSON 片段，低频且容易写错，折叠起来 */
const ADVANCED_FIELD_TYPES = new Set(["key-value", "json"]);
function isAdvancedField(field: HarnessField): boolean {
  return ADVANCED_FIELD_TYPES.has(field.type);
}

/** 需要占整行的字段：模型角色 / 模型列表这类多行编辑器放半栏会挤成一团 */
function isWideField(field: HarnessField): boolean {
  return ["model-roles", "model-list", "model-map"].includes(field.type);
}

const basicFields = computed(() => (spec.value?.fields || []).filter((field) => !isAdvancedField(field)));
const advancedFields = computed(() => (spec.value?.fields || []).filter(isAdvancedField));

/** 高级区已填写的项数，>0 时折叠标题显示「已自定义 N 项」 */
const advancedFilledCount = computed(
  () => advancedFields.value.filter((field) => fieldValue(field).trim().length > 0).length,
);
const currentModelIds = computed(() => {
  const ids = new Set<string>();
  for (const field of spec.value?.fields || []) {
    const value = formValues.value[field.key];
    if (field.type === "model-select" && typeof value === "string" && value.trim()) ids.add(value.trim());
    if (field.type === "model-roles" && value && typeof value === "object" && !Array.isArray(value)) {
      for (const role of Object.values(value as Record<string, unknown>)) if (typeof role === "string" && role.trim()) ids.add(role.trim());
    }
    if ((field.type === "model-list" || field.type === "model-map") && Array.isArray(value)) {
      for (const item of value) if (item && typeof item === "object" && !Array.isArray(item)) {
        const id = (item as Record<string, unknown>).id;
        if (typeof id === "string" && id.trim()) ids.add(id.trim());
      }
    }
  }
  return ids;
});
const modelOptions = computed<Array<{ label: string; value: string; disabled?: boolean }>>(() => {
  const fetchedIds = new Set(fetchedModels.value.map((model) => model.id));
  const options: Array<{ label: string; value: string; disabled?: boolean }> = fetchedModels.value.map((model) => ({
    label: model.owned_by ? `${model.id} · ${model.owned_by}` : model.id,
    value: model.id,
  }));
  for (const id of currentModelIds.value) {
    if (!fetchedIds.has(id)) options.push({ label: `${id}（当前配置，未在本次列表中）`, value: id, disabled: true });
  }
  return options;
});

function selectOptions(field: HarnessField) {
  const options = [...(field.options || [])];
  const current = String(formValues.value[field.key] || "").trim();
  if (current && !options.some((option) => option.value === current)) {
    options.push({ label: `${current}（当前配置，不在推荐选项中）`, value: current, disabled: true });
  }
  return options;
}
const modelSelectorDisabled = computed(() => !modelsFetched.value || fetchedModels.value.length === 0);
const modelEndpointKey = computed(() => ({
  claude_code: "ANTHROPIC_BASE_URL",
  codex: "base_url",
  opencode: "baseURL",
  hermes: "base_url",
  gemini_cli: "GOOGLE_GEMINI_BASE_URL",
}[activeTab.value] || ""));

async function loadAgentTypes() {
  try {
    agentTypes.value = await presetsApi.listAgentTypes();
    if (agentTypes.value.length > 0) activeTab.value = agentTypes.value[0].id;
  } catch (error: any) {
    message.error(error?.message || "加载 Agent 类型失败");
  }
}

async function loadPresets() {
  loading.value = true;
  try {
    presets.value = await presetsApi.list(activeTab.value);
  } catch (error: any) {
    message.error(error?.message || "加载预设失败");
  } finally {
    loading.value = false;
  }
}

function tabLabel(id: string) {
  return agentTypes.value.find((agent) => agent.id === id)?.name || id;
}

function fieldValue(field: HarnessField): string {
  const value = formValues.value[field.key];
  if (["key-value", "json", "model-list"].includes(field.type)) {
    return value && typeof value === "object" ? JSON.stringify(value, null, 2) : "";
  }
  return String(value ?? "");
}

function updateField(key: string, value: string) {
  formValues.value[key] = value;
}

function updateRole(role: string, value: string | null) {
  formValues.value.roles = {
    ...(formValues.value.roles as Record<string, string> || {}),
    [role]: value || "",
  };
}


function modelListValue(): Array<Record<string, unknown>> {
  return Array.isArray(formValues.value.models)
    ? formValues.value.models.filter((item): item is Record<string, unknown> => Boolean(item && typeof item === "object" && !Array.isArray(item)))
    : [];
}

function updateModelListItem(index: number, key: string, value: unknown) {
  const models = modelListValue().map((item) => ({ ...item }));
  if (!models[index]) return;
  if (value === undefined || value === "" || value === null) delete models[index][key];
  else models[index][key] = value;
  formValues.value.models = models;
}

function addModelListItem() {
  formValues.value.models = [...modelListValue(), { id: "" }];
}

function removeModelListItem(index: number) {
  formValues.value.models = modelListValue().filter((_, itemIndex) => itemIndex !== index);
}
function parseStructuredValues(): Record<string, unknown> {
  const values = { ...formValues.value };
  for (const field of spec.value?.fields || []) {
    if (!["key-value", "json", "model-list", "model-map"].includes(field.type)) continue;
    const existing = values[field.key];
    const expectedArray = field.type === "model-list" || field.type === "model-map";
    if (existing && typeof existing === "object") {
      if (expectedArray ? !Array.isArray(existing) : Array.isArray(existing)) {
        throw new Error(`${field.label}必须是 JSON ${expectedArray ? "数组" : "对象"}`);
      }
      continue;
    }
    const source = String(existing ?? "").trim();
    if (!source) {
      values[field.key] = expectedArray ? [] : {};
      continue;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(source);
    } catch {
      throw new Error(`${field.label}必须是合法 JSON`);
    }
    if (expectedArray ? !Array.isArray(parsed) : !parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error(`${field.label}必须是 JSON ${expectedArray ? "数组" : "对象"}`);
    }
    values[field.key] = parsed;
  }
  return values;
}

function validateStructuredValues(values: Record<string, unknown>) {
  const fetchedIds = new Set(fetchedModels.value.map((model) => model.id));
  const assertModelAvailable = (label: string, raw: unknown) => {
    const id = String(raw ?? "").trim();
    if (id && modelsFetched.value && !fetchedIds.has(id)) throw new Error(`${label}必须从已获取的模型列表中选择`);
  };
  for (const field of spec.value?.fields || []) {
    const value = values[field.key];
    if (field.type === "select" && value !== undefined && value !== "" && !selectOptions(field).some((option) => option.value === value)) {
      throw new Error(`${field.label}不是有效选项`);
    }
    if (field.type === "model-select") assertModelAvailable(field.label, value);
    if (field.type === "model-roles" && value && typeof value === "object" && !Array.isArray(value)) {
      for (const [role, model] of Object.entries(value as Record<string, unknown>)) assertModelAvailable(`${field.label}（${role}）`, model);
    }
    if (field.type === "number") {
      const raw = String(value ?? "").trim();
      if (raw && (!/^\d+$/.test(raw) || Number(raw) < (field.min ?? 0))) {
        throw new Error(`${field.label}必须是有效的非负整数`);
      }
    }
    if (field.type === "key-value") {
      const object = value;
      if (!object || typeof object !== "object" || Array.isArray(object)) continue;
      for (const [key, itemValue] of Object.entries(object)) {
        if (!key.trim()) throw new Error(`${field.label}存在空键名`);
        if (typeof itemValue !== "string") throw new Error(`${field.label}的值必须是字符串`);
      }
    }
    if ((field.type === "model-list" || field.type === "model-map") && Array.isArray(value)) {
      const seen: Record<string, true> = {};
      for (const item of value) {
        if (!item || typeof item !== "object" || Array.isArray(item)) throw new Error(`${field.label}包含无效模型项`);
        const model = item as Record<string, unknown>;
        const id = String(model.id ?? "").trim();
        if (!id) throw new Error(`${field.label}存在空模型 ID`);
        assertModelAvailable(field.label, id);
        if (seen[id]) throw new Error(`${field.label}存在重复模型 ID：${id}`);
        seen[id] = true;
        if (field.type === "model-list" && model.context_length !== undefined && (!Number.isInteger(Number(model.context_length)) || Number(model.context_length) <= 0)) {
          throw new Error(`${field.label}的上下文长度必须是正整数`);
        }
      }
    }
  }
}
function endpointCredentials() {
  const form = formValues.value;
  if (activeTab.value === "claude_code") return { baseUrl: String(form.ANTHROPIC_BASE_URL || ""), apiKey: String(form.ANTHROPIC_AUTH_TOKEN || "") };
  if (activeTab.value === "opencode") return { baseUrl: String(form.baseURL || ""), apiKey: String(form.apiKey || "") };
  if (activeTab.value === "gemini_cli") return { baseUrl: String(form.GOOGLE_GEMINI_BASE_URL || ""), apiKey: String(form.GEMINI_API_KEY || "") };
  return { baseUrl: String(form.base_url || ""), apiKey: String(form.api_key || "") };
}

async function fetchModels() {
  const { baseUrl, apiKey } = endpointCredentials();
  if (!baseUrl.trim() || !apiKey.trim()) {
    message.warning("请先填写 API 端点与 API Key");
    return;
  }
  fetchingModels.value = true;
  modelsFetched.value = false;
  fetchedModels.value = [];
  try {
    fetchedModels.value = await providersApi.fetchModels({ api_base_url: baseUrl.trim(), api_key: apiKey.trim(), timeout_seconds: 10 });
    modelsFetched.value = true;
    message[fetchedModels.value.length ? "success" : "warning"](fetchedModels.value.length ? `已获取 ${fetchedModels.value.length} 个模型` : "未获取到模型");
  } catch (error: any) {
    message.error(error?.message || "获取模型失败");
  } finally {
    fetchingModels.value = false;
  }
}

async function openAdd() {
  editingId.value = null;
  formName.value = "";
  fetchedModels.value = [];
  modelsFetched.value = false;
  channelFillId.value = null;
  channelFillHint.value = null;
  try {
    const defaults = await presetsApi.getDefaults(activeTab.value);
    formValues.value = { ...defaults.values };
  } catch {
    formValues.value = {};
  }
  void loadChannels();
  showModal.value = true;
}

function openEdit(preset: Preset) {
  editingId.value = preset.id;
  formName.value = preset.name;
  formValues.value = formSpecFor(preset.agent_type)?.fromSettings(preset.settings_config) || {};
  fetchedModels.value = [];
  modelsFetched.value = false;
  channelFillId.value = null;
  channelFillHint.value = null;
  void loadChannels();
  showModal.value = true;
}

async function activate(preset: Preset) {
  // opencode 为累加模式（对齐 cc-switch additive）：卡片按钮 = 对该预设独立启停，
  // 激活加入 opencode.json 的 provider 段、取消激活移出，均不影响其他已激活预设。
  // 其余应用为单激活整体切换（对齐 cc-switch Claude）：激活项主按钮已禁用为“当前激活”，
  // 此处只会被非激活卡片触发 = 切换到该预设并激活。
  const isOpenCode = activeTab.value === "opencode";
  const activating = !preset.is_active;
  try {
    const result = isOpenCode
      ? await presetsApi.setActive(activeTab.value, preset.id, activating)
      : await presetsApi.switch(activeTab.value, preset.id);
    message.success(
      isOpenCode
        ? activating
          ? `已激活「${preset.name}」`
          : `已取消激活「${preset.name}」`
        : `已激活「${preset.name}」`,
    );
    for (const warning of result.warnings) message.warning(warning);
    await loadPresets();
  } catch (error: any) {
    message.error(error?.message || "操作失败");
  }
}

async function save(activateAfter: boolean) {
  if (!formName.value.trim()) {
    message.warning("请输入预设名称");
    return;
  }
  if (!spec.value) {
    message.error("该 Agent 类型不支持结构化表单");
    return;
  }

  let settings: Record<string, unknown>;
  try {
    const values = parseStructuredValues();
    validateStructuredValues(values);
    settings = spec.value.toSettings(values);
  } catch (error: any) {
    message.error(error?.message || "高级配置格式错误");
    return;
  }

  try {
    let savedId = editingId.value;
    if (savedId) {
      await presetsApi.update(savedId, { name: formName.value.trim(), settings_config: settings });
      message.success("已更新");
    } else {
      savedId = (await presetsApi.create({ name: formName.value.trim(), agent_type: activeTab.value, settings_config: settings })).id;
      message.success("已创建");
    }
    showModal.value = false;
    await loadPresets();
    if (activateAfter) {
      const preset = presets.value.find((item) => item.id === savedId);
      if (preset) {
        if (activeTab.value === "opencode") {
          // opencode 保存并激活 = 确保置为激活态（累加模式，不清除其他已激活项）。
          // 编辑已激活的 preset 时 update() 已同步 live，仅需在尚未激活时补置位。
          if (!preset.is_active) {
            const result = await presetsApi.setActive(activeTab.value, preset.id, true);
            message.success(`已激活「${preset.name}」`);
            for (const warning of result.warnings) message.warning(warning);
            await loadPresets();
          }
        } else {
          await activate(preset);
        }
      }
    }
  } catch (error: any) {
    message.error(error?.message || "操作失败");
  }
}

function remove(preset: Preset) {
  if (preset.is_active) {
    message.warning("当前激活的预设不可直接删除，请先取消激活后重试");
    return;
  }
  confirm({
    title: "删除预设",
    description: "删除后将无法再用该预设启动对应 Agent。",
    targetLabel: "预设",
    target: preset.name,
    impacts: [
      "预设内的配置与已填写的凭据将被移除",
      "此操作不可撤销，需要重新配置才能恢复",
    ],
    positiveText: "删除",
    destructive: true,
    onConfirm: async () => {
      await presetsApi.remove(preset.id);
      message.success("已删除");
      await loadPresets();
    },
    onError: (error: any) => message.error(error?.message || "删除失败"),
  });
}

function onDrop(target: Preset) {
  const fromId = draggingId.value;
  draggingId.value = null;
  if (!fromId || fromId === target.id) return;
  if (isOfficialPreset(target)) return; // 官方直连行恒置顶，不可作为拖放目标
  const from = presets.value.findIndex((item) => item.id === fromId);
  const to = presets.value.findIndex((item) => item.id === target.id);
  if (from < 0 || to < 0 || from === to) return;
  // 官方行不参与排序：本地展示与提交顺序都剔除官方行并恒排首位
  const reordered = [...presets.value];
  const [moved] = reordered.splice(from, 1);
  reordered.splice(to, 0, moved);
  const official = reordered.filter(isOfficialPreset);
  const others = reordered.filter((preset) => !isOfficialPreset(preset));
  presets.value = [...official, ...others];
  void presetsApi.reorder(activeTab.value, others.map((preset) => preset.id)).catch(async (error: any) => {
    message.error(error?.message || "保存排序失败");
    await loadPresets();
  });
}

watch(activeTab, () => void loadPresets());
onMounted(async () => {
  await loadAgentTypes();
  await loadPresets();
});
</script>

<template>
  <AppPageShell
    :title="`${tabLabel(activeTab)} 预设`"
    :desc="pageDesc"
    :loading="loading"
    :empty="presets.length === 0"
    empty-title="暂无预设"
    empty-description="点击右上角「新建预设」创建"
  >
    <template #actions>
      <NButton type="primary" @click="openAdd">+ 新建预设</NButton>
    </template>

    <template #before>
      <div class="agent-tabs">
        <button
          v-for="agent in agentTypes"
          :key="agent.id"
          class="agent-tab"
          :class="{ active: activeTab === agent.id }"
          @click="activeTab = agent.id"
        >
          {{ agent.name }}
        </button>
      </div>
    </template>

    <div class="s-grid-auto">
      <div
        v-for="preset in presets"
        :key="preset.id"
        class="s-card"
        :class="{ 'is-active': preset.is_active, 'is-dragging': draggingId === preset.id }"
        :draggable="!isOfficialPreset(preset)"
        @dragstart="draggingId = preset.id"
        @dragover.prevent
        @drop.prevent="onDrop(preset)"
      >
        <div class="s-card-head">
          <span class="s-card-title">{{ preset.name }}</span>
          <div class="preset-tags">
            <span v-if="isOfficialPreset(preset)" class="s-badge s-badge--accent">官方</span>
            <span v-if="preset.is_active && activeTab === 'opencode'" class="s-badge s-badge--success">已激活</span>
            <span v-else-if="preset.is_active && activeTab !== 'opencode'" class="s-badge s-badge--success">当前</span>
          </div>
        </div>
        <div class="s-card-body">
          <p v-if="isOfficialPreset(preset)" class="preset-desc">{{ officialDesc(preset.agent_type) }}</p>
          <p v-else-if="preset.notes" class="preset-desc">{{ preset.notes }}</p>
          <div class="preset-actions">
            <!-- 单激活应用（claude_code/codex/hermes/gemini_cli）对齐 cc-switch：激活项无“取消激活”，
                 主按钮禁用显示“当前激活”，切换只能点其他卡片；opencode 为累加模式保留独立启停 -->
            <NButton v-if="activeTab !== 'opencode' && preset.is_active" size="small" disabled>当前激活</NButton>
            <NButton v-else size="small" type="primary" ghost @click="activate(preset)">{{ activeTab === "opencode" && preset.is_active ? "取消激活" : "激活" }}</NButton>
            <!-- 官方直连行：凭据可编辑 + 一键恢复默认，不允许删除 -->
            <template v-if="isOfficialPreset(preset)">
              <NButton size="small" @click="openOfficialEdit(preset)">编辑</NButton>
              <NButton size="small" @click="resetOfficial(preset)">恢复默认</NButton>
            </template>
            <template v-else>
              <NButton size="small" @click="openEdit(preset)">编辑</NButton>
              <NButton size="small" type="error" ghost :disabled="preset.is_active" @click="remove(preset)">删除</NButton>
            </template>
          </div>
        </div>
      </div>
    </div>

    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editingId ? '编辑预设' : '新建预设'"
      style="width: min(600px, calc(100vw - 32px))"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <template v-if="spec">
        <div class="m-body">
          <p v-if="nativeProtocolHint()" class="m-tip">{{ nativeProtocolHint() }}</p>

          <!-- 1. 基本信息：名称 + 从渠道一键填充 -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">基本信息</span>
              <span class="m-sec-meta">{{ spec.label }}</span>
            </div>
            <div class="m-grid">
              <div class="m-field">
                <label class="m-label">预设名称</label>
                <NInput v-model:value="formName" placeholder="请输入预设名称" @keydown.enter="save(false)" />
              </div>
              <div class="m-field">
                <label class="m-label">从渠道填充</label>
                <NSelect
                  v-model:value="channelFillId"
                  :options="channelOptions"
                  filterable
                  clearable
                  :loading="channelsLoading"
                  :disabled="channels.length === 0"
                  :placeholder="channels.length === 0 ? '暂无渠道，请先在「渠道」页配置' : '选择渠道，自动填充名称、端点与 Key'"
                  @update:value="applyChannelFill"
                />
              </div>
              <p v-if="channelFillHint" class="m-note m-full">{{ channelFillHint }}</p>
            </div>
          </section>

          <!-- 2. 接入参数：端点 / Key / 模型，短字段双栏 -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">接入参数</span>
              <span v-if="fetchedModels.length" class="m-sec-meta">已获取 {{ fetchedModels.length }} 个模型</span>
              <NButton
                size="tiny"
                secondary
                class="m-sec-action"
                :loading="fetchingModels"
                :disabled="!formValues[modelEndpointKey]"
                @click="fetchModels"
              >
                获取模型
              </NButton>
            </div>

            <div class="m-grid">
              <div
                v-for="field in basicFields"
                :key="field.key"
                class="m-field"
                :class="{ 'm-full': isWideField(field) }"
              >
                <label class="m-label">{{ field.label }}</label>
                <template v-if="field.type === 'model-roles'">
                  <div v-for="role in field.roles || []" :key="role" class="role-row">
                    <span class="role-label">{{ role }}</span>
                    <NSelect
                      :value="(formValues.roles as Record<string, string> || {})[role]"
                      :options="modelOptions"
                      filterable
                      clearable
                      :disabled="modelSelectorDisabled"
                      :placeholder="modelSelectorDisabled ? '请先获取模型列表' : `${role} 模型`"
                      @update:value="(value) => updateRole(role, value)"
                    />
                  </div>
                </template>
                <NSelect
                  v-else-if="field.type === 'model-select'"
                  :value="formValues[field.key] as string"
                  :options="modelOptions"
                  filterable
                  clearable
                  :disabled="modelSelectorDisabled"
                  :placeholder="modelSelectorDisabled ? '请先获取模型列表' : field.placeholder"
                  @update:value="(value) => updateField(field.key, value || '')"
                />
                <NSelect
                  v-else-if="field.type === 'select'"
                  :value="String(formValues[field.key] || '')"
                  :options="selectOptions(field).map((option) => ({ label: option.label, value: option.value, disabled: option.disabled }))"
                  :placeholder="field.placeholder"
                  @update:value="(value) => updateField(field.key, value || '')"
                />
                <template v-else-if="field.type === 'model-list' || field.type === 'model-map'">
                  <div class="model-list-editor">
                    <div v-for="(model, index) in modelListValue()" :key="index" class="model-row">
                      <NSelect :value="String(model.id || '')" :options="modelOptions" filterable clearable :disabled="modelSelectorDisabled" placeholder="选择模型" @update:value="(value) => updateModelListItem(index, 'id', value || '')" />
                      <NInput :value="String(model.name || '')" placeholder="显示名称（可选）" @update:value="(value) => updateModelListItem(index, 'name', value)" />
                      <NInputNumber :value="typeof model.context_length === 'number' ? model.context_length : null" :min="1" placeholder="上下文" @update:value="(value) => updateModelListItem(index, 'context_length', value)" />
                      <NButton size="small" quaternary type="error" aria-label="删除模型" @click="removeModelListItem(index)">删除</NButton>
                    </div>
                    <NButton size="small" dashed :disabled="modelSelectorDisabled" @click="addModelListItem">添加模型</NButton>
                  </div>
                </template>
                <NInput v-else-if="field.type === 'secret'" :value="fieldValue(field)" type="password" show-password-on="click" :placeholder="field.placeholder" @update:value="(value) => updateField(field.key, value)" />
                <NInput v-else :value="fieldValue(field)" :placeholder="field.placeholder" :inputmode="field.type === 'number' ? 'numeric' : undefined" @update:value="(value) => updateField(field.key, value)" />
                <span v-if="field.hint" class="m-note">{{ field.hint }}</span>
              </div>
            </div>
          </section>

          <!-- 3. 高级配置：原生环境变量 / JSON，改动过才提示 -->
          <ModalAdvanced
            v-if="advancedFields.length"
            :count="advancedFilledCount"
            hint="环境变量 · 原生 JSON"
          >
            <div v-for="field in advancedFields" :key="field.key" class="m-field adv-extra">
              <label class="m-label">
                {{ field.label }}
                <span v-if="field.hint" class="m-note">{{ field.hint }}</span>
              </label>
              <NInput
                :value="fieldValue(field)"
                type="textarea"
                :autosize="{ minRows: 3, maxRows: 8 }"
                :placeholder="field.placeholder"
                @update:value="(value) => updateField(field.key, value)"
              />
            </div>
          </ModalAdvanced>
        </div>
      </template>
      <p v-else class="s-state-desc">该 Agent 类型暂无结构化表单</p>
      <template #footer>
        <div class="modal-actions">
          <NButton size="small" @click="showModal = false">取消</NButton>
          <NButton size="small" type="primary" ghost @click="save(true)">保存并激活</NButton>
          <NButton size="small" type="primary" @click="save(false)">保存</NButton>
        </div>
      </template>
    </NModal>

    <!-- 官方直连配置编辑：端点/模型锚定官方，仅凭据可填（与后端清洗白名单对齐） -->
    <NModal
      v-model:show="officialModal"
      preset="card"
      title="官方直连配置"
      style="width: min(520px, calc(100vw - 32px))"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <template v-if="officialEditing">
        <div class="m-body">
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">基本信息</span>
              <span class="m-sec-meta">{{ officialEditing.agent_type }}</span>
            </div>
            <div class="m-field">
              <label class="m-label">预设名称</label>
              <NInput v-model:value="officialName" placeholder="请输入预设名称" />
            </div>
          </section>

          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">凭据</span>
              <span class="m-sec-meta">可留空，留空则保持原值</span>
            </div>
            <div
              v-for="field in officialFieldsOf(officialEditing.agent_type)"
              :key="field.key"
              class="m-field"
            >
              <label class="m-label">{{ field.label }}</label>
              <NInput
                v-model:value="officialCredential[field.key]"
                type="password"
                show-password-on="click"
                :placeholder="`${field.label}（可留空）`"
              />
            </div>
            <p class="m-sec-desc">{{ officialModalTip(officialEditing.agent_type) }}</p>
          </section>
        </div>
      </template>
      <template #footer>
        <div class="modal-actions">
          <NButton size="small" @click="officialModal = false">取消</NButton>
          <NButton size="small" type="primary" @click="saveOfficial">保存</NButton>
        </div>
      </template>
    </NModal>
  </AppPageShell>
</template>

<style scoped>
/* 卡片 / 页头 / 统计 / 徽标 / 表格 / 弹窗分区全部走 style.css 的 p-* / s-* / m-* 规范，
   这里只保留本页特有的「预设卡 / 模型编辑器 / 分段控件」等业务造型。 */

/* 分段控件：造型保留，间距 / 字号 / 圆角 / 颜色全部走令牌 */
.agent-tabs {
  display: inline-flex;
  gap: var(--sp-1);
  margin-bottom: var(--sp-6);
  padding: var(--sp-1);
  background: var(--surface-alt);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.agent-tab {
  min-width: 100px;
  padding: var(--sp-2) var(--sp-4);
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  cursor: pointer;
  font-family: inherit;
  font-size: var(--fs-base);
  font-weight: 500;
  color: var(--muted);
  transition: background-color 120ms ease, color 120ms ease;
}

.agent-tab:hover {
  background: var(--hover-bg);
  color: var(--fg-2);
}

.agent-tab.active {
  background: var(--fg);
  color: var(--surface);
  font-weight: 600;
}

/* 整卡可拖拽：仅保留 grab 光标与激活 / 拖拽态，其余走 .s-card */
.s-card {
  cursor: grab;
}

.s-card.is-active {
  border-color: var(--success);
}

.s-card.is-dragging {
  opacity: 0.55;
  border-style: dashed;
}

.preset-tags {
  display: flex;
  align-items: center;
  gap: var(--sp-1);
}

.preset-desc {
  margin: 0 0 var(--sp-2);
  color: var(--muted);
  font-size: var(--fs-sm);
  line-height: 1.5;
}

.preset-actions {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

/* 模型列表编辑器：条目的列宽用 flex-wrap 自适应，不再写私有 @media */
.model-list-editor {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.model-row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  align-items: center;
}

.model-row > :nth-child(1) {
  flex: 2 1 160px;
}

.model-row > :nth-child(2) {
  flex: 1 1 120px;
}

.model-row > :nth-child(3) {
  flex: 0 0 110px;
}

.model-row > :nth-child(4) {
  flex: 0 0 auto;
}

.role-row {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.role-label {
  width: 70px;
  color: var(--muted);
  font-size: var(--fs-sm);
  font-weight: 500;
}

.modal-actions {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  justify-content: flex-end;
}
</style>
