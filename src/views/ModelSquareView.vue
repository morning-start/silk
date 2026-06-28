<script setup lang="ts">
import { ref, onMounted, computed, watch } from "vue";
import {
  NCard,
  NButton,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  NEmpty,
  NSpin,
  NGrid,
  NGi,
  useMessage,
  useDialog,
} from "naive-ui";
import { api, type ModelMapping, type ProviderGroup, type GroupProviderInfo } from "../api";

const message = useMessage();
const dialog = useDialog();

const mappings = ref<ModelMapping[]>([]);
const groups = ref<ProviderGroup[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const showModal = ref(false);
const editingId = ref<string | null>(null);

const groupProviders = ref<GroupProviderInfo[]>([]);
const loadingProviders = ref(false);

const formValue = ref({
  model_name: "",
  provider_group_id: null as string | null,
  max_input_tokens: null as number | null,
  max_context_tokens: null as number | null,
  max_output_tokens: null as number | null,
  input_price_per_1m: null as number | null,
  output_price_per_1m: null as number | null,
  capabilities: [] as string[],
  description: "",
  vendor: "",
  knowledge_cutoff: null as string | null,
  model_family: "",
  reference_url: null as string | null,
  strategy: "weighted_round_robin",
  enabled: true,
});

watch(
  () => formValue.value.provider_group_id,
  async (groupId) => {
    groupProviders.value = [];
    if (!groupId) return;
    loadingProviders.value = true;
    try {
      groupProviders.value = await api.getGroupProviders(groupId);
    } catch {
      // ignore
    } finally {
      loadingProviders.value = false;
    }
  }
);

const capabilityOptions = [
  { label: "思考", value: "thinking" },
  { label: "识图", value: "vision" },
  { label: "文本", value: "text" },
  { label: "代码", value: "code" },
  { label: "生图", value: "image_gen" },
  { label: "语音", value: "audio" },
];

function capabilityLabel(val: string): string {
  return capabilityOptions.find((c) => c.value === val)?.label || val;
}

function capabilityColor(val: string): string {
  const colors: Record<string, string> = {
    thinking: "purple",
    vision: "blue",
    text: "default",
    code: "green",
    image_gen: "orange",
    audio: "pink",
  };
  return colors[val] || "default";
}

const groupOptions = computed(() =>
  groups.value.map((g) => ({
    label: `${g.name} (${g.model_name})`,
    value: g.id,
  }))
);

async function loadData() {
  loading.value = true;
  error.value = null;
  try {
    const [m, g] = await Promise.all([
      api.listModelMappings(),
      api.listGroups(),
    ]);
    mappings.value = m;
    groups.value = g;
  } catch (e: any) {
    error.value = e.message || "加载数据失败";
  } finally {
    loading.value = false;
  }
}

function handleAdd() {
  editingId.value = null;
  formValue.value = {
    model_name: "",
    provider_group_id: null,
    max_input_tokens: null,
    max_context_tokens: null,
    max_output_tokens: null,
    input_price_per_1m: null,
    output_price_per_1m: null,
    capabilities: [],
    description: "",
    vendor: "",
    knowledge_cutoff: null,
    model_family: "",
    reference_url: null,
    strategy: "weighted_round_robin",
    enabled: true,
  };
  groupProviders.value = [];
  showModal.value = true;
}

function handleEdit(row: ModelMapping) {
  editingId.value = row.id;
  formValue.value = {
    model_name: row.model_name,
    provider_group_id: row.provider_group_id,
    max_input_tokens: row.max_input_tokens,
    max_context_tokens: row.max_context_tokens,
    max_output_tokens: row.max_output_tokens,
    input_price_per_1m: row.input_price_per_1m,
    output_price_per_1m: row.output_price_per_1m,
    capabilities: row.capabilities || [],
    description: row.description || "",
    vendor: row.vendor || "",
    knowledge_cutoff: row.knowledge_cutoff || null,
    model_family: row.model_family || "",
    reference_url: row.reference_url || null,
    strategy: "weighted_round_robin",
    enabled: row.enabled,
  };
  showModal.value = true;
}

function handleDelete(row: ModelMapping) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除模型映射 "${row.model_name}" 吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteModelMapping(row.id);
        mappings.value = mappings.value.filter((m) => m.id !== row.id);
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
      const updated = await api.updateModelMapping(editingId.value, formValue.value as any);
      const idx = mappings.value.findIndex((m) => m.id === editingId.value);
      if (idx >= 0) mappings.value[idx] = updated;
      message.success("更新成功");
    } else {
      const created = await api.createModelMapping(formValue.value as any);
      mappings.value.unshift(created);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch {
    message.error("操作失败");
  }
}

function formatPrice(val: number | null): string {
  if (val == null) return "-";
  return `$${val}/1M`;
}

