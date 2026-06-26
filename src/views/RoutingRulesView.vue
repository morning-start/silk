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
import { useRoutingRulesStore } from "../stores/routingRules";
import { useProvidersStore } from "../stores/providers";
import { useGroupsStore } from "../stores/groups";
import { storeToRefs } from "pinia";
import type { RoutingRule } from "../api";

const rulesStore = useRoutingRulesStore();
const providersStore = useProvidersStore();
const groupsStore = useGroupsStore();
const { rules, loading } = storeToRefs(rulesStore);
const { providers } = storeToRefs(providersStore);
const { groups } = storeToRefs(groupsStore);
const message = useMessage();
const dialog = useDialog();

const showModal = ref(false);
const editingId = ref<string | null>(null);

const formValue = ref<any>({
  name: "",
  match_path: "",
  match_method: "*",
  match_content_type: "",
  target_provider_id: "",
  target_group_id: null,
  protocol_conversion: true,
  model_name_override: "",
  priority: 100,
  enabled: true,
});

const methodOptions = [
  { label: "全部 (*)", value: "*" },
  { label: "GET", value: "GET" },
  { label: "POST", value: "POST" },
  { label: "PUT", value: "PUT" },
  { label: "DELETE", value: "DELETE" },
];

const columns: DataTableColumns<RoutingRule> = [
  { title: "名称", key: "name", width: 120 },
  { title: "匹配路径", key: "match_path", width: 180 },
  { title: "方法", key: "match_method", width: 80 },
  {
    title: "目标",
    key: "target",
    width: 150,
    render(row) {
      if (row.target_group_id) {
        const g = groups.value.find((gr) => gr.id === row.target_group_id);
        return h(NText, {}, { default: () => `分组: ${g?.name || row.target_group_id}` });
      }
      const p = providers.value.find((pr) => pr.id === row.target_provider_id);
      return h(NText, {}, { default: () => p?.name || row.target_provider_id });
    },
  },
  {
    title: "协议转换",
    key: "protocol_conversion",
    width: 90,
    render(row) {
      return h(
        NText,
        { type: row.protocol_conversion ? "success" : "default" },
        { default: () => (row.protocol_conversion ? "开启" : "关闭") }
      );
    },
  },
  { title: "优先级", key: "priority", width: 70 },
  {
    title: "状态",
    key: "enabled",
    width: 70,
    render(row) {
      return h(
        NText,
        { type: row.enabled ? "success" : "warning" },
        { default: () => (row.enabled ? "启用" : "禁用") }
      );
    },
  },
  {
    title: "操作",
    key: "actions",
    width: 160,
    render(row) {
      return [
        h(NButton, { size: "small", onClick: () => handleEdit(row) }, { default: () => "编辑" }),
        " ",
        h(NButton, { size: "small", type: "error", onClick: () => handleDelete(row) }, { default: () => "删除" }),
      ];
    },
  },
];

function handleAdd() {
  editingId.value = null;
  formValue.value = {
    name: "",
    match_path: "",
    match_method: "*",
    match_content_type: "",
    target_provider_id: "",
    target_group_id: null,
    protocol_conversion: true,
    model_name_override: "",
    priority: 100,
    enabled: true,
  };
  showModal.value = true;
}

function handleEdit(row: RoutingRule) {
  editingId.value = row.id;
  formValue.value = { ...row };
  showModal.value = true;
}

function handleDelete(row: RoutingRule) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除规则 "${row.name}" 吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await rulesStore.remove(row.id);
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
      await rulesStore.update(editingId.value, formValue.value);
      message.success("更新成功");
    } else {
      await rulesStore.create(formValue.value);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch {
    // error handled by store
  }
}

onMounted(() => {
  rulesStore.fetchAll();
  providersStore.fetchAll();
  groupsStore.fetchAll();
});
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <n-text style="font-size: 18px; font-weight: 600">路由规则</n-text>
      <n-button type="primary" @click="handleAdd">添加规则</n-button>
    </div>

    <n-data-table :columns="columns" :data="rules" :loading="loading" :bordered="false" striped />

    <n-modal
      v-model:show="showModal"
      preset="dialog"
      :title="editingId ? '编辑规则' : '添加规则'"
      style="width: 600px"
    >
      <n-form :model="formValue" label-placement="left" label-width="100">
        <n-form-item label="名称" required>
          <n-input v-model:value="formValue.name" placeholder="如：OpenAI Chat 转发" />
        </n-form-item>
        <n-form-item label="匹配路径" required>
          <n-input v-model:value="formValue.match_path" placeholder="/v1/chat/completions" />
        </n-form-item>
        <n-form-item label="匹配方法">
          <n-select v-model:value="formValue.match_method" :options="methodOptions" />
        </n-form-item>
        <n-form-item label="Content-Type">
          <n-input v-model:value="formValue.match_content_type" placeholder="可选，如：application/json" />
        </n-form-item>
        <n-form-item label="目标类型">
          <n-select
            :value="formValue.target_group_id ? 'group' : 'provider'"
            @update:value="(v: string) => { if (v === 'group') { formValue.target_provider_id = ''; } else { formValue.target_group_id = null; } }"
            :options="[
              { label: '单个 Provider', value: 'provider' },
              { label: '负载均衡分组', value: 'group' },
            ]"
          />
        </n-form-item>
        <n-form-item v-if="!formValue.target_group_id" label="目标 Provider">
          <n-select
            v-model:value="formValue.target_provider_id"
            :options="providers.map((p) => ({ label: p.name, value: p.id }))"
            placeholder="选择 Provider"
          />
        </n-form-item>
        <n-form-item v-if="formValue.target_group_id" label="目标分组">
          <n-select
            v-model:value="formValue.target_group_id"
            :options="groups.map((g) => ({ label: g.name, value: g.id }))"
            placeholder="选择分组"
          />
        </n-form-item>
        <n-form-item label="模型覆盖">
          <n-input v-model:value="formValue.model_name_override" placeholder="可选" />
        </n-form-item>
        <n-form-item label="优先级">
          <n-input-number v-model:value="formValue.priority" :min="0" :max="10000" style="width: 100%" />
        </n-form-item>
        <n-form-item label="协议转换">
          <n-switch v-model:value="formValue.protocol_conversion" />
        </n-form-item>
        <n-form-item label="启用">
          <n-switch v-model:value="formValue.enabled" />
        </n-form-item>
      </n-form>
      <template #action>
        <n-button @click="showModal = false">取消</n-button>
        <n-button type="primary" @click="handleSubmit">确定</n-button>
      </template>
    </n-modal>
  </div>
</template>
