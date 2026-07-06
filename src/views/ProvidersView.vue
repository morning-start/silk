<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  NButton,
  NCard,
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
import { api, type Provider } from "../api";
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
const testingStates = ref<Record<string, boolean>>({});
const keyVisibility = ref<boolean[]>([]);

const protocolOptions = [
  { label: "OpenAI Chat", value: "openai_chat" },
  { label: "Claude Messages", value: "claude_messages" },
  { label: "OpenAI Response", value: "openai_response" },
];

const keyStrategyOptions = [
  { label: "轮询", value: "round_robin" },
  { label: "加权轮询", value: "weighted" },
  { label: "顺序故障转移", value: "failover" },
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
  key_strategy: "round_robin",
  keys: [] as ProviderKeyForm[],
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

function createDefaultKey(name = "默认"): ProviderKeyForm {
  return { name, value: "", enabled: true, weight: 1 };
}

function resetForm() {
  editingId.value = null;
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
    key_strategy: "round_robin",
    keys: [createDefaultKey()],
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
  };
  showModal.value = true;
}

function addKey() {
  formValue.value.keys.push(createDefaultKey(""));
  keyVisibility.value.push(false);
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
  try {
    await navigator.clipboard.writeText(value);
    message.success("Key 已复制到剪贴板");
  } catch {
    message.error("复制失败");
  }
}

function normalizeUrl() {
  const url = formValue.value.api_base_url.trim();
  formValue.value.api_base_url = url.replace(/\/v1\/?$/, "").replace(/\/+$/, "");
}

function healthTagType(status: string | null): "success" | "error" | "warning" | "default" {
  return status === "healthy"
    ? "success"
    : status === "unhealthy"
      ? "error"
      : "warning";
}

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
    const models = await api.fetchProviderModels({
      api_base_url: formValue.value.api_base_url,
      api_key: apiKey,
      proxy_url: formValue.value.proxy_url || undefined,
      timeout_seconds: formValue.value.timeout_seconds,
    });
    formValue.value.models = models.map((model) => model.id);
    message.success(models.length > 0 ? `获取到 ${models.length} 个模型` : "未返回模型列表");
  } catch (e: any) {
    message.error(e?.message || "获取模型列表失败");
  } finally {
    fetchingModels.value = false;
  }
}

async function handleTest(row: Provider) {
  testingStates.value[row.id] = true;
  try {
    const result = await api.testProvider(row.id);
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
  } catch {
    // error handled by store
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
              <NTag size="tiny" :type="healthTagType(item.health_status)">
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

          <NFormItem label="API Keys" required>
            <div class="key-list">
              <div v-for="(key, index) in formValue.keys" :key="index" class="key-row">
                <NInput v-model:value="key.name" placeholder="名称" style="width: 120px" />
                <NInput
                  v-model:value="key.value"
                  :type="keyVisibility[index] ? 'text' : 'password'"
                  placeholder="sk-..."
                />
                <NButton quaternary size="small" @click="toggleKeyVisibility(index)">
                  {{ keyVisibility[index] ? "隐藏" : "显示" }}
                </NButton>
                <NButton quaternary size="small" @click="copyKeyValue(key.value)">
                  复制
                </NButton>
                <NInputNumber
                  v-if="formValue.key_strategy === 'weighted'"
                  v-model:value="key.weight"
                  :min="1"
                  :max="100"
                  style="width: 100px"
                  placeholder="权重"
                />
                <div class="key-enabled">
                  <span>启用</span>
                  <NSwitch v-model:value="key.enabled" size="small" />
                </div>
                <NButton quaternary circle type="error" @click="removeKey(index)">×</NButton>
              </div>
              <div class="key-actions">
                <NButton size="small" secondary @click="addKey">+ 添加密钥</NButton>
                <span class="key-hint">本地个人中转站可直接查看和复制已保存的渠道 Key。</span>
              </div>
            </div>
          </NFormItem>

          <NFormItem label="模型列表">
            <div class="models-block">
              <div class="models-actions">
                <NButton size="small" secondary :loading="fetchingModels" :disabled="!canFetchModels" @click="fetchModels">
                  获取模型
                </NButton>
                <span class="models-hint">使用第一个已启用且非空的 Key 请求 `/v1/models`。</span>
              </div>
              <div v-if="formValue.models.length > 0" class="model-list">
                <span
                  v-for="(model, index) in formValue.models"
                  :key="model + '-' + index"
                  class="model-pill"
                  @click="formValue.models.splice(index, 1)"
                >
                  {{ model }} ×
                </span>
              </div>
              <div v-else class="models-empty">暂未获取模型列表</div>
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

.key-enabled {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-color-2, #64748b);
  white-space: nowrap;
}

.key-actions {
  display: flex;
  justify-content: flex-start;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.key-hint {
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
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
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

@media (max-width: 900px) {
  .form-row,
  .key-row {
    flex-direction: column;
    align-items: stretch;
  }

  .key-enabled {
    justify-content: space-between;
  }
}
</style>