function formatTokens(val: number | null): string {
  if (val == null) return "-";
  if (val >= 1000) return `${val / 1000}K`;
  return `${val}`;
}

const groupName = (id: string | null) => {
  if (!id) return "-";
  return groups.value.find((g) => g.id === id)?.name || id;
};

onMounted(loadData);
</script>

<template>
  <div class="model-square">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">模型池</h2>
        <NTag size="small" type="info">{{ mappings.length }} 个模型</NTag>
      </div>
      <div class="toolbar-right">
        <NButton type="primary" @click="handleAdd">+ 新增模型映射</NButton>
      </div>
    </div>

    <NSpin :show="loading" style="min-height: 200px">
      <!-- Error State -->
      <template v-if="error && !loading">
        <div class="error-state">
          <div class="error-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <h3 class="error-title">数据加载失败</h3>
          <p class="error-desc">{{ error }}</p>
          <NButton type="primary" @click="loadData()">重新加载</NButton>
        </div>
      </template>
      <!-- Empty State -->
      <template v-else-if="mappings.length === 0 && !loading">
        <NEmpty description="暂无模型映射，点击上方按钮添加" />
      </template>

      <NGrid v-else :x-gap="16" :y-gap="16" :cols="3" style="margin-top: 16px">
        <NGi v-for="item in mappings" :key="item.id">
          <NCard
            :bordered="false"
            class="model-card"
            :class="{ disabled: !item.enabled }"
          >
            <div class="mc-header">
              <div class="mc-name-group">
                <span class="mc-name">{{ item.model_name }}</span>
                <NTag size="tiny" type="success" v-if="item.enabled">启用</NTag>
                <NTag size="tiny" type="warning" v-else>禁用</NTag>
              </div>
            </div>

            <div class="mc-vendor" v-if="item.vendor">{{ item.vendor }}</div>
            <div class="mc-desc" v-if="item.description">{{ item.description }}</div>

            <div class="mc-stats">
              <span>
                渠道 <span class="num">{{ item.provider_group_id ? 1 : 0 }}</span>
              </span>
            </div>

            <div class="mc-specs">
              <template v-if="item.max_input_tokens">
                <span>输入 <span class="num">{{ formatTokens(item.max_input_tokens) }}</span></span>
                <span class="sep">·</span>
              </template>
              <template v-if="item.max_context_tokens">
                <span>上下文 <span class="num">{{ formatTokens(item.max_context_tokens) }}</span></span>
                <span class="sep">·</span>
              </template>
              <template v-if="item.max_output_tokens">
                <span>输出 <span class="num">{{ formatTokens(item.max_output_tokens) }}</span></span>
              </template>
            </div>

            <div class="mc-pricing" v-if="item.input_price_per_1m || item.output_price_per_1m">
              <span v-if="item.input_price_per_1m">
                输入 <span class="num">{{ formatPrice(item.input_price_per_1m) }}</span>
              </span>
              <span class="sep" v-if="item.input_price_per_1m && item.output_price_per_1m">·</span>
              <span v-if="item.output_price_per_1m">
                输出 <span class="num">{{ formatPrice(item.output_price_per_1m) }}</span>
              </span>
            </div>

            <div class="mc-caps" v-if="item.capabilities && item.capabilities.length > 0">
              <NTag
                v-for="cap in item.capabilities"
                :key="cap"
                :type="capabilityColor(cap) as any"
                size="tiny"
                round
              >
                {{ capabilityLabel(cap) }}
              </NTag>
            </div>

            <div class="mc-group" v-if="item.provider_group_id">
              <span class="group-badge">{{ groupName(item.provider_group_id) }}</span>
            </div>

            <div class="mc-actions">
              <NButton size="tiny" quaternary @click="handleEdit(item)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="handleDelete(item)">删除</NButton>
            </div>
          </NCard>
        </NGi>
      </NGrid>
    </NSpin>

    <!-- Add/Edit Modal -->
    <NModal
      v-model:show="showModal"
      preset="card"
      :title="editingId ? '编辑模型映射' : '新增模型映射'"
      style="max-width: 560px"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <NForm :model="formValue" label-placement="left" label-width="110">
        <NFormItem label="模型名称" required>
          <NInput v-model:value="formValue.model_name" placeholder="例如：gpt-4、claude-3-opus" />
        </NFormItem>
        <NFormItem label="关联分组">
          <NSelect
            v-model:value="formValue.provider_group_id"
            :options="groupOptions"
            placeholder="选择渠道分组"
            clearable
          />
        </NFormItem>

        <!-- 分组内渠道列表 -->
        <NFormItem v-if="groupProviders.length > 0" label="渠道列表">
          <div style="width: 100%; display: flex; flex-direction: column; gap: 6px">
            <div v-for="p in groupProviders" :key="p.id" class="gp-row">
              <span class="gp-name">{{ p.name }}</span>
              <span class="gp-protocols">{{ p.protocols.join(', ') }}</span>
              <span class="gp-models">{{ p.models_count }} 模型</span>
              <NTag size="tiny" :type="p.health_status === 'healthy' ? 'success' : 'warning'">
                {{ p.health_status === 'healthy' ? '正常' : '异常' }}
              </NTag>
            </div>
          </div>
        </NFormItem>

        <div class="form-row">
          <NFormItem label="最大输入" style="flex: 1">
            <NInputNumber v-model:value="formValue.max_input_tokens" placeholder="128K" :min="0" style="width: 100%" />
          </NFormItem>
          <NFormItem label="上下文" style="flex: 1">
            <NInputNumber v-model:value="formValue.max_context_tokens" placeholder="128K" :min="0" style="width: 100%" />
          </NFormItem>
          <NFormItem label="最大输出" style="flex: 1">
            <NInputNumber v-model:value="formValue.max_output_tokens" placeholder="4K" :min="0" style="width: 100%" />
          </NFormItem>
        </div>

        <div class="form-row">
          <NFormItem label="输入价格" style="flex: 1">
            <NInputNumber v-model:value="formValue.input_price_per_1m" placeholder="$30/1M" :min="0" style="width: 100%" />
          </NFormItem>
          <NFormItem label="输出价格" style="flex: 1">
            <NInputNumber v-model:value="formValue.output_price_per_1m" placeholder="$60/1M" :min="0" style="width: 100%" />
          </NFormItem>
        </div>

        <NFormItem label="描述">
          <NInput v-model:value="formValue.description" placeholder="模型描述，如"最新 GPT-4 模型，支持多模态"" type="textarea" :rows="2" />
        </NFormItem>

        <div class="form-row">
          <NFormItem label="厂商" style="flex: 1">
            <NInput v-model:value="formValue.vendor" placeholder="OpenAI" />
          </NFormItem>
          <NFormItem label="模型系列" style="flex: 1">
            <NInput v-model:value="formValue.model_family" placeholder="gpt-4" />
          </NFormItem>
        </div>

        <div class="form-row">
          <NFormItem label="知识截止" style="flex: 1">
            <NInput v-model:value="formValue.knowledge_cutoff" placeholder="2024-04" />
          </NFormItem>
          <NFormItem label="参考链接" style="flex: 1">
            <NInput v-model:value="formValue.reference_url" placeholder="https://..." />
          </NFormItem>
        </div>

        <NFormItem label="模型能力">
          <div class="cap-checkboxes">
            <label v-for="cap in capabilityOptions" :key="cap.value" class="cap-checkbox">
              <input type="checkbox" :value="cap.value" :checked="formValue.capabilities.includes(cap.value)"
                @change="(e: any) => {
                  if (e.target.checked) formValue.capabilities.push(cap.value);
                  else formValue.capabilities = formValue.capabilities.filter((c) => c !== cap.value);
                }"
              />
              {{ cap.label }}
            </label>
          </div>
        </NFormItem>

        <NFormItem label="启用">
          <NSwitch v-model:value="formValue.enabled" />
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
.model-square {
  max-width: 1200px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.model-card {
  border-radius: 12px;
  transition: box-shadow 0.2s;
}

.model-card:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
}

