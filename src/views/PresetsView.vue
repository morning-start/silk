<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NButton, NInput, NInputNumber, NModal, NSelect, NTag, useDialog, useMessage } from "naive-ui";
import { api, type AgentTypeInfo, type Preset, type ProviderModelInfo } from "../api";
import { formSpecFor, type HarnessField, type HarnessFormSpec } from "../config/harnessForms";

const message = useMessage();
const dialog = useDialog();
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

const currentPreset = computed(() => presets.value.find((preset) => preset.is_active) || null);
const openCodeActiveCount = computed(() => {
  if (activeTab.value !== 'opencode') return 0;
  return presets.value.filter((preset) => preset.is_active).length;
});
const spec = computed<HarnessFormSpec | undefined>(() => formSpecFor(activeTab.value));
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
    agentTypes.value = await api.listAgentTypes();
    if (agentTypes.value.length > 0) activeTab.value = agentTypes.value[0].id;
  } catch (error: any) {
    message.error(error?.message || "加载 Agent 类型失败");
  }
}

async function loadPresets() {
  loading.value = true;
  try {
    presets.value = await api.listPresets(activeTab.value);
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
    fetchedModels.value = await api.fetchProviderModels({ api_base_url: baseUrl.trim(), api_key: apiKey.trim(), timeout_seconds: 10 });
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
  try {
    const defaults = await api.getPresetDefaults(activeTab.value);
    formValues.value = { ...defaults.values };
  } catch {
    formValues.value = {};
  }
  showModal.value = true;
}

function openEdit(preset: Preset) {
  editingId.value = preset.id;
  formName.value = preset.name;
  formValues.value = formSpecFor(preset.agent_type)?.fromSettings(preset.settings_config) || {};
  fetchedModels.value = [];
  modelsFetched.value = false;
  showModal.value = true;
}

async function activate(preset: Preset) {
  try {
    const result = await api.switchPreset(activeTab.value, preset.id);
    const isActive = preset.is_active;
    message.success(isActive ? `已取消激活「${preset.name}」` : `已激活「${preset.name}」`);
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
      await api.updatePreset(savedId, { name: formName.value.trim(), settings_config: settings });
      message.success("已更新");
    } else {
      savedId = (await api.createPreset({ name: formName.value.trim(), agent_type: activeTab.value, settings_config: settings })).id;
      message.success("已创建");
    }
    showModal.value = false;
    await loadPresets();
    if (activateAfter) {
      const preset = presets.value.find((item) => item.id === savedId);
      if (preset) await activate(preset);
    }
  } catch (error: any) {
    message.error(error?.message || "操作失败");
  }
}

