<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NButton,
  NCheckbox,
  NGrid,
  NGi,
  NIcon,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  useMessage,
} from "naive-ui";
import { SearchOutline } from "@vicons/ionicons5";
import { storeToRefs } from "pinia";
import { openUrl } from "@tauri-apps/plugin-opener";
import { providersApi } from "../api/providers";
import { discoveryApi } from "../api/discovery";
import type { PresetProvider, Provider, ProviderHeaderEntry } from "../api";
import { copyWithFeedback } from "../utils/clipboard";
import { healthStatusType } from "../utils/health";
import { useConfirm } from "../utils/confirm";
import AppFormModal from "../components/AppFormModal.vue";
import AppPageShell from "../components/AppPageShell.vue";
import ModalAdvanced from "../components/ModalAdvanced.vue";
import { useProvidersStore } from "../stores/providers";

type ProviderKeyForm = {
  name: string;
  value: string;
  enabled: boolean;
  weight: number;
};

const providersStore = useProvidersStore();
const { providers, loading, error } = storeToRefs(providersStore);
const message = useMessage();
const { confirm } = useConfirm();

const searchQuery = ref("");
const showModal = ref(false);
const editingId = ref<string | null>(null);
const fetchingModels = ref(false);
/** 手动添加模型的输入值（/v1/models 无结果时的补充手段） */
const newModel = ref("");
/** 勾选待删除的模型 ID 集合 */
const selectedModels = ref<string[]>([]);
/** 模型列表搜索关键字（模糊：大小写不敏感 + 字符按序匹配） */
const modelSearch = ref("");
/** 过滤后的模型列表（全选/反选作用于过滤结果） */
const filteredModels = computed(() => {
  const q = modelSearch.value.trim().toLowerCase();
  if (!q) return formValue.value.models;
  return formValue.value.models.filter((model) => fuzzyMatch(model, q));
});

/** 模糊匹配：查询串所有字符按顺序出现在目标串中（含普通子串场景） */
function fuzzyMatch(target: string, query: string): boolean {
  const lower = target.toLowerCase();
  let cursor = 0;
  for (const ch of query) {
    const idx = lower.indexOf(ch, cursor);
    if (idx < 0) return false;
    cursor = idx + 1;
  }
  return true;
}
const testingStates = ref<Record<string, boolean>>({});
const keyVisibility = ref<boolean[]>([]);

const protocolOptions = [
  { label: "OpenAI Chat", value: "openai" },
  { label: "Claude Messages", value: "messages" },
  { label: "OpenAI Response", value: "responses" },
  { label: "Google Gemini", value: "gemini" },
];

const keyStrategyOptions = [
  { label: "加权轮询", value: "round_robin" },
  { label: "加权随机", value: "weighted" },
  { label: "最少连接", value: "least_conn" },
];

// ---------------------------------------------------------------------------
// 预置模板：把官方地址 / 协议 / 常用模型一次填好，省掉查文档
// ---------------------------------------------------------------------------

const presetProviders = ref<PresetProvider[]>([]);
const presetId = ref<string | null>(null);

const presetOptions = computed(() =>
  presetProviders.value.map((p) => ({ label: p.name, value: p.id })),
);

const selectedPreset = computed(
  () => presetProviders.value.find((p) => p.id === presetId.value) ?? null,
);

async function loadPresetProviders() {
  if (presetProviders.value.length > 0) return;
  try {
    presetProviders.value = await discoveryApi.getPresetProviders();
  } catch {
    /* 模板加载失败不影响手动填写 */
  }
}

/**
 * 应用预置模板
 *
 * 只覆盖"官方既定"的字段（地址、协议）；名称仅在用户还没填时补上；
 * 模型取并集追加，避免清掉用户已经勾好的模型。
 */