.model-card.disabled {
  opacity: 0.6;
}

.mc-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.mc-name-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mc-name {
  font-size: 16px;
  font-weight: 600;
}

.mc-vendor {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  margin-bottom: 4px;
}

.mc-desc {
  font-size: 13px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 8px;
  line-height: 1.4;
}

.mc-strategy {
  font-size: 11px;
  color: var(--text-color-3, #94a3b8);
  background: var(--tag-bg, #f1f5f9);
  padding: 2px 8px;
  border-radius: 4px;
}

.mc-stats {
  font-size: 13px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 8px;
}

.mc-specs {
  font-size: 12px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
}

.mc-pricing {
  font-size: 12px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
}

.sep {
  color: var(--border-color, #e2e8f0);
  margin: 0 4px;
}

.num {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-weight: 600;
}

.mc-caps {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}

.mc-group {
  margin-bottom: 8px;
}

.group-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: #eef2ff;
  color: #6366f1;
  font-weight: 500;
}

.mc-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  padding-top: 10px;
  margin-top: 4px;
}

.gp-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid var(--border-color, #e2e8f0);
  font-size: 13px;
}

.gp-name {
  font-weight: 600;
  min-width: 80px;
}

.gp-protocols {
  color: var(--text-color-2, #64748b);
  font-size: 12px;
  flex: 1;
}

.gp-models {
  color: var(--text-color-2, #64748b);
  font-size: 12px;
  font-family: 'JetBrains Mono', 'Consolas', monospace;
}

.form-row {
  display: flex;
  gap: 12px;
}

.cap-checkboxes {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.cap-checkbox {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  cursor: pointer;
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
