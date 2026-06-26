<script setup lang="ts">
import { ref, onMounted, h } from "vue";
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
  NText,
  useMessage,
  useDialog,
  type DataTableColumns,
} from "naive-ui";
import { useProvidersStore } from "../stores/providers";
import { storeToRefs } from "pinia";
import type { Provider } from "../api";

const providersStore = useProvidersStore();
const { providers, loading } = storeToRefs(providersStore);
const message = useMessage();
const dialog = useDialog();

const showModal = ref(false);
const editingId = ref<string | null>(null);

const formRef = ref<any>(null);
const formValue = ref({
  name: "",
  provider_type: "openai",
  api_base_url: "",
  api_key: "",
  model_name: "",
  proxy_url: "",
  timeout_seconds: 30,
  max_retries: 3,
  status: "enabled",
});

const typeOptions = [
  { label: "OpenAI", value: "openai" },
  { label: "Anthropic", value: "anthropic" },
  { label: "Azure", value: "azure" },
  { label: "自定义", value: "custom" },
];

const columns: DataTableColumns<Provider> = [
  { title: "名称", key: "name", width: 120 },
  { title: "类型", key: "provider_type", width: 100 },
  { title: "模型", key: "model_name", width: 140 },
  {
    title: "状态",
    key: "status",
    width: 80,
    render(row) {
      return h(
        NText,
        { type: row.status === "enabled" ? "success" : "warning" },
        { default: () => (row.status === "enabled" ? "启用" : "禁用") }
      );
    },
  },
  { title: "超时(s)", key: "timeout_seconds", width: 80 },
  { title: "重试", key: "max_retries", width: 70 },
  {
    title: "操作",
    key: "actions",
    width: 180,
    render(row) {
      return [
        h(
          NButton,
          {
            size: "small",
            onClick: () => handleEdit(row),
          },
          { default: () => "编辑" }
        ),
        " ",
        h(
          NButton,
          {
            size: "small",
            type: "error",
            onClick: () => handleDelete(row),
          },
          { default: () => "删除" }
        ),
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
  showModal.value = true;
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
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <n-text style="font-size: 18px; font-weight: 600">Provider 管理</n-text>
      <n-button type="primary" @click="handleAdd">添加 Provider</n-button>
    </div>

    <n-data-table
      :columns="columns"
      :data="providers"
      :loading="loading"
      :bordered="false"
      striped
    />

    <n-modal
      v-model:show="showModal"
      preset="dialog"
      :title="editingId ? '编辑 Provider' : '添加 Provider'"
      style="width: 600px"
    >
      <n-form ref="formRef" :model="formValue" label-placement="left" label-width="100">
        <n-form-item label="名称" required>
          <n-input v-model:value="formValue.name" placeholder="如：OpenAI 官方" />
        </n-form-item>
        <n-form-item label="类型" required>
          <n-select v-model:value="formValue.provider_type" :options="typeOptions" />
        </n-form-item>
        <n-form-item label="API 地址" required>
          <n-input v-model:value="formValue.api_base_url" placeholder="https://api.openai.com/v1" />
        </n-form-item>
        <n-form-item :label="editingId ? 'API Key (留空不修改)' : 'API Key'" :required="!editingId">
          <n-input v-model:value="formValue.api_key" type="password" placeholder="sk-..." show-password-on="click" />
        </n-form-item>
        <n-form-item label="默认模型">
          <n-input v-model:value="formValue.model_name" placeholder="gpt-4o" />
        </n-form-item>
        <n-form-item label="代理地址">
          <n-input v-model:value="formValue.proxy_url" placeholder="可选" />
        </n-form-item>
        <n-form-item label="超时(秒)">
          <n-input-number v-model:value="formValue.timeout_seconds" :min="5" :max="300" style="width: 100%" />
        </n-form-item>
        <n-form-item label="最大重试">
          <n-input-number v-model:value="formValue.max_retries" :min="0" :max="10" style="width: 100%" />
        </n-form-item>
        <n-form-item label="启用">
          <n-switch v-model:value="formValue.status" checked-value="enabled" unchecked-value="disabled" />
        </n-form-item>
      </n-form>
      <template #action>
        <n-button @click="showModal = false">取消</n-button>
        <n-button type="primary" @click="handleSubmit">确定</n-button>
      </template>
    </n-modal>
  </div>
</template>