function applyPreset(id: string) {
  const preset = presetProviders.value.find((p) => p.id === id);
  if (!preset) return;
  presetId.value = id;
  formValue.value.api_base_url = preset.api_base_url;
  formValue.value.protocols = [...preset.protocols];
  if (!formValue.value.name.trim()) {
    formValue.value.name = preset.name;
  }
  const merged = new Set(formValue.value.models);
  for (const model of preset.models) merged.add(model.id);
  formValue.value.models = [...merged];
}

async function openPresetKeyUrl() {
  if (!selectedPreset.value) return;
  try {
    await openUrl(selectedPreset.value.api_key_url);
  } catch {
    message.error("打开链接失败");
  }
}

const formValue = ref({
  name: "",
  protocols: [] as string[],
  models: [] as string[],
  api_base_url: "",
  proxy_url: "",
  timeout_seconds: 30,
  max_retries: 3,
  status: "enabled",
  models_passthrough: false,
  key_strategy: "round_robin",
  keys: [] as ProviderKeyForm[],
  custom_headers: [] as ProviderHeaderEntry[],
});

const filteredProviders = computed(() => {
  const q = searchQuery.value.toLowerCase().trim();
  if (!q) return providers.value;
  return providers.value.filter(
    (p) =>
      p.name.toLowerCase().includes(q) ||
      p.api_base_url.toLowerCase().includes(q) ||
      p.protocols?.some((pr) => pr.toLowerCase().includes(q)),
  );
});

const hasEnabledKey = computed(() =>
  formValue.value.keys.some((key) => key.enabled && key.value.trim()),
);

const canFetchModels = computed(
  () => !!formValue.value.api_base_url.trim() && hasEnabledKey.value,
);

const canSubmit = computed(
  () =>
    !!formValue.value.name.trim() &&
    formValue.value.protocols.length > 0 &&
    !!formValue.value.api_base_url.trim() &&
    hasEnabledKey.value,
);

// 加权轮询 / 加权随机都使用 weight 字段，最少连接不使用
const isWeightedStrategy = computed(
  () =>
    formValue.value.key_strategy === "round_robin" ||
    formValue.value.key_strategy === "weighted",
);

/** 有效密钥数量（分区统计 + footer 摘要共用） */
const enabledKeyCount = computed(
  () => formValue.value.keys.filter((key) => key.enabled && key.value.trim()).length,
);

/** 高级设置中偏离默认值的项数，>0 时标题显示「已自定义 N 项」 */
const advancedChangedCount = computed(() => {
  const value = formValue.value;
  let count = 0;
  if (value.proxy_url.trim()) count += 1;
  if (value.timeout_seconds !== 30) count += 1;
  if (value.max_retries !== 3) count += 1;
  if (value.custom_headers.length > 0) count += 1;
  return count;
});

/** footer 常驻摘要：滚到表单深处时仍能确认当前配置的关键结论 */
const modalSummary = computed(() => [
  `${enabledKeyCount.value}/${formValue.value.keys.length} 密钥`,
  `${formValue.value.models.length} 模型`,
  formValue.value.protocols.length > 0 ? formValue.value.protocols.join(" / ") : "未选协议",
]);

function createDefaultKey(name = "默认"): ProviderKeyForm {
  return { name, value: "", enabled: true, weight: 1 };
}

function createDefaultHeader(): ProviderHeaderEntry {
  return { name: "", value: "", enabled: true };
}

function resetForm() {
  editingId.value = null;
  selectedModels.value = [];
  modelSearch.value = "";
  presetId.value = null;
  keyVisibility.value = [false];
  formValue.value = {
    name: "",
    protocols: [],
    models: [],
    api_base_url: "",
    proxy_url: "",
    timeout_seconds: 30,
    max_retries: 3,
    status: "enabled",
    models_passthrough: false,
    key_strategy: "round_robin",
    keys: [createDefaultKey()],
    custom_headers: [],
  };
}

function closeModal() {
  showModal.value = false;
}

function handleAdd() {
  resetForm();
  showModal.value = true;
}

