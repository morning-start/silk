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
  NTag,
  useMessage,
  useDialog,
  type DataTableColumns,
} from "naive-ui";
import { useGroupsStore } from "../stores/groups";
import { useProvidersStore } from "../stores/providers";
import { storeToRefs } from "pinia";
import type { ProviderGroup } from "../api";

const groupsStore = useGroupsStore();
const providersStore = useProvidersStore();
const { groups, loading } = storeToRefs(groupsStore);
const { providers } = storeToRefs(providersStore);
const message = useMessage();
const dialog = useDialog();

const showModal = ref(false);
const showMemberModal = ref(false);
const editingId = ref<string | null>(null);
const selectedGroupId = ref<string>("");

const formValue = ref({
  name: "",
  model_name: "",
  strategy: "round_robin",
  enabled: true,
});

const memberFormValue = ref({
  provider_id: "",
  weight: 1,
});

const strategyOptions = [
  { label: "轮询 (Round Robin)", value: "round_robin" },
  { label: "加权 (Weighted)", value: "weighted" },
  { label: "最少连接 (Least Conn)", value: "least_conn" },
];

const columns: DataTableColumns<ProviderGroup> = [
  { title: "名称", key: "name", width: 150 },
  { title: "模型名", key: "model_name", width: 150 },
  {
    title: "策略",
    key: "strategy",
    width: 120,
    render(row) {
      const label = strategyOptions.find((s) => s.value === row.strategy)?.label || row.strategy;
      return h(NTag, { size: "small" }, { default: () => label });
    },
  },
  {
    title: "状态",
    key: "enabled",
    width: 80,
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
    width: 240,
    render(row) {
      return [
        h(NButton, { size: "small", onClick: () => handleManageMembers(row) }, { default: () => "成员" }),
        " ",
        h(NButton, { size: "small", onClick: () => handleEdit(row) }, { default: () => "编辑" }),
        " ",
        h(NButton, { size: "small", type: "error", onClick: () => handleDelete(row) }, { default: () => "删除" }),
      ];
    },
  },
];

const memberColumns = [
  { title: "ID", key: "provider_id", width: 200 },
  { title: "权重", key: "weight", width: 80 },
  {
    title: "状态",
    key: "enabled",
    width: 80,
    render(row: any) {
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
    width: 100,
    render(row: any) {
      return h(
        NButton,
        { size: "small", type: "error", onClick: () => handleRemoveMember(row.id) },
        { default: () => "移除" }
      );
    },
  },
];

function handleAdd() {
  editingId.value = null;
  formValue.value = { name: "", model_name: "", strategy: "round_robin", enabled: true };
  showModal.value = true;
}

function handleEdit(row: ProviderGroup) {
  editingId.value = row.id;
  formValue.value = {
    name: row.name,
    model_name: row.model_name,
    strategy: row.strategy,
    enabled: row.enabled,
  };
  showModal.value = true;
}

function handleDelete(row: ProviderGroup) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除分组 "${row.name}" 吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await groupsStore.remove(row.id);
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
      await groupsStore.update(editingId.value, formValue.value);
      message.success("更新成功");
    } else {
      await groupsStore.create(formValue.value);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch {
    // error handled by store
  }
}

function handleManageMembers(row: ProviderGroup) {
  selectedGroupId.value = row.id;
  groupsStore.fetchById(row.id);
  showMemberModal.value = true;
}

async function handleAddMember() {
  try {
    await groupsStore.addMember(selectedGroupId.value, memberFormValue.value.provider_id, memberFormValue.value.weight);
    message.success("添加成功");
    memberFormValue.value = { provider_id: "", weight: 1 };
  } catch {
    message.error("添加失败");
  }
}

async function handleRemoveMember(id: string) {
  try {
    await groupsStore.removeMember(id, selectedGroupId.value);
    message.success("移除成功");
  } catch {
    message.error("移除失败");
  }
}

onMounted(() => {
  groupsStore.fetchAll();
  providersStore.fetchAll();
});
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <n-text style="font-size: 18px; font-weight: 600">负载均衡分组</n-text>
      <n-button type="primary" @click="handleAdd">创建分组</n-button>
    </div>

    <n-data-table :columns="columns" :data="groups" :loading="loading" :bordered="false" striped />

    <!-- 创建/编辑分组 -->
    <n-modal
      v-model:show="showModal"
      preset="dialog"
      :title="editingId ? '编辑分组' : '创建分组'"
      style="width: 500px"
    >
      <n-form :model="formValue" label-placement="left" label-width="100">
        <n-form-item label="名称" required>
          <n-input v-model:value="formValue.name" placeholder="如：GPT-4 分组" />
        </n-form-item>
        <n-form-item label="模型名" required>
          <n-input v-model:value="formValue.model_name" placeholder="gpt-4o" />
        </n-form-item>
        <n-form-item label="负载策略">
          <n-select v-model:value="formValue.strategy" :options="strategyOptions" />
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

    <!-- 成员管理 -->
    <n-modal
      v-model:show="showMemberModal"
      preset="dialog"
      title="分组成员管理"
      style="width: 700px"
    >
      <div v-if="groupsStore.currentGroup">
        <div style="margin-bottom: 16px">
          <n-text>分组：</n-text>
          <n-text strong>{{ groupsStore.currentGroup.group.name }}</n-text>
        </div>

        <div style="display: flex; gap: 8px; margin-bottom: 16px">
          <n-select
            v-model:value="memberFormValue.provider_id"
            :options="providers.map((p) => ({ label: p.name, value: p.id }))"
            placeholder="选择 Provider"
            style="flex: 1"
          />
          <n-input-number v-model:value="memberFormValue.weight" :min="1" :max="100" style="width: 100px" />
          <n-button type="primary" @click="handleAddMember">添加</n-button>
        </div>

        <n-data-table :columns="memberColumns" :data="(groupsStore.currentGroup as any).members || []" :bordered="false" />
      </div>
      <template #action>
        <n-button @click="showMemberModal = false">关闭</n-button>
      </template>
    </n-modal>
  </div>
</template>
