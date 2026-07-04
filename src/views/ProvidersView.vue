<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useMessage, useDialog } from "naive-ui";
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

// Template-used functions (vue-tsc tracking)
void [addKey, removeKey, fetchModels, handleSubmit];
</script>

<template>
  <div class="providers-page">
    <!-- Toolbar -->
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">渠道管理</h2>
        <span class="badge badge-accent">{{ providers.length }} 个渠道</span>
      </div>
      <div class="toolbar-right">
        <div class="search-box">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
          <input class="input" placeholder="搜索渠道名称..." v-model="searchQuery">
        </div>
        <button class="btn btn-primary" @click="handleAdd">+ 新增渠道</button>
      </div>
    </div>

    <div class="page-content">
      <div v-if="error" class="error-state">
        <div class="error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:var(--danger)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h3 class="error-title">数据加载失败</h3>
        <p class="error-desc">{{ error }}</p>
        <button class="btn btn-primary" @click="providersStore.fetchAll()">重新加载</button>
      </div>

      <div v-else-if="!loading && filteredProviders.length === 0" class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:var(--muted)"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
        </div>
        <h3 class="empty-title" v-if="searchQuery">未找到匹配的渠道</h3>
        <h3 class="empty-title" v-else>暂无渠道</h3>
        <p class="empty-desc" v-if="!searchQuery">添加第一个 AI 渠道，开始配置您的 API 网关</p>
        <button v-if="!searchQuery" class="btn btn-primary" @click="handleAdd">+ 新增渠道</button>
      </div>

      <div v-else class="provider-grid">
        <div v-for="item in filteredProviders" :key="item.id" class="provider-card">
          <div>
            <div class="pc-header">
            <span class="pc-name">{{ item.name }}</span>
            <div class="pc-header-right">
              <span class="status-dot-sm" :class="item.status === 'enabled' ? 'online' : 'offline'"></span>
              <span class="pc-status-text" :class="item.status === 'enabled' ? 'text-success' : ''">
                {{ item.status === 'enabled' ? (item.health_status === 'healthy' ? '正常' : '异常') : '禁用' }}
              </span>
            </div>
          </div>
          <div class="pc-url">{{ item.api_base_url.replace(/^https?:\/\//, '') }}</div>
          <div class="pc-meta">
            <span class="pc-keys">{{ item.key_count || 0 }} Keys</span>
            <span class="badge badge-success" v-if="item.models?.length">{{ item.models.length }} 模型</span>
            <span class="badge badge-neutral">超时 {{ item.timeout_seconds }}s</span>
          </div>
        </div>
        <div class="pc-actions">
          <button class="btn btn-ghost btn-sm" @click="handleEdit(item)">编辑</button>
          <button class="btn btn-ghost btn-sm" :disabled="testingStates[item.id]" @click="handleTest(item)">{{ testingStates[item.id] ? '测试中...' : '测试' }}</button>
          <button class="btn btn-ghost btn-sm" style="color:var(--danger)" @click="handleDelete(item)">删除</button>
        </div>
        </div>
      </div>
    </div>

    <!-- Modal -->
    <div v-if="showModal" class="modal-overlay" @click.self="showModal = false">
      <div class="modal">
        <div class="modal-header">
          <h3>{{ editingId ? '编辑渠道' : '新增渠道' }}</h3>
          <button class="btn-icon" @click="showModal = false">×</button>
        </div>
        <div class="modal-body form-stack">
          <div class="field">
            <label>名称</label>
            <input class="input" v-model="formValue.name" placeholder="如：OpenAI 官方" />
          </div>
          <div class="field">
            <label>接口协议</label>
            <select class="input" v-model="formValue.protocols" multiple>
              <option value="openai_chat">OpenAI Chat</option>
              <option value="claude_messages">Claude Messages</option>
              <option value="openai_response">OpenAI Response</option>
            </select>
          </div>
          <div class="field">
            <label>API 地址</label>
            <input class="input" v-model="formValue.api_base_url" placeholder="https://api.openai.com" @blur="normalizeUrl" />
          </div>
          <div class="field">
            <label>密钥策略</label>
            <select class="input" v-model="formValue.key_strategy" style="width:200px">
              <option value="round_robin">轮询</option>
              <option value="weighted">加权轮询</option>
              <option value="failover">顺序故障转移</option>
            </select>
          </div>
          <div class="field">
            <label>密钥</label>
            <div style="display:flex;flex-direction:column;gap:8px;width:100%">
              <div v-for="(k, i) in formValue.keys" :key="i" class="pc-key-row">
                <input class="input" v-model="k.name" placeholder="名称" style="width:80px;flex-shrink:0" />
                <input class="input" v-model="k.value" type="password" placeholder="sk-..." style="flex:1;font-family:var(--font-mono)" />
                <input v-if="formValue.key_strategy === 'weighted'" class="input" v-model.number="k.weight" type="number" min="1" max="100" style="width:70px" placeholder="权重" />
                <label class="toggle" :class="{ on: k.enabled }" @click="k.enabled = !k.enabled"></label>
                <button class="btn-icon" @click="removeKey(i)">×</button>
              </div>
              <button class="btn btn-ghost btn-sm" @click="addKey" style="align-self:flex-start">+ 添加密钥</button>
            </div>
          </div>
          <div class="field">
            <label>模型</label>
            <div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center">
              <span v-for="(m, i) in formValue.models" :key="i" class="badge badge-neutral" style="cursor:pointer" @click="formValue.models.splice(i, 1)">{{ m }} ×</span>
              <button class="btn btn-ghost btn-sm" @click="fetchModels" :disabled="!formValue.api_base_url || formValue.keys.length === 0">{{ fetchingModels ? '获取中...' : '获取模型' }}</button>
            </div>
          </div>
          <div class="field">
            <label>代理地址</label>
            <input class="input" v-model="formValue.proxy_url" placeholder="可选" />
          </div>
          <div class="form-row">
            <div class="field">
              <label>超时（秒）</label>
              <input class="input" v-model.number="formValue.timeout_seconds" type="number" min="5" max="300" />
            </div>
            <div class="field">
              <label>最大重试</label>
              <input class="input" v-model.number="formValue.max_retries" type="number" min="0" max="10" />
            </div>
          </div>
          <div class="field">
            <div class="row-between">
              <label>启用</label>
              <label class="toggle" :class="{ on: formValue.status === 'enabled' }" @click="formValue.status = formValue.status === 'enabled' ? 'disabled' : 'enabled'"></label>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showModal = false">取消</button>
          <button class="btn btn-primary" @click="handleSubmit">{{ editingId ? '保存修改' : '确认添加' }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.providers-page {
  width: 100%;
}

.provider-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
}

.provider-card {
  background: var(--surface, #ffffff);
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius-lg, 12px);
  padding: 20px;
  box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.05));
  transition: all 150ms ease;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.provider-card:hover {
  border-color: var(--accent, #0891b2);
  box-shadow: var(--shadow, 0 4px 6px -1px rgba(0,0,0,0.1));
}

.pc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.pc-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--fg, #0f172a);
}

.pc-header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.pc-status-text {
  font-size: 12px;
  color: var(--muted, #64748b);
}

.pc-status-text.text-success {
  color: var(--success, #10b981);
}

.pc-url {
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  font-size: 12px;
  color: var(--muted, #64748b);
  margin-bottom: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pc-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 14px;
}

.pc-keys {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  background: var(--accent-soft, rgba(8,145,178,0.08));
  color: var(--accent, #0891b2);
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
}

.pc-actions {
  display: flex;
  gap: 6px;
  border-top: 1px solid var(--border-soft, #e2e8f0);
  padding-top: 12px;
  margin-top: auto;
}
</style>