function handleEdit(row: Provider) {
  editingId.value = row.id;
  selectedModels.value = [];
  modelSearch.value = "";
  presetId.value = null;  keyVisibility.value = (row.keys && row.keys.length > 0)
    ? row.keys.map(() => false)
    : [false];
  formValue.value = {
    name: row.name,
    protocols: row.protocols || [],
    models: row.models || [],
    api_base_url: row.api_base_url,
    proxy_url: row.proxy_url || "",
    timeout_seconds: row.timeout_seconds,
    max_retries: row.max_retries,
    status: row.status,
    models_passthrough: row.models_passthrough ?? false,
    key_strategy: row.key_strategy || "round_robin",
    keys:
      row.keys && row.keys.length > 0
        ? row.keys.map((key) => ({
            name: key.name,
            value: key.value,
            enabled: key.enabled,
            weight: key.weight,
          }))
        : [createDefaultKey()],
    custom_headers:
      row.custom_headers && row.custom_headers.length > 0
        ? row.custom_headers.map((h) => ({ ...h }))
        : [],
  };
  showModal.value = true;
}

function addKey() {
  formValue.value.keys.push(createDefaultKey(""));
  keyVisibility.value.push(false);
}

function addHeader() {
  formValue.value.custom_headers.push(createDefaultHeader());
}

function removeHeader(index: number) {
  formValue.value.custom_headers.splice(index, 1);
}

function removeKey(index: number) {
  if (formValue.value.keys.length === 1) {
    formValue.value.keys[0] = createDefaultKey();
    keyVisibility.value[0] = false;
    return;
  }
  formValue.value.keys.splice(index, 1);
  keyVisibility.value.splice(index, 1);
}

function toggleKeyVisibility(index: number) {
  keyVisibility.value[index] = !keyVisibility.value[index];
}

async function copyKeyValue(value: string) {
  if (!value.trim()) {
    message.warning("当前 Key 为空");
    return;
  }
  const ok = await copyWithFeedback(value, "API Key");
  if (ok) {
    message.success("Key 已复制到剪贴板");
  } else {
    message.error("复制失败");
  }
}

function normalizeUrl() {
  const url = formValue.value.api_base_url.trim();
  formValue.value.api_base_url = url.replace(/\/v1\/?$/, "").replace(/\/+$/, "");
}

// 供应商整体状态：禁用优先于健康检测结果
function healthLabel(item: Provider): string {
  if (item.status !== "enabled") return "禁用";
  return item.health_status === "healthy" ? "正常" : "异常";
}

function keySummary(item: Provider): string {
  return `${item.key_count || 0} Keys`;
}

async function fetchModels() {
  const apiKey =
    formValue.value.keys.find((key) => key.enabled && key.value.trim())?.value || "";
  if (!formValue.value.api_base_url.trim() || !apiKey) {
    message.warning("请先填写 API 地址和 API Key");
    return;
  }

  normalizeUrl();
  fetchingModels.value = true;
  try {
    const models = await providersApi.fetchModels({
      api_base_url: formValue.value.api_base_url,
      api_key: apiKey,
      proxy_url: formValue.value.proxy_url || undefined,
      timeout_seconds: formValue.value.timeout_seconds,
    });
    // 清洗拉取结果：去重、剔除空 id，避免重复 :key 导致 DOM 复用错位（勾选状态串扰）
    const fetched = [...new Set(models.map((model) => model.id).filter(Boolean))];
    // 未保存时读取当前模型数组：不在数组中的新模型默认勾选，便于快速识别新增项
    const prevModels = formValue.value.models;
    formValue.value.models = fetched;
    selectedModels.value = fetched.filter((model) => !prevModels.includes(model));
    message.success(fetched.length > 0 ? `获取到 ${fetched.length} 个模型` : "未返回模型列表");
  } catch (e: any) {
    message.error(e?.message || "获取模型列表失败");
  } finally {
    fetchingModels.value = false;
  }
}

