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
  NTag,
  NCard,
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
const { rules, loading, error } = storeToRefs(rulesStore);
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
  { title: "名称", key: "name", width: 130 },
  {
    title: "匹配路径",
    key: "match_path",
    width: 180,
    render(row) {
      return h("span", { class: "text-mono" }, row.match_path);
    },
  },
  {
    title: "方法",
    key: "match_method",
    width: 80,
    render(row) {
      return h(NTag, { size: "small", type: "info" }, { default: () => row.match_method === "*" ? "ANY" : row.match_method });
    },
  },
  {
    title: "目标",
    key: "target",
    width: 150,
    render(row) {
      if (row.target_group_id) {
        const g = groups.value.find((gr) => gr.id === row.target_group_id);
        return h(NTag, { size: "small", type: "success" }, { default: () => `分组: ${g?.name || row.target_group_id}` });
      }
      const p = providers.value.find((pr) => pr.id === row.target_provider_id);
      return h(NTag, { size: "small", type: "primary" }, { default: () => p?.name || row.target_provider_id });
    },
  },
  {
    title: "协议转换",
    key: "protocol_conversion",
    width: 90,
    render(row) {
      return h(
        NTag,
        { size: "small", type: row.protocol_conversion ? "success" : "default" },
        { default: () => row.protocol_conversion ? "开启" : "关闭" }
      );
    },
  },
  {
    title: "状态",
    key: "enabled",
    width: 70,
    render(row) {
      return h(
        NSwitch,
        { value: row.enabled, onUpdateValue: () => toggleEnabled(row), size: "small" },
      );
    },
  },
  {
    title: "操作",
    key: "actions",
    width: 140,
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

async function toggleEnabled(row: RoutingRule) {
  try {
    await rulesStore.update(row.id, { enabled: !row.enabled });
    message.success(row.enabled ? "已禁用" : "已启用");
  } catch {
    message.error("操作失败");
  }
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

const targetType = ref<"provider" | "group">("provider");

onMounted(() => {
  rulesStore.fetchAll();
  providersStore.fetchAll();
  groupsStore.fetchAll();
});
</script>

<template>
  <div class="rules-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">路由规则</h2>
        <NTag size="small" type="info">{{ rules.length }} 条规则</NTag>
      </div>
      <div class="toolbar-right">
        <NButton type="primary" @click="handleAdd">+ 新增路由</NButton>
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
          <NButton type="primary" @click="rulesStore.fetchAll()">重新加载</NButton>
        </div>
      </template>
      <!-- Empty State -->
      <template v-else-if="!loading && rules.length === 0">
        <div class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
          </div>
          <h3 class="empty-title">暂无路由规则</h3>
          <p class="empty-desc">添加路由规则来匹配 API 请求，将其转发到对应的 Provider</p>
          <NButton type="primary" @click="handleAdd">+ 新增路由</NButton>
        </div>
      </template>
      <!-- Data Table -->
      <NDataTable
        v-else
        :columns="columns"
        :data="rules"
        :loading="loading"
        :bordered="false"
        :single-line="false"
        striped
      />
    </NCard>

    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editingId ? '编辑规则' : '添加路由'"
      style="max-width: 600px"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <NForm :model="formValue" label-placement="left" label-width="100">
        <NFormItem label="名称" required>
          <NInput v-model:value="formValue.name" placeholder="如：OpenAI Chat 转发" />
        </NFormItem>
        <NFormItem label="匹配路径" required>
          <NInput v-model:value="formValue.match_path" placeholder="/v1/chat/completions" />
        </NFormItem>
        <div class="form-row">
          <NFormItem label="匹配方法" style="flex: 1">
            <NSelect v-model:value="formValue.match_method" :options="methodOptions" />
          </NFormItem>
          <NFormItem label="Content-Type" style="flex: 1">
            <NInput v-model:value="formValue.match_content_type" placeholder="可选" />
          </NFormItem>
        </div>
        <NFormItem label="目标类型">
          <NSelect
            :value="formValue.target_group_id ? 'group' : 'provider'"
            @update:value="(v: string) => {
              targetType = v as any;
              if (v === 'group') { formValue.target_provider_id = ''; }
              else { formValue.target_group_id = null; }
            }"
            :options="[
              { label: '单个 Provider', value: 'provider' },
              { label: '负载均衡分组', value: 'group' },
            ]"
          />
        </NFormItem>
        <NFormItem v-if="!formValue.target_group_id" label="目标 Provider">
          <NSelect
            v-model:value="formValue.target_provider_id"
            :options="providers.map((p) => ({ label: p.name, value: p.id }))"
            placeholder="选择 Provider"
            clearable
          />
        </NFormItem>
        <NFormItem v-if="formValue.target_group_id || targetType === 'group'" label="目标分组">
          <NSelect
            v-model:value="formValue.target_group_id"
            :options="groups.map((g) => ({ label: g.name, value: g.id }))"
            placeholder="选择分组"
            clearable
          />
        </NFormItem>
        <div class="form-row">
          <NFormItem label="模型覆盖" style="flex: 1">
            <NInput v-model:value="formValue.model_name_override" placeholder="可选" />
          </NFormItem>
          <NFormItem label="优先级" style="flex: 0 0 120px">
            <NInputNumber v-model:value="formValue.priority" :min="0" :max="10000" style="width: 100%" />
          </NFormItem>
        </div>
        <div class="form-row">
          <NFormItem label="协议转换" style="flex: 1">
            <NSwitch v-model:value="formValue.protocol_conversion" />
          </NFormItem>
          <NFormItem label="启用" style="flex: 1">
            <NSwitch v-model:value="formValue.enabled" />
          </NFormItem>
        </div>
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
.rules-page {
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

.text-mono {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-size: 12px;
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
