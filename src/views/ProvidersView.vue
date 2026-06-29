<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import {
  NButton,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  NIcon,
  useMessage,
  useDialog,
} from "naive-ui";
import {
  SearchOutline,
} from "@vicons/ionicons5";
import { useProvidersStore } from "../stores/providers";
import { storeToRefs } from "pinia";
import type { Provider } from "../api";
import { api } from "../api";

const providersStore = useProvidersStore();
const { providers, loading, error } = storeToRefs(providersStore);
const message = useMessage();
const dialog = useDialog();

const searchQuery = ref("");

const filteredProviders = computed(() => {
  const q = searchQuery.value.toLowerCase().trim();
  if (!q) return providers.value;
  return providers.value.filter(
    (p) =>
      p.name.toLowerCase().includes(q) ||
      p.api_base_url.toLowerCase().includes(q) ||
      p.protocols?.some((pr: string) => pr.toLowerCase().includes(q))
  );
});

const showModal = ref(false);
const editingId = ref<string | null>(null);

const formValue = ref({
  name: "",
  protocols: [] as string[],
  models: [] as string[],
  api_base_url: "",
  proxy_url: "",
  timeout_seconds: 30,
  max_retries: 3,
  status: "enabled" as string,
  key_strategy: "round_robin",
  keys: [] as { name: string; value: string; enabled: boolean; weight: number }[],
});

// 模型获取
const fetchingModels = ref(false);

const protocolOptions = [
  { label: "OpenAI Chat", value: "openai_chat" },
  { label: "Claude Messages", value: "claude_messages" },
  { label: "OpenAI Response", value: "openai_response" },
];

function handleAdd() {
  editingId.value = null;
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
    keys: [{ name: "默认", value: "", enabled: true, weight: 1 }],
  };
  showModal.value = true;
}

function addKey() {
  formValue.value.keys.push({ name: "", value: "", enabled: true, weight: 1 });
}

function removeKey(index: number) {
  formValue.value.keys.splice(index, 1);
}

async function fetchModels() {
  // 确定要使用的 API Key：取第一个启用的密钥
  let apiKey = formValue.value.keys.find((k) => k.enabled && k.value)?.value || "";
  if (!formValue.value.api_base_url || !apiKey) {
    message.warning("请先填写 API 地址和 API Key");
    return;
  }
  // 发送前确保 URL 已清洗
  normalizeUrl();
  fetchingModels.value = true;
  try {
    const models = await api.fetchProviderModels({
      api_base_url: formValue.value.api_base_url,
      api_key: apiKey,
      proxy_url: formValue.value.proxy_url || undefined,
      timeout_seconds: formValue.value.timeout_seconds,
    });
    formValue.value.models = models.map((m) => m.id);
    if (models.length > 0) {
      message.success(`获取到 ${models.length} 个模型`);
    }
  } catch (e: any) {
    message.error(e.message || "获取模型列表失败");
  } finally {
    fetchingModels.value = false;
  }
}

/** 自动清洗 API 地址：去除尾部 /v1 或 /v1/ */
function normalizeUrl() {
  const url = formValue.value.api_base_url.trim();
  formValue.value.api_base_url = url
    .replace(/\/v1\/?$/, '')
    .replace(/\/+$/, '');
}