/** 手动添加模型：trim 去空白、非空校验、去重后追加 */
function addModel() {
  const model = newModel.value.trim();
  if (!model) return;
  if (formValue.value.models.includes(model)) {
    message.warning("该模型已存在");
    return;
  }
  formValue.value.models.push(model);
  newModel.value = "";
}

/** 全选：勾选当前可见（含搜索过滤）的全部模型 */
function selectAllModels() {
  selectedModels.value = [...filteredModels.value];
}

/** 反选：翻转当前可见（含搜索过滤）的勾选集合 */
function invertModels() {
  selectedModels.value = filteredModels.value.filter(
    (model) => !selectedModels.value.includes(model)
  );
}

/** 删除选中的模型并清空勾选 */
function deleteSelectedModels() {
  formValue.value.models = formValue.value.models.filter(
    (model) => !selectedModels.value.includes(model)
  );
  selectedModels.value = [];
}

/** 复选框勾选/取消勾选（来自复选框自身事件） */
function setModelSelected(model: string, checked: boolean) {
  const idx = selectedModels.value.indexOf(model);
  if (checked && idx < 0) selectedModels.value.push(model);
  if (!checked && idx >= 0) selectedModels.value.splice(idx, 1);
}

/** 行点击（模型名区域）切换选中 */
function toggleModelSelected(model: string) {
  const idx = selectedModels.value.indexOf(model);
  if (idx >= 0) selectedModels.value.splice(idx, 1);
  else selectedModels.value.push(model);
}

// 安全网：models 变化后把选中集修剪为 models 子集，任何路径都不会残留失效勾选
watch(
  () => formValue.value.models,
  (list) => {
    selectedModels.value = selectedModels.value.filter((model) => list.includes(model));
  }
);

async function handleTest(row: Provider) {
  testingStates.value[row.id] = true;
  try {
    const result = await providersApi.test(row.id);
    if (result.health_status === "healthy") {
      message.success(`连接成功 · ${result.response_time_ms}ms`);
    } else {
      message.error(`连接失败 · ${result.error || "未知错误"}`);
    }
    await providersStore.fetchAll();
  } catch (e: any) {
    message.error(e?.message || "测试失败");
  } finally {
    testingStates.value[row.id] = false;
  }
}

function handleDelete(row: Provider) {
  confirm({
    title: "删除渠道",
    description: "该渠道的 API Key 与模型列表配置会一并移除。",
    targetLabel: "渠道",
    target: row.name,
    impacts: [
      "引用该渠道的模型映射会失去这条链路，需要重新指定渠道",
      "此操作不可撤销",
    ],
    positiveText: "删除",
    destructive: true,
    onConfirm: () => providersStore.remove(row.id),
    onError: () => message.error("删除失败"),
  });
}

async function handleSubmit() {
  try {
    normalizeUrl();
    const payload = {
      ...formValue.value,
      proxy_url: formValue.value.proxy_url.trim() || undefined,
      models: [...formValue.value.models],
      keys: formValue.value.keys.map((key) => ({
        ...key,
        name: key.name.trim() || "默认",
        value: key.value.trim(),
        weight: key.weight || 1,
      })),
    };

    if (editingId.value) {
      await providersStore.update(editingId.value, payload as any);
      message.success("更新成功");
    } else {
      await providersStore.create(payload as any);
      message.success("创建成功");
    }

    closeModal();
  } catch (e: any) {
    // 保存失败必须可见——否则用户以为已保存（如「模型穿透」开关未真正生效）
    message.error(e?.message || "保存失败，请重试");
  }
}

onMounted(() => {
  providersStore.fetchAll();
  loadPresetProviders();
});
</script>

