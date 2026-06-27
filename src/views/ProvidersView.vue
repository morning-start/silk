<script setup lang="ts">
import { ref, computed, onMounted, h } from "vue";
import {
  NButton,
  NDataTable,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  NText,
  NCard,
  useMessage,
  useDialog,
  type DataTableColumns,
} from "naive-ui";
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
      p.provider_type.toLowerCase().includes(q)
  );
});

const showModal = ref(false);
const editingId = ref<string | null>(null);

const formValue = ref({
  name: "",
  provider_type: "openai",
  api_base_url: "",
  api_key: "",
  model_name: "",
  proxy_url: "",
  timeout_seconds: 30,
  max_retries: 3,
  status: "enabled" as string,
});

// 模型获取
const modelOptions = ref<{ label: string; value: string }[]>([]);
const fetchingModels = ref(false);

const typeOptions = [
  { label: "OpenAI", value: "openai" },
  { label: "Anthropic", value: "anthropic" },
  { label: "Azure", value: "azure" },
  { label: "自定义", value: "custom" },
];

const columns: DataTableColumns<Provider> = [
  { title: "名称", key: "name", width: 120 },
  {
    title: "类型",
    key: "provider_type",
    width: 100,
    render(row) {
      const colors: Record<string, string> = {
        openai: "blue",
        anthropic: "orange",
        azure: "azure",
        custom: "default",
      };
      return h(NTag, { size: "small", type: (colors[row.provider_type] || "default") as any }, {
        default: () => row.provider_type,
      });
    },
  },
  { title: "Base URL", key: "api_base_url", ellipsis: { tooltip: true } },
  {
    title: "状态",
    key: "status",
    width: 80,
    render(row) {
      const enabled = row.status === "enabled";
      return h(NTag, { size: "small", type: enabled ? "success" : "warning" }, {
        default: () => enabled ? "启用" : "禁用",
      });
    },
  },
  { title: "超时(s)", key: "timeout_seconds", width: 80 },
  { title: "重试", key: "max_retries", width: 70 },
  { title: "健康状态", key: "health_status", width: 100,
    render(row) {
      if (!row.health_status) return h(NText, { depth: 3 }, { default: () => "未检测" });
      const ok = row.health_status === "healthy";
      return h(NTag, { size: "small", type: ok ? "success" : "error" }, {
        default: () => ok ? "正常" : "异常",
      });
    },
  },
  {
    title: "操作",
    key: "actions",
    width: 160,
    render(row) {
      return [
        h(NButton, { size: "small", quaternary: true, onClick: () => handleEdit(row) }, { default: () => "编辑" }),
        " ",
        h(NButton, { size: "small", quaternary: true, type: "error" as any, onClick: () => handleDelete(row) }, { default: () => "删除" }),
      ];
    },
  },
];

function handleAdd() {
  editingId.value = null;
  formValue.value = {
    name: "",
    provider_type: "openai",
    api_base_url: "",
    api_key: "",
    model_name: "",
    proxy_url: "",
    timeout_seconds: 30,
    max_retries: 3,
    status: "enabled",
  };
  modelOptions.value = [];
  showModal.value = true;
}

async function fetchModels() {
  if (!formValue.value.api_base_url || !formValue.value.api_key) {
    message.warning("请先填写 API 地址和 API Key");
    return;
  }
  fetchingModels.value = true;
  modelOptions.value = [];
  try {
    const models = await api.fetchProviderModels({
      api_base_url: formValue.value.api_base_url,
      api_key: formValue.value.api_key,
      proxy_url: formValue.value.proxy_url || undefined,
      timeout_seconds: formValue.value.timeout_seconds,
    });
    modelOptions.value = models.map((m) => ({ label: m, value: m }));
    if (models.length > 0 && !formValue.value.model_name) {
      formValue.value.model_name = models[0];
    }
    message.success(`获取到 ${models.length} 个模型`);
  } catch (e: any) {
    message.error(e.message || "获取模型列表失败");
  } finally {
    fetchingModels.value = false;
  }
}

function handleEdit(row: Provider) {
  editingId.value = row.id;
  formValue.value = {
    name: row.name,
    provider_type: row.provider_type,
    api_base_url: row.api_base_url,
    api_key: "",
    model_name: row.model_name || "",
    proxy_url: row.proxy_url || "",
    timeout_seconds: row.timeout_seconds,
    max_retries: row.max_retries,
    status: row.status,
  };
  showModal.value = true;
}

function handleDelete(row: Provider) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除 Provider "${row.name}" 吗？`,
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
    if (editingId.value) {
      const data: any = { ...formValue.value };
      if (!data.api_key) delete data.api_key;
      await providersStore.update(editingId.value, data);
      message.success("更新成功");
    } else {
      await providersStore.create(formValue.value);
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
        <h2 class="page-title">Provider 服务商</h2>
        <NTag size="small" type="info">{{ providers.length }} 个服务商</NTag>
      </div>
      <div class="toolbar-right">
        <NInput
          v-model:value="searchQuery"
          placeholder="搜索服务商..."
          clearable
          style="width: 200px"
          size="small"
        >
          <template #prefix>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px;margin-top:2px"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
          </template>
        </NInput>
        <NButton type="primary" @click="handleAdd">+ 新增服务商</NButton>
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
          <h3 class="empty-title" v-if="searchQuery">未找到匹配的服务商</h3>
          <h3 class="empty-title" v-else>暂无服务商</h3>
          <p class="empty-desc" v-if="!searchQuery">添加第一个 AI 服务商，开始配置您的 API 网关</p>
          <NButton v-if="!searchQuery" type="primary" @click="handleAdd">+ 新增服务商</NButton>
        </div>
      </template>
      <!-- Data Table -->
      <NDataTable
        v-else
        :columns="columns"
        :data="filteredProviders"
        :loading="loading"
        :bordered="false"
        :single-line="false"
        striped
      />
    </NCard>

    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editingId ? '编辑 Provider' : '添加 Provider'"
      style="max-width: 600px"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <NForm ref="formRef" :model="formValue" label-placement="left" label-width="100">
        <NFormItem label="名称" required>
          <NInput v-model:value="formValue.name" placeholder="如：OpenAI 官方" />
        </NFormItem>
        <NFormItem label="类型" required>
          <NSelect v-model:value="formValue.provider_type" :options="typeOptions" />
        </NFormItem>
        <NFormItem label="API 地址" required>
          <NInput v-model:value="formValue.api_base_url" placeholder="https://api.openai.com/v1" />
        </NFormItem>
        <NFormItem :label="editingId ? 'API Key (留空不修改)' : 'API Key'" :required="!editingId">
          <NInput v-model:value="formValue.api_key" type="password" placeholder="sk-..." show-password-on="click" />
        </NFormItem>
        <NFormItem label="默认模型">
          <div style="display: flex; gap: 8px; width: 100%; align-items: flex-start">
            <NSelect
              v-model:value="formValue.model_name"
              :options="modelOptions"
              :placeholder="modelOptions.length > 0 ? '选择模型' : 'gpt-4o'"
              :allow-clear="true"
              :filterable="true"
              :tag="true"
              style="flex: 1"
              :disabled="fetchingModels"
            />
            <NButton
              secondary
              size="small"
              @click="fetchModels"
              :loading="fetchingModels"
              :disabled="!formValue.api_base_url || !formValue.api_key"
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

.table-card {
  border-radius: 12px;
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