function remove(preset: Preset) {
  if (preset.is_active) {
    message.warning("当前激活的预设不可删除，请先切换到其他预设");
    return;
  }
  dialog.warning({
    title: "确认删除",
    content: `确定要删除预设「${preset.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deletePreset(preset.id);
        message.success("已删除");
        await loadPresets();
      } catch (error: any) {
        message.error(error?.message || "删除失败");
      }
    },
  });
}

function onDrop(target: Preset) {
  const fromId = draggingId.value;
  draggingId.value = null;
  if (!fromId || fromId === target.id) return;
  const from = presets.value.findIndex((item) => item.id === fromId);
  const to = presets.value.findIndex((item) => item.id === target.id);
  if (from < 0 || to < 0 || from === to) return;
  const reordered = [...presets.value];
  const [moved] = reordered.splice(from, 1);
  reordered.splice(to, 0, moved);
  presets.value = reordered;
  void api.updatePresetOrder(activeTab.value, reordered.map((preset) => preset.id)).catch(async (error: any) => {
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
  <div class="presets-page">
    <div class="agent-tabs">
      <button v-for="agent in agentTypes" :key="agent.id" class="agent-tab" :class="{ active: activeTab === agent.id }" @click="activeTab = agent.id">
        {{ agent.name }}
      </button>
    </div>

    <div class="page-header">
      <div>
        <h2>{{ tabLabel(activeTab) }} 预设</h2>
        <p class="subtitle">{{ activeTab === 'opencode' && openCodeActiveCount > 0 ? `已激活 ${openCodeActiveCount} 个配置` : currentPreset ? `当前激活：${currentPreset.name}` : "未激活任何预设（live 配置保持原状）" }}</p>
      </div>
      <NButton type="primary" @click="openAdd">+ 新建预设</NButton>
    </div>

    <div v-if="loading" class="empty-desc">加载中…</div>
    <div v-else-if="presets.length === 0" class="empty-state"><p class="empty-desc">暂无预设，点击右上角「新建预设」创建</p></div>
    <div v-else class="preset-grid">
      <div v-for="preset in presets" :key="preset.id" class="preset-card" :class="{ active: preset.is_active, dragging: draggingId === preset.id }" draggable="true" @dragstart="draggingId = preset.id" @dragover.prevent @drop.prevent="onDrop(preset)">
        <div class="preset-card-head">
          <span class="preset-name">{{ preset.name }}</span>
          <NTag v-if="preset.is_active && activeTab === 'opencode'" type="success" size="small">已激活</NTag>
          <NTag v-else-if="preset.is_active && activeTab !== 'opencode'" type="success" size="small">当前</NTag>
        </div>
        <div class="preset-actions">
          <NButton size="small" type="primary" ghost @click="activate(preset)">{{ preset.is_active ? "取消激活" : "激活" }}</NButton>
          <NButton size="small" @click="openEdit(preset)">编辑</NButton>
          <NButton size="small" type="error" ghost :disabled="preset.is_active" @click="remove(preset)">删除</NButton>
        </div>
      </div>
    </div>
  <NModal v-model:show="showModal" preset="card" :title="editingId ? '编辑预设' : '新建预设'" style="width: 560px">
      <template v-if="spec">
        <div class="form-item fetch-row">
          <label>模型列表</label>
          <NButton size="small" type="primary" ghost :loading="fetchingModels" :disabled="!formValues[modelEndpointKey]" @click="fetchModels">获取模型</NButton>
          <span v-if="fetchedModels.length" class="fetch-count">已获取 {{ fetchedModels.length }} 个</span>
        </div>
        <div v-for="field in spec.fields" :key="field.key" class="form-item">
          <label>{{ field.label }}</label>
          <template v-if="field.type === 'model-roles'">
            <div v-for="role in field.roles || []" :key="role" class="role-row">
              <span class="role-label">{{ role }}</span>
              <NSelect :value="(formValues.roles as Record<string, string> || {})[role]" :options="modelOptions" filterable clearable :disabled="modelSelectorDisabled" :placeholder="modelSelectorDisabled ? '请先获取模型列表' : `${role} 模型`" @update:value="(value) => updateRole(role, value)" />
            </div>
          </template>
          <NSelect v-else-if="field.type === 'model-select'" :value="formValues[field.key] as string" :options="modelOptions" filterable clearable :disabled="modelSelectorDisabled" :placeholder="modelSelectorDisabled ? '请先获取模型列表' : field.placeholder" @update:value="(value) => updateField(field.key, value || '')" />
          <NSelect v-else-if="field.type === 'select'" :value="String(formValues[field.key] || '')" :options="selectOptions(field).map((option) => ({ label: option.label, value: option.value, disabled: option.disabled }))" :placeholder="field.placeholder" @update:value="(value) => updateField(field.key, value || '')" />
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
          <NInput v-else-if="['key-value', 'json'].includes(field.type)" :value="fieldValue(field)" type="textarea" :autosize="{ minRows: 3, maxRows: 8 }" :placeholder="field.placeholder" @update:value="(value) => updateField(field.key, value)" />
          <NInput v-else :value="fieldValue(field)" :placeholder="field.placeholder" :inputmode="field.type === 'number' ? 'numeric' : undefined" @update:value="(value) => updateField(field.key, value)" />
        </div>
      </template>
      <p v-else class="empty-desc">该 Agent 类型暂无结构化表单</p>
      <template #footer>
        <div class="modal-actions">
          <NButton size="small" @click="showModal = false">取消</NButton>
          <NButton size="small" type="primary" ghost @click="save(true)">保存并激活</NButton>
          <NButton size="small" type="primary" @click="save(false)">保存</NButton>
        </div>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.presets-page { width: 100%; }
.agent-tabs { display: flex; justify-content: center; gap: 6px; margin-bottom: 24px; padding: 4px; background: var(--surface-alt, #f1f5f9); border-radius: 12px; }
.agent-tab { min-width: 100px; padding: 10px 20px; border: 0; border-radius: 8px; background: transparent; cursor: pointer; font-family: inherit; }
.agent-tab:hover { background: var(--hover-bg, #e2e8f0); }
.agent-tab.active { background: #18a058; color: #fff; font-weight: 600; }
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 16px; }
.page-header h2 { margin: 0 0 4px; font-size: 18px; }
.subtitle, .empty-desc, .fetch-count { color: var(--muted, #94a3b8); font-size: 13px; }
.subtitle { margin: 0; }
.preset-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
.preset-card { padding: 14px; border: 1px solid var(--border-color, #e2e8f0); border-radius: 10px; background: var(--card-bg, #fff); cursor: grab; }
.model-list-editor { display: flex; flex-direction: column; gap: 8px; }
.model-row { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) 110px auto; gap: 6px; align-items: center; }
@media (max-width: 700px) {
  .model-row { grid-template-columns: 1fr 1fr; }
}
.preset-card.active { border-color: #18a058; box-shadow: 0 0 0 1px rgb(24 160 88 / 30%); }
.preset-card.dragging { opacity: .55; border-style: dashed; }
.preset-card-head, .preset-actions, .modal-actions, .fetch-row, .role-row { display: flex; align-items: center; }
.preset-card-head { justify-content: space-between; margin-bottom: 12px; }
.preset-name { font-size: 14px; font-weight: 600; }
.preset-actions, .modal-actions { gap: 6px; }
.empty-state { padding: 48px 0; text-align: center; }
.form-item { margin-bottom: 12px; }
.form-item > label { display: block; margin-bottom: 6px; color: var(--muted, #64748b); font-size: 13px; }
.fetch-row { gap: 10px; }
.active-count { color: var(--muted, #94a3b8); font-size: 13px; margin-left: 8px; }
.fetch-row label { margin-bottom: 0; }
.role-row { gap: 10px; margin-bottom: 6px; }
.role-label { width: 70px; color: var(--muted, #64748b); font-size: 13px; font-weight: 500; }
.modal-actions { justify-content: flex-end; }
</style>
