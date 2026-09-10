<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NButton,
  NCard,
  NCheckbox,
  NForm,
  NFormItem,
  NGrid,
  NGi,
  NIcon,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import { SearchOutline } from "@vicons/ionicons5";
import { storeToRefs } from "pinia";
import { providersApi } from "../api/providers";
import type { Provider, ProviderHeaderEntry } from "../api";
import { copyWithFeedback } from "../utils/clipboard";
import { healthStatusType } from "../utils/health";
import AppFormModal from "../components/AppFormModal.vue";
import AppPageShell from "../components/AppPageShell.vue";
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
const dialog = useDialog();

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
  keyVisibility.value = (row.keys && row.keys.length > 0)
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
  dialog.warning({
    title: "确认删除",
    content: `确定要删除渠道 "${row.name}" 吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await providersStore.remove(row.id);
        message.success("删除成功");
      } catch {
        message.error("删除失败");
      }
    },
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
});
</script>

<template>
  <AppPageShell
    title="渠道管理"
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
      <div v-if="searchQuery.trim()" class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
        </div>
        <h3 class="empty-title">未找到匹配的渠道</h3>
        <p class="empty-desc">换个关键词，或者直接新增一个渠道。</p>
      </div>
      <div v-else class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
        </div>
        <h3 class="empty-title">暂无渠道</h3>
        <p class="empty-desc">添加第一个 AI 渠道，开始配置您的 API 网关。</p>
        <NButton type="primary" @click="handleAdd">+ 新增渠道</NButton>
      </div>
    </template>

    <NGrid :x-gap="16" :y-gap="16" :cols="3" style="margin-top: 16px">
      <NGi v-for="item in filteredProviders" :key="item.id">
        <NCard :bordered="false" class="provider-card" :class="{ disabled: item.status !== 'enabled' }">
          <div class="pc-header">
            <div class="pc-title">
              <span class="pc-name">{{ item.name }}</span>
              <NTag size="tiny" :type="healthStatusType(item.health_status)">
                {{ healthLabel(item) }}
              </NTag>
            </div>
            <span class="pc-protocol-count">{{ item.protocols?.length || 0 }} 协议</span>
          </div>

          <div class="pc-url">{{ item.api_base_url.replace(/^https?:\/\//, "") }}</div>

          <div class="pc-tags">
            <NTag size="small" type="info">{{ keySummary(item) }}</NTag>
            <NTag size="small" type="success" v-if="item.models?.length">{{ item.models.length }} 模型</NTag>
            <NTag size="small" type="default">超时 {{ item.timeout_seconds }}s</NTag>
            <NTag size="small" :type="item.models_passthrough ? 'warning' : 'default'">
              {{ item.models_passthrough ? '穿透' : '不穿透' }}
            </NTag>
          </div>

          <div class="pc-protocols" v-if="item.protocols?.length">
            <NTag v-for="protocol in item.protocols" :key="protocol" size="tiny" round>{{ protocol }}</NTag>
          </div>

          <div class="pc-models" v-if="item.models?.length">
            <span v-for="model in item.models.slice(0, 4)" :key="model" class="pc-model-pill">{{ model }}</span>
            <NTag v-if="item.models.length > 4" size="tiny" round>+{{ item.models.length - 4 }}</NTag>
          </div>

          <div class="pc-actions">
            <NButton size="tiny" quaternary @click="handleEdit(item)">编辑</NButton>
            <NButton size="tiny" quaternary :loading="testingStates[item.id]" @click="handleTest(item)">测试</NButton>
            <NButton size="tiny" quaternary type="error" @click="handleDelete(item)">删除</NButton>
          </div>
        </NCard>
      </NGi>
    </NGrid>

    <template #after>
      <AppFormModal
        v-model:show="showModal"
        :title="editingId ? '编辑渠道' : '新增渠道'"
        width="760px"
        :submit-text="editingId ? '保存修改' : '确认添加'"
        :submit-disabled="!canSubmit"
        @cancel="closeModal"
        @submit="handleSubmit"
      >
        <NForm :model="formValue" label-placement="left" label-width="92">
          <div class="form-row">
            <NFormItem label="名称" required style="flex: 1">
              <NInput v-model:value="formValue.name" placeholder="如：OpenAI 官方" />
            </NFormItem>
            <NFormItem label="状态" style="flex: 0 0 140px">
              <NSwitch
                :value="formValue.status === 'enabled'"
                @update:value="(value: boolean) => { formValue.status = value ? 'enabled' : 'disabled'; }"
              />
            </NFormItem>
            <NFormItem label="模型穿透" style="flex: 0 0 auto">
              <NSwitch v-model:value="formValue.models_passthrough" />
              <span class="form-hint">显示在 /v1/models</span>
            </NFormItem>
          </div>

          <NFormItem label="接口协议" required>
            <NSelect
              v-model:value="formValue.protocols"
              multiple
              filterable
              :options="protocolOptions"
              placeholder="选择协议，可多选"
            />
          </NFormItem>

          <NFormItem label="API 地址" required>
            <NInput v-model:value="formValue.api_base_url" placeholder="https://api.openai.com" @blur="normalizeUrl" />
          </NFormItem>

          <div class="form-row">
            <NFormItem label="密钥策略" style="flex: 1">
              <NSelect v-model:value="formValue.key_strategy" :options="keyStrategyOptions" />
            </NFormItem>
            <NFormItem label="代理地址" style="flex: 1">
              <NInput v-model:value="formValue.proxy_url" placeholder="可选" />
            </NFormItem>
          </div>

          <NFormItem label-placement="top" label-style="width: 100%">
            <template #label>
              <div class="key-title">
                <span class="key-title-name">API Keys <span class="key-title-required">*</span></span>
                <NButton size="small" secondary @click="addKey">+ 添加密钥</NButton>
                <span class="key-hint">本地个人中转站可直接查看和复制已保存的渠道 Key。</span>
              </div>
            </template>
            <div class="key-list">
              <div v-if="isWeightedStrategy" class="key-row key-row-head">
                <span class="key-head-weight">权重</span>
                <span class="key-head-name">名称</span>
                <span class="key-head-value">API Key</span>
              </div>
              <div v-for="(key, index) in formValue.keys" :key="index" class="key-row">
                <NInputNumber
                  v-if="isWeightedStrategy"
                  v-model:value="key.weight"
                  :min="1"
                  :max="100"
                  :show-button="false"
                  style="width: 34px; flex: none"
                  placeholder="权重"
                />
                <NInput
                  v-model:value="key.name"
                  placeholder="名称"
                  style="width: 120px; flex: none"
                />
                <NInput
                  v-model:value="key.value"
                  :type="keyVisibility[index] ? 'text' : 'password'"
                  placeholder="sk-..."
                  style="flex: 1; min-width: 0"
                />
                <NButton quaternary size="small" @click="toggleKeyVisibility(index)">
                  {{ keyVisibility[index] ? "隐藏" : "显示" }}
                </NButton>
                <NButton quaternary size="small" @click="copyKeyValue(key.value)">
                  复制
                </NButton>
                <div class="key-enabled">
                  <span>启用</span>
                  <NSwitch v-model:value="key.enabled" size="small" />
                </div>
                <NButton quaternary circle type="error" @click="removeKey(index)">×</NButton>
              </div>
            </div>
          </NFormItem>

          <NFormItem label="自定义请求头">
            <div class="header-list">
              <div v-for="(header, index) in formValue.custom_headers" :key="index" class="header-row">
                <NInput v-model:value="header.name" placeholder="Header 名称" style="flex: 1" />
                <NInput v-model:value="header.value" placeholder="Header 值" style="flex: 2" />
                <div class="header-enabled">
                  <span>启用</span>
                  <NSwitch v-model:value="header.enabled" size="small" />
                </div>
                <NButton quaternary circle type="error" @click="removeHeader(index)">×</NButton>
              </div>
              <div class="header-actions">
                <NButton size="small" secondary @click="addHeader">+ 添加请求头</NButton>
                <span class="header-hint">自定义请求头会覆盖同名的适配器头和转发的客户端头。</span>
              </div>
            </div>
          </NFormItem>

          <NFormItem label="模型列表">
            <div class="models-block">
              <div class="models-actions">
                <NButton size="small" secondary :loading="fetchingModels" :disabled="!canFetchModels" @click="fetchModels">
                  获取模型
                </NButton>
                <span class="models-hint">使用第一个已启用且非空的 Key 请求 `/v1/models`，成功会覆盖下方列表。</span>
                <template v-if="formValue.models.length > 0">
                  <NInput
                    v-model:value="modelSearch"
                    placeholder="搜索模型"
                    size="small"
                    clearable
                    class="model-search"
                  />
                  <div class="models-select-actions">
                    <NButton size="tiny" quaternary @click="selectAllModels">全选</NButton>
                    <NButton size="tiny" quaternary @click="invertModels">反选</NButton>
                    <NButton size="tiny" quaternary type="error" :disabled="selectedModels.length === 0" @click="deleteSelectedModels">
                      删除选中{{ selectedModels.length > 0 ? `（${selectedModels.length}）` : '' }}
                    </NButton>
                  </div>
                </template>
              </div>
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
                <div v-else class="models-empty">无匹配模型</div>
              </div>
              <div v-else class="models-empty">暂无模型，可手动添加或点击「获取模型」</div>
              <div class="model-add-row">
                <NInput
                  v-model:value="newModel"
                  placeholder="手动输入模型 ID，回车添加"
                  size="small"
                  clearable
                  :disabled="fetchingModels"
                  @keyup.enter="addModel"
                />
                <NButton size="small" secondary :disabled="!newModel.trim()" @click="addModel">添加</NButton>
              </div>
            </div>
          </NFormItem>

          <div class="form-row">
            <NFormItem label="超时（秒）" style="flex: 1">
              <NInputNumber v-model:value="formValue.timeout_seconds" :min="1" :max="300" style="width: 100%" />
            </NFormItem>
            <NFormItem label="最大重试" style="flex: 1">
              <NInputNumber v-model:value="formValue.max_retries" :min="0" :max="10" style="width: 100%" />
            </NFormItem>
          </div>
        </NForm>
      </AppFormModal>
    </template>
  </AppPageShell>
</template>

<style scoped>
.provider-card {
  border-radius: 12px;
  transition: box-shadow 0.2s;
}

.provider-card:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
}

.provider-card.disabled {
  opacity: 0.72;
}

.pc-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.pc-title {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.pc-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--fg, #0f172a);
}

.pc-protocol-count {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  white-space: nowrap;
}

.pc-url {
  font-family: "JetBrains Mono", ui-monospace, monospace;
  font-size: 12px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pc-tags,
.pc-protocols,
.pc-models {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.pc-tags {
  margin-bottom: 10px;
}

.pc-protocols {
  margin-bottom: 10px;
}

.pc-models {
  margin-bottom: 12px;
}

.pc-model-pill {
  display: inline-flex;
  align-items: center;
  max-width: 180px;
  padding: 2px 8px;
  border-radius: 999px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  color: #475569;
  font-size: 11px;
  font-family: "JetBrains Mono", ui-monospace, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pc-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  padding-top: 10px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.key-list {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.key-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.key-row-head {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  padding-bottom: 2px;
}

.key-head-weight {
  flex: none;
  width: 34px;
}

.key-head-name {
  flex: none;
  width: 120px;
}

.key-head-value {
  flex: 1;
  min-width: 0;
}

.key-enabled {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-color-2, #64748b);
  white-space: nowrap;
}

.key-title {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  flex-wrap: wrap;
}

.key-title-name {
  font-weight: 500;
}

.key-title-required {
  color: var(--error-color, #d03050);
  margin-left: 2px;
}

.key-hint {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.header-list {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.header-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.header-enabled {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-color-2, #64748b);
  white-space: nowrap;
}

.header-actions {
  display: flex;
  justify-content: flex-start;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.header-hint {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.models-block {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.models-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.models-hint {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.model-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 2px 12px;
  margin-top: 10px;
  max-height: 260px;
  overflow-y: auto;
}

.model-check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 4px 8px;
  border-radius: var(--radius, 8px);
  cursor: pointer;
}

.model-check-row:hover {
  background: var(--hover-bg, #f8fafc);
}

.model-check-name {
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  font-size: 12px;
  color: var(--fg, #0f172a);
  word-break: break-all;
}

.model-search {
  width: 160px;
  margin-left: auto;
}

.models-select-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.model-pill {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(8, 145, 178, 0.08);
  color: var(--accent, #0891b2);
  font-size: 12px;
  font-family: "JetBrains Mono", ui-monospace, monospace;
  cursor: pointer;
}

.models-empty {
  font-size: 13px;
  color: var(--text-color-3, #94a3b8);
}

.model-add-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}

.model-add-row .n-input {
  flex: 1;
  min-width: 0;
}

.form-hint {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  margin-left: 8px;
  white-space: nowrap;
}

@media (max-width: 900px) {
  .form-row,
  .key-row,
  .header-row {
    flex-direction: column;
    align-items: stretch;
  }

  .key-row-head {
    display: none;
  }

  .key-enabled,
  .header-enabled {
    justify-content: space-between;
  }
}
</style>