function handleEdit(row: Provider) {
  editingId.value = row.id;
  formValue.value = {
    name: row.name,
    protocols: row.protocols || [],
    models: row.models || [],
    api_base_url: row.api_base_url,
    proxy_url: row.proxy_url || "",
    timeout_seconds: row.timeout_seconds,
    max_retries: row.max_retries,
    status: row.status,
    key_strategy: "round_robin",
    keys: (row.keys && row.keys.length > 0)
      ? row.keys.map((k) => ({ name: k.name, value: k.value, enabled: k.enabled, weight: k.weight }))
      : [{ name: "默认", value: "", enabled: true, weight: 1 }],
  };
  showModal.value = true;
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

const testingStates = ref<Record<string, boolean>>({});

async function handleTest(row: Provider) {
  testingStates.value[row.id] = true;
  try {
    const result = await api.testProvider(row.id);
    if (result.health_status === "healthy") {
      message.success(`连接成功 · ${result.response_time_ms}ms`);
    } else {
      message.error(`连接失败 · ${result.error || "未知错误"}`);
    }
  } catch (e: any) {
    message.error(e.message || "测试失败");
  } finally {
    testingStates.value[row.id] = false;
  }
}

async function handleSubmit() {
  try {
    if (editingId.value) {
      const data: any = { ...formValue.value };
      data.keys = data.keys || [];
      delete data.api_key;
      await providersStore.update(editingId.value, data);
      message.success("更新成功");
    } else {
      const data: any = { ...formValue.value };
      data.keys = data.keys || [];
      delete data.api_key;
      await providersStore.create(data);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch {
    // error handled by store
  }
}

onMounted(() => {
  providersStore.fetchAll();
});
</script>

<template>
  <div class="providers-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">渠道管理</h2>
        <NTag size="small" type="info">{{ providers.length }} 个渠道</NTag>
      </div>
      <div class="toolbar-right">
        <NInput
          v-model:value="searchQuery"
          placeholder="搜索渠道..."
          clearable
          style="width: 200px"
          size="small"
        >
          <template #prefix>
            <NIcon><SearchOutline /></NIcon>
          </template>
        </NInput>
        <NButton type="primary" @click="handleAdd">+ 新增渠道</NButton>
      </div>
    </div>

    <NCard :bordered="false" class="table-card" size="small">
      <!-- Error State -->
      <template v-if="error">
        <div class="error-state">
          <div class="error-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <h3 class="error-title">数据加载失败</h3>
          <p class="error-desc">{{ error }}</p>
          <NButton type="primary" @click="providersStore.fetchAll()">重新加载</NButton>
        </div>
      </template>

      <!-- Empty State -->
      <template v-else-if="!loading && filteredProviders.length === 0">
        <div class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
          </div>
          <h3 class="empty-title" v-if="searchQuery">未找到匹配的渠道</h3>
          <h3 class="empty-title" v-else>暂无渠道</h3>
          <p class="empty-desc" v-if="!searchQuery">添加第一个 AI 渠道，开始配置您的 API 网关</p>
          <NButton v-if="!searchQuery" type="primary" @click="handleAdd">+ 新增渠道</NButton>
        </div>
      </template>

      <template v-else>
        <div
          v-for="item in filteredProviders"
          :key="item.id"
          class="provider-card"
        >
          <div class="pc-header">
            <span class="pc-name">{{ item.name }}</span>
            <div class="pc-header-right">
              <span class="pc-status-dot" :class="item.status === 'enabled' ? 'online' : 'offline'"></span>
              <span class="pc-status-text" :class="item.status === 'enabled' ? 'text-success' : ''">
                {{ item.status === 'enabled' ? (item.health_status === 'healthy' ? '正常' : '异常') : '禁用' }}
              </span>
            </div>
          </div>

          <div class="pc-url">{{ item.api_base_url.replace(/^https?:\/\//, '') }}</div>

          <div class="pc-meta">
            <span class="pc-meta-item">
              🔑 {{ item.key_count || 0 }} key{{ item.key_count !== 1 ? 's' : '' }}
            </span>
            <span class="pc-meta-item" v-if="item.models?.length">
              {{ item.models.length }} 模型
            </span>
            <span class="pc-meta-item">超时 {{ item.timeout_seconds }}s</span>
          </div>

          <div class="pc-actions">
            <NButton size="tiny" quaternary @click="handleEdit(item)">编辑</NButton>
            <NButton size="tiny" quaternary :loading="testingStates[item.id]" @click="handleTest(item)">测试</NButton>
            <NButton size="tiny" quaternary type="error" @click="handleDelete(item)">删除</NButton>
          </div>
        </div>
      </template>
    </NCard>

    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editingId ? '编辑渠道' : '新增渠道'"
      style="max-width: 600px"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <NForm ref="formRef" :model="formValue" label-placement="left" label-width="100">
        <NFormItem label="名称" required>
          <NInput v-model:value="formValue.name" placeholder="如：OpenAI 官方" />
        </NFormItem>
        <NFormItem label="接口协议" required>
          <NSelect
            v-model:value="formValue.protocols"
            :options="protocolOptions"
            multiple
            placeholder="选择支持的接口协议"
          />
        </NFormItem>
        <NFormItem label="API 地址" required>
          <NInput v-model:value="formValue.api_base_url" placeholder="https://api.openai.com" @blur="normalizeUrl" />
        </NFormItem>

        <!-- API 密钥 -->
        <NFormItem label="密钥策略">
          <NSelect
            v-model:value="formValue.key_strategy"
            :options="[
              { label: '轮询', value: 'round_robin' },
              { label: '加权轮询', value: 'weighted' },
              { label: '顺序故障转移', value: 'failover' },
            ]"
            style="width: 200px"
          />
        </NFormItem>
        <NFormItem label="密钥">
          <div style="width: 100%">
            <div v-for="(k, i) in formValue.keys" :key="i" style="display:flex;gap:8px;margin-bottom:8px;align-items:center">
              <NInput v-model:value="k.name" placeholder="名称" size="small" style="flex:0 0 80px" />
              <NInput v-model:value="k.value" type="password" placeholder="sk-..." size="small" style="flex:1" show-password-on="click" />
              <NInputNumber v-if="formValue.key_strategy === 'weighted'" v-model:value="k.weight" :min="1" :max="100" size="small" style="width:70px" placeholder="权重" />
              <NSwitch v-model:value="k.enabled" size="small" />
              <NButton quaternary size="tiny" type="error" @click="removeKey(i)">×</NButton>
            </div>
            <NButton size="tiny" quaternary @click="addKey">+ 添加密钥</NButton>
          </div>
        </NFormItem>
        <NFormItem label="模型">
          <div style="display: flex; gap: 8px; width: 100%; flex-wrap: wrap; align-items: center">
            <template v-if="formValue.models.length > 0">
              <NTag
                v-for="(m, i) in formValue.models"
                :key="i"
                closable
                size="small"
                @close="formValue.models.splice(i, 1)"
              >
                {{ m }}
              </NTag>
            </template>
            <NButton
              secondary
              size="small"
              @click="fetchModels"
              :loading="fetchingModels"
              :disabled="!formValue.api_base_url || formValue.keys.length === 0"
            >
              获取模型
            </NButton>
          </div>
        </NFormItem>
        <NFormItem label="代理地址">
          <NInput v-model:value="formValue.proxy_url" placeholder="可选" />
        </NFormItem>
        <div class="form-row">
          <NFormItem label="超时(秒)" style="flex: 1">
            <NInputNumber v-model:value="formValue.timeout_seconds" :min="5" :max="300" style="width: 100%" />
          </NFormItem>
          <NFormItem label="最大重试" style="flex: 1">
            <NInputNumber v-model:value="formValue.max_retries" :min="0" :max="10" style="width: 100%" />
          </NFormItem>
        </div>
        <NFormItem label="启用">
          <NSwitch v-model:value="formValue.status" checked-value="enabled" unchecked-value="disabled" />
        </NFormItem>
      </NForm>
      <template #footer>
        <div style="display: flex; justify-content: flex-end; gap: 8px">
          <NButton @click="showModal = false">取消</NButton>
          <NButton type="primary" @click="handleSubmit">{{ editingId ? '保存修改' : '确认添加' }}</NButton>
        </div>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.providers-page {
  max-width: 1200px;
}

.providers-page > .provider-card {
  margin-bottom: 0;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar-right {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.providers-page {
  max-width: 1200px;
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
  align-content: start;
}

.providers-page > .toolbar {
  grid-column: 1 / -1;
}

.providers-page > .error-state,
.providers-page > .empty-state {
  grid-column: 1 / -1;
}

.provider-card {
  border-radius: 12px;
  background: var(--card-color, #ffffff);
  border: 1px solid var(--border-color, #e2e8f0);
  padding: 16px;
  transition: box-shadow 0.2s;
  cursor: pointer;
}

.provider-card:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
}

.pc-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.pc-header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.pc-name {
  font-size: 15px;
  font-weight: 600;
}

.pc-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #94a3b8;
}

.pc-status-dot.online {
  background: #22c55e;
  box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.2);
}

.pc-status-dot.offline {
  background: #94a3b8;
}

.pc-status-text {
  font-size: 13px;
  color: #94a3b8;
}

.pc-status-text.text-success {
  color: #22c55e;
}

.pc-url {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  margin-bottom: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pc-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 12px;
}

.pc-meta-item {
  font-size: 12px;
  color: var(--text-color-2, #64748b);
  background: var(--tag-bg, #f1f5f9);
  padding: 2px 8px;
  border-radius: 4px;
}

.pc-actions {
  display: flex;
  gap: 4px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  padding-top: 10px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.error-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  text-align: center;
}

.error-icon {
  margin-bottom: 16px;
}

.error-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-color, #1e293b);
  margin: 0 0 8px;
}

.error-desc {
  font-size: 13px;
  color: var(--text-color-3, #94a3b8);
  margin: 0 0 20px;
  max-width: 400px;
}
</style>