<template>
  <AppPageShell
    title="渠道管理"
    desc="管理转发渠道：配置地址、协议、密钥与模型，并测试连通性。"
    :loading="loading"
    :error="error"
    :empty="filteredProviders.length === 0"
    @reload="providersStore.fetchAll()"
  >
    <template #count>
      <NTag size="small" type="info">{{ providers.length }} 个渠道</NTag>
    </template>
    <template #actions>
      <NInput v-model:value="searchQuery" clearable placeholder="搜索渠道名称、地址或协议..." style="width: 280px">
        <template #prefix>
          <NIcon><SearchOutline /></NIcon>
        </template>
      </NInput>
      <NButton type="primary" @click="handleAdd">+ 新增渠道</NButton>
    </template>
    <template #empty>
      <div v-if="searchQuery.trim()" class="s-state">
        <h3 class="s-state-title">未找到匹配的渠道</h3>
        <p class="s-state-desc">换个关键词，或者直接新增一个渠道。</p>
      </div>
      <div v-else class="s-state">
        <h3 class="s-state-title">暂无渠道</h3>
        <p class="s-state-desc">添加第一个 AI 渠道，开始配置您的 API 网关。</p>
        <NButton type="primary" @click="handleAdd">+ 新增渠道</NButton>
      </div>
    </template>

    <NGrid class="provider-grid" :x-gap="16" :y-gap="16" cols="1 s:2 m:3" responsive="screen">
      <NGi v-for="item in filteredProviders" :key="item.id">
        <div class="s-card" :class="{ disabled: item.status !== 'enabled' }">
          <div class="s-card-head">
            <div class="pc-title">
              <span class="pc-name">{{ item.name }}</span>
              <NTag size="tiny" :type="healthStatusType(item.health_status)">
                {{ healthLabel(item) }}
              </NTag>
            </div>
            <span class="s-card-meta">{{ item.protocols?.length || 0 }} 协议</span>
          </div>

          <div class="s-card-body">
            <div class="s-chip">{{ item.api_base_url.replace(/^https?:\/\//, "") }}</div>

            <div class="pc-meta">
              <NTag size="small" type="info">{{ keySummary(item) }}</NTag>
              <NTag size="small" type="success" v-if="item.models?.length">{{ item.models.length }} 模型</NTag>
              <NTag size="small" type="default">超时 {{ item.timeout_seconds }}s</NTag>
              <NTag size="small" :type="item.models_passthrough ? 'warning' : 'default'">
                {{ item.models_passthrough ? '穿透' : '不穿透' }}
              </NTag>
            </div>

            <div class="pc-proto" v-if="item.protocols?.length">
              <NTag v-for="protocol in item.protocols" :key="protocol" size="tiny" round>{{ protocol }}</NTag>
            </div>

            <div class="pc-models" v-if="item.models?.length">
              <span v-for="model in item.models.slice(0, 4)" :key="model" class="s-chip">{{ model }}</span>
              <NTag v-if="item.models.length > 4" size="tiny" round>+{{ item.models.length - 4 }}</NTag>
            </div>

            <div class="pc-actions">
              <NButton size="tiny" quaternary @click="handleEdit(item)">编辑</NButton>
              <NButton size="tiny" quaternary :loading="testingStates[item.id]" @click="handleTest(item)">测试</NButton>
              <NButton size="tiny" quaternary type="error" @click="handleDelete(item)">删除</NButton>
            </div>
          </div>
        </div>
      </NGi>
    </NGrid>

    <template #after>
      <AppFormModal
        v-model:show="showModal"
        :title="editingId ? '编辑渠道' : '新增渠道'"
        width="720px"
        :submit-text="editingId ? '保存修改' : '确认添加'"
        :submit-disabled="!canSubmit"
        :summary="modalSummary"
        @cancel="closeModal"
        @submit="handleSubmit"
      >
        <div class="m-body">
          <!-- 1. 基本信息：先回答"这是什么渠道、开没开" -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">基本信息</span>
              <span class="m-sec-meta">{{ editingId ? "编辑已有渠道" : "新建渠道" }}</span>
            </div>
            <div class="m-grid" style="--m-cols: 4">
              <div v-if="presetOptions.length > 0" class="m-field m-full">
                <label class="m-label">官方模板</label>
                <div class="preset-row">
                  <NSelect
                    :value="presetId"
                    :options="presetOptions"
                    filterable
                    clearable
                    placeholder="选择官方渠道，自动填好地址 / 协议 / 常用模型"
                    @update:value="(value: string | null) => { if (value) applyPreset(value); }"
                  />
                  <NButton v-if="selectedPreset" size="small" secondary @click="openPresetKeyUrl">
                    去申请 Key
                  </NButton>
                </div>
                <span class="m-note">
                  {{ selectedPreset ? selectedPreset.description : "官方模板只覆盖地址、协议与常用模型，名称与 Key 仍可自行修改" }}
                </span>
              </div>
              <div class="m-field m-span-2">
                <label class="m-label">名称<i class="m-req">*</i></label>
                <NInput v-model:value="formValue.name" placeholder="如：OpenAI 官方" />
              </div>
              <div class="m-field">
                <label class="m-label">状态</label>
                <div class="m-switch">
                  <NSwitch
                    :value="formValue.status === 'enabled'"
                    @update:value="(value: boolean) => { formValue.status = value ? 'enabled' : 'disabled'; }"
                  />
                  <span class="m-note">{{ formValue.status === "enabled" ? "已启用" : "已停用" }}</span>
                </div>
              </div>
              <div class="m-field">
                <label class="m-label">模型穿透</label>
                <div class="m-switch">
                  <NSwitch v-model:value="formValue.models_passthrough" />
                  <span class="m-note">/v1/models 可见</span>
                </div>
              </div>
            </div>
          </section>

          <!-- 2. 接入配置：协议 / 策略 / 地址 -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">接入配置</span>
            </div>
            <div class="m-grid">
              <div class="m-field">
                <label class="m-label">接口协议<i class="m-req">*</i></label>
                <NSelect
                  v-model:value="formValue.protocols"
                  multiple
                  filterable
                  :options="protocolOptions"
                  placeholder="选择协议，可多选"
                />
              </div>
              <div class="m-field">
                <label class="m-label">密钥策略</label>
                <NSelect v-model:value="formValue.key_strategy" :options="keyStrategyOptions" />
                <span class="m-note">多密钥时的调度方式</span>
              </div>
              <div class="m-field m-full">
                <label class="m-label">API 地址<i class="m-req">*</i></label>
                <NInput v-model:value="formValue.api_base_url" placeholder="https://api.openai.com" @blur="normalizeUrl" />
                <span class="m-note">渠道根地址，末尾不要带 <code>/v1</code></span>
              </div>
            </div>
          </section>

          <!-- 3. API 密钥：渠道的"通行证"，列表限高，数量留在标题行 -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">API 密钥</span>
              <span class="m-sec-meta">{{ enabledKeyCount }}/{{ formValue.keys.length }} 启用</span>
              <NButton size="tiny" secondary class="m-sec-action" @click="addKey">+ 添加密钥</NButton>
            </div>
            <p class="m-sec-desc">
              至少需要一个已启用的密钥才能保存。本地中转站可直接查看与复制已保存的渠道 Key。
            </p>
            <div class="m-list">
              <div v-if="isWeightedStrategy" class="m-list-head">
                <span class="key-head-weight">权重</span>
                <span class="key-head-name">名称</span>
                <span class="key-head-value">API Key</span>
              </div>
              <div v-for="(key, index) in formValue.keys" :key="index" class="m-item">
                <NInputNumber
                  v-if="isWeightedStrategy"
                  v-model:value="key.weight"
                  :min="1"
                  :max="100"
                  :show-button="false"
                  style="width: 40px; flex: none"
                />
                <NInput
                  v-model:value="key.name"
                  placeholder="名称"
                  style="width: 110px; flex: none"
                />
                <NInput
                  v-model:value="key.value"
                  :type="keyVisibility[index] ? 'text' : 'password'"
                  :placeholder="selectedPreset?.api_key_placeholder || 'sk-...'"
                  style="flex: 1; min-width: 0"
                />
                <NButton quaternary size="small" @click="toggleKeyVisibility(index)">
                  {{ keyVisibility[index] ? "隐藏" : "显示" }}
                </NButton>
                <NButton quaternary size="small" @click="copyKeyValue(key.value)">复制</NButton>
                <div class="key-enabled">
                  <NSwitch v-model:value="key.enabled" size="small" />
                </div>
                <NButton quaternary circle type="error" @click="removeKey(index)">×</NButton>
              </div>
              <div v-if="formValue.keys.length === 0" class="m-empty">
                还没有密钥，点击右上角「添加密钥」
              </div>
            </div>
          </section>

          <!-- 4. 模型列表：数量常驻标题行，操作分两组（同步 / 选择） -->
          <section class="m-section">
            <div class="m-sec-head">
              <span class="m-sec-title">模型列表</span>
              <span class="m-sec-meta">{{ formValue.models.length }} 个模型</span>
              <NButton
                size="tiny"
                secondary
                class="m-sec-action"
                :loading="fetchingModels"
                :disabled="!canFetchModels"
                @click="fetchModels"
              >
                获取模型
              </NButton>
              <NInput
                v-if="formValue.models.length > 0"
                v-model:value="modelSearch"
                placeholder="搜索模型"
                size="small"
                clearable
                class="m-sec-search"
              />
            </div>
            <p class="m-sec-desc">
              使用第一个已启用且非空的 Key 请求 <code>/v1/models</code>，成功会覆盖下方列表。
            </p>

            <div v-if="formValue.models.length > 0">
              <div v-if="filteredModels.length > 0" class="model-list">
                <label
                  v-for="model in filteredModels"
                  :key="model"
                  class="model-check-row"
                  @click="toggleModelSelected(model)"
                >
                  <NCheckbox
                    :checked="selectedModels.includes(model)"
                    size="small"
                    @click.stop
                    @update:checked="(checked) => setModelSelected(model, checked)"
                  />
                  <span class="model-check-name">{{ model }}</span>
                </label>
              </div>
              <div v-else class="m-empty">无匹配模型</div>
            </div>
            <div v-else class="m-empty">暂无模型，可点击「获取模型」同步，或在下方手动添加</div>

            <div class="m-inline">
              <NInput
                v-model:value="newModel"
                placeholder="手动输入模型 ID，回车添加"
                size="small"
                clearable
                :disabled="fetchingModels"
                @keyup.enter="addModel"
              />
              <NButton size="small" secondary :disabled="!newModel.trim()" @click="addModel">添加</NButton>
              <div v-if="formValue.models.length > 0" class="m-inline-end">
                <NButton size="tiny" quaternary @click="selectAllModels">全选</NButton>
                <NButton size="tiny" quaternary @click="invertModels">反选</NButton>
                <NButton size="tiny" quaternary type="error" :disabled="selectedModels.length === 0" @click="deleteSelectedModels">
                  删除选中{{ selectedModels.length > 0 ? `（${selectedModels.length}）` : "" }}
                </NButton>
              </div>
            </div>
          </section>

          <!-- 5. 高级设置：低频且高风险，折叠起来但用徽标提示"是否改过" -->
          <ModalAdvanced :count="advancedChangedCount" hint="代理 · 超时 · 重试 · 请求头">
            <div class="m-grid" style="--m-cols: 3">
              <div class="m-field">
                <label class="m-label">代理地址</label>
                <NInput v-model:value="formValue.proxy_url" placeholder="可选" />
              </div>
              <div class="m-field">
                <label class="m-label">超时（秒）</label>
                <NInputNumber v-model:value="formValue.timeout_seconds" :min="1" :max="300" style="width: 100%" />
              </div>
              <div class="m-field">
                <label class="m-label">最大重试</label>
                <NInputNumber v-model:value="formValue.max_retries" :min="0" :max="10" style="width: 100%" />
              </div>
            </div>

            <div class="m-field adv-headers">
              <label class="m-label">
                自定义请求头
                <span class="m-note">覆盖同名的适配器头与转发的客户端头</span>
              </label>
              <div class="m-list">
                <div v-for="(header, index) in formValue.custom_headers" :key="index" class="m-item m-item-soft">
                  <NInput v-model:value="header.name" placeholder="Header 名称" style="flex: 1; min-width: 0" />
                  <NInput v-model:value="header.value" placeholder="Header 值" style="flex: 2; min-width: 0" />
                  <div class="header-enabled">
                    <NSwitch v-model:value="header.enabled" size="small" />
                  </div>
                  <NButton quaternary circle type="error" @click="removeHeader(index)">×</NButton>
                </div>
              </div>
              <div style="margin-top: 8px">
                <NButton size="small" secondary @click="addHeader">+ 添加请求头</NButton>
              </div>
            </div>
          </ModalAdvanced>
        </div>
      </AppFormModal>
    </template>
  </AppPageShell>
</template>

<style scoped>
/* 卡片 / 页头 / 徽标 / 统计 / 空态全部走 style.css 的 p-* / s-* 规范；
   这里只保留本页特有的「渠道卡内部」与「弹窗字段」造型。 */

.s-card {
  transition: border-color var(--transition);
}

.s-card:not(.disabled):hover {
  border-color: color-mix(in srgb, var(--accent) 34%, var(--border));
}

.s-card.disabled {
  opacity: 0.6;
}

.pc-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-2);
  margin-bottom: var(--sp-2);
}

.pc-title {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  flex-wrap: wrap;
}

.pc-name {
  font-size: var(--fs-md);
  font-weight: 600;
  color: var(--fg);
  letter-spacing: -0.01em;
}

.pc-meta,
.pc-proto {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
  margin-bottom: var(--sp-2);
}

.pc-models {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
  margin-bottom: var(--sp-3);
}

.pc-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-1);
  border-top: 1px solid var(--border);
  padding-top: var(--sp-2);
  margin-top: var(--sp-1);
}

.provider-grid :deep(.n-grid-item) {
  min-width: 0;
}

/* 高级设置里的请求头区块：与上方网格拉开距离 */
.adv-headers {
  margin-top: var(--sp-3);
}

.key-head-weight {
  flex: none;
  width: 40px;
}

.key-head-name {
  flex: none;
  width: 110px;
}

.key-head-value {
  flex: 1;
  min-width: 0;
}

/* 密钥 / 请求头条目右侧的开关：只留控件，文字标签交给分区说明 */
.key-enabled,
.header-enabled {
  display: flex;
  align-items: center;
  flex: none;
  gap: var(--sp-2);
  color: var(--muted);
  white-space: nowrap;
  font-size: var(--fs-sm);
}

/* 模型勾选网格：限高滚动，弹窗高度不随模型数量增长 */
.model-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 2px var(--sp-3);
  max-height: 200px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-right: var(--sp-1);
}

.model-check-row {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  padding: var(--sp-1) var(--sp-2);
  border-radius: var(--radius);
  cursor: pointer;
}

.model-check-row:hover {
  background: var(--hover-bg);
}

.model-check-name {
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  color: var(--fg);
  word-break: break-all;
}

/* 官方模板：选模板 + 去申请 Key 一行解决 */
.preset-row {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.preset-row :deep(.n-select) {
  flex: 1;
  min-width: 0;
}

.preset-row :deep(.n-button) {
  flex: none;
}
</style>
