<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import {
  NCard,
  NButton,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  NGrid,
  NGi,
  NIcon,
  useMessage,
  useDialog,
} from "naive-ui";
import {
  SearchOutline,
} from "@vicons/ionicons5";
import { api, type ModelMapping, type NewMappingChannel, type Provider } from "../api";
import AppFormModal from "../components/AppFormModal.vue";
import AppPageShell from "../components/AppPageShell.vue";

const message = useMessage();
const dialog = useDialog();

const mappings = ref<ModelMapping[]>([]);
const allProviders = ref<Provider[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const showModal = ref(false);
const editingId = ref<string | null>(null);

// 表单数据
const formValue = ref({
  model_name: "",
  strategy: "round_robin",
  // 步骤①：选中的渠道 ID 列表
  selectedProviderIds: [] as string[],
  // 步骤②：每个渠道选中的模型列表 { provider_id → model[] }
  selectedModelsMap: {} as Record<string, string[]>,
  max_input_tokens: null as number | null,
  max_context_tokens: null as number | null,
  max_output_tokens: null as number | null,
  input_price_per_1m: null as number | null,
  output_price_per_1m: null as number | null,
  capabilities: [] as string[],
  description: "",
  enabled: true,
});

// 步骤②：模型模糊搜索关键字
const modelSearchKeyword = ref("");

// 已选渠道的提供者信息（供模型步骤使用）
const selectedProviders = computed(() =>
  allProviders.value.filter((p) => formValue.value.selectedProviderIds.includes(p.id))
);

// 按渠道分组的模型（供步骤②展示）
const channelModels = computed(() => {
  const kw = modelSearchKeyword.value.trim().toLowerCase();
  return selectedProviders.value.map((p) => {
    const models = (p.models || []).filter((m) => !kw || m.toLowerCase().includes(kw));
    return {
      provider_id: p.id,
      provider_name: p.name,
      models,
      selectedModels: formValue.value.selectedModelsMap[p.id] || [],
    };
  });
});

// 切换选中/取消某个渠道的某个模型
function toggleModel(providerId: string, model: string) {
  const map = { ...formValue.value.selectedModelsMap };
  if (!map[providerId]) map[providerId] = [];
  const idx = map[providerId].indexOf(model);
  if (idx >= 0) {
    map[providerId] = map[providerId].filter((m) => m !== model);
  } else {
    map[providerId] = [...map[providerId], model];
  }
  formValue.value.selectedModelsMap = map;
}

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

async function loadData() {
  loading.value = true;
  error.value = null;
  try {
    const [m, p] = await Promise.all([
      api.listModelMappings(),
      api.listProviders(),
    ]);
    mappings.value = m;
    allProviders.value = p;
  } catch (e: any) {
    error.value = e.message || "加载数据失败";
  } finally {
    loading.value = false;
  }
}

function resetForm() {
  editingId.value = null;
  formValue.value = {
    model_name: "",
    strategy: "round_robin",
    selectedProviderIds: [],
    selectedModelsMap: {},
    max_input_tokens: null,
    max_context_tokens: null,
    max_output_tokens: null,
    input_price_per_1m: null,
    output_price_per_1m: null,
    capabilities: [],
    description: "",
    enabled: true,
  };
  modelSearchKeyword.value = "";
}

function handleAdd() {
  resetForm();
  showModal.value = true;
}

function handleEdit(row: ModelMapping) {
  editingId.value = row.id;
  // 从 channels 回填 selectedProviderIds 和 selectedModelsMap
  const providerIds: string[] = [];
  const modelsMap: Record<string, string[]> = {};
  for (const c of row.channels || []) {
    providerIds.push(c.provider_id);
    if (c.selected_models && c.selected_models.length > 0) {
      modelsMap[c.provider_id] = c.selected_models;
    }
  }
  formValue.value = {
    model_name: row.model_name,
    strategy: row.strategy || "round_robin",
    selectedProviderIds: providerIds,
    selectedModelsMap: modelsMap,
    max_input_tokens: row.max_input_tokens,
    max_context_tokens: row.max_context_tokens,
    max_output_tokens: row.max_output_tokens,
    input_price_per_1m: row.input_price_per_1m,
    output_price_per_1m: row.output_price_per_1m,
    capabilities: row.capabilities || [],
    description: row.description || "",
    enabled: row.enabled,
  };
  modelSearchKeyword.value = "";
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
    const channels: NewMappingChannel[] = formValue.value.selectedProviderIds.map((pid) => {
      const selectedModels = formValue.value.selectedModelsMap[pid];
      return {
        provider_id: pid,
        selected_models: selectedModels && selectedModels.length > 0 ? selectedModels : undefined,
      };
    });

    const payload = {
      model_name: formValue.value.model_name,
      strategy: formValue.value.strategy,
      max_input_tokens: formValue.value.max_input_tokens,
      max_context_tokens: formValue.value.max_context_tokens,
      max_output_tokens: formValue.value.max_output_tokens,
      input_price_per_1m: formValue.value.input_price_per_1m,
      output_price_per_1m: formValue.value.output_price_per_1m,
      capabilities: formValue.value.capabilities.length > 0 ? formValue.value.capabilities : undefined,
      description: formValue.value.description || undefined,
      enabled: formValue.value.enabled,
      channels: channels.length > 0 ? channels : undefined,
    };

    if (editingId.value) {
      const updated = await api.updateModelMapping(editingId.value, payload as any);
      const idx = mappings.value.findIndex((m) => m.id === editingId.value);
      if (idx >= 0) mappings.value[idx] = updated;
      message.success("更新成功");
    } else {
      const created = await api.createModelMapping(payload as any);
      mappings.value.unshift(created);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch (e: any) {
    message.error(e?.message || "操作失败");
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

// 渠道健康状态显示
function healthColor(status: string | null): "success" | "error" | "warning" | "default" {
  return status === "healthy" ? "success" : status === "unhealthy" ? "error" : "warning";
}
function healthLabel(status: string | null): string {
  return status === "healthy" ? "正常" : status === "unhealthy" ? "异常" : "未知";
}

// 卡片显示渠道的选中模型摘要
function channelModelSummary(selected: string[]): string {
  if (!selected || selected.length === 0) return "";
  if (selected.length <= 2) return selected.join(", ");
  return `${selected.slice(0, 2).join(", ")} +${selected.length - 2}`;
}

onMounted(loadData);
</script>

<template>
  <AppPageShell
    title="模型池"
    :loading="loading"
    :error="error"
    :empty="mappings.length === 0"
    @reload="loadData()"
  >
    <template #count>
      <NTag size="small" type="info">{{ mappings.length }} 个模型</NTag>
    </template>
    <template #actions>
      <NButton type="primary" @click="handleAdd">+ 新增模型映射</NButton>
    </template>
    <template #empty>
      <div class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><circle cx="12" cy="12" r="9"/><path d="M8 12h8"/><path d="M12 8v8"/></svg>
        </div>
        <h3 class="empty-title">暂无模型映射</h3>
        <p class="empty-desc">先把多个渠道下的模型归并到同一个模型池，再交给路由或默认转发使用。</p>
        <NButton type="primary" @click="handleAdd">+ 新增模型映射</NButton>
      </div>
    </template>

    <NGrid :x-gap="16" :y-gap="16" :cols="3" style="margin-top: 16px">
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

          <div class="mc-desc" v-if="item.description">{{ item.description }}</div>

          <div class="mc-stats" v-if="item.channels">
            <span>渠道 <span class="num">{{ item.channels.length }}</span></span>
          </div>
          <div class="mc-channels" v-if="item.channels && item.channels.length > 0">
            <div
              v-for="c in item.channels.slice(0, 3)"
              :key="c.id"
              class="channel-badge"
              :class="{ healthy: c.provider_health === 'healthy' }"
            >
              <span class="cb-name">{{ c.provider_name }}</span>
              <span class="cb-models" v-if="c.selected_models && c.selected_models.length > 0">
                {{ channelModelSummary(c.selected_models) }}
              </span>
            </div>
            <NTag v-if="item.channels.length > 3" size="tiny" round>
              +{{ item.channels.length - 3 }}
            </NTag>
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

          <div class="mc-actions">
            <NButton size="tiny" quaternary @click="handleEdit(item)">编辑</NButton>
            <NButton size="tiny" quaternary type="error" @click="handleDelete(item)">删除</NButton>
          </div>
        </NCard>
      </NGi>
    </NGrid>

    <template #after>
      <AppFormModal
        v-model:show="showModal"
        :title="editingId ? '编辑模型映射' : '新增模型映射'"
        width="640px"
        :submit-text="editingId ? '保存修改' : '确认添加'"
        :submit-disabled="!formValue.model_name || formValue.selectedProviderIds.length === 0"
        @submit="handleSubmit"
      >
        <NForm :model="formValue" label-placement="left" label-width="90">
        <NFormItem label="模型名称" required>
          <NInput v-model:value="formValue.model_name" placeholder="例如：gpt-4、claude-3-opus" />
        </NFormItem>

        <NFormItem label="关联渠道">
          <div style="width: 100%">
            <div v-if="allProviders.length === 0" style="font-size: 13px; color: #94a3b8; padding: 8px 0">
              暂无可用渠道，请先在「渠道管理」中添加
            </div>
            <div v-else class="channel-list">
              <label
                v-for="p in allProviders"
                :key="p.id"
                class="channel-item"
                :class="{ selected: formValue.selectedProviderIds.includes(p.id) }"
              >
                <input
                  type="checkbox"
                  class="channel-checkbox"
                  :checked="formValue.selectedProviderIds.includes(p.id)"
                  @change="(e: any) => {
                    if (e.target.checked) {
                      formValue.selectedProviderIds.push(p.id);
                    } else {
                      formValue.selectedProviderIds = formValue.selectedProviderIds.filter(id => id !== p.id);
                      const map = { ...formValue.selectedModelsMap };
                      delete map[p.id];
                      formValue.selectedModelsMap = map;
                    }
                  }"
                />
                <div class="channel-info">
                  <span class="channel-name">{{ p.name }}</span>
                  <span class="channel-protocols">
                    <NTag v-for="proto in (p.protocols || [])" :key="proto" size="tiny" round style="margin-right: 2px">
                      {{ proto }}
                    </NTag>
                  </span>
                  <span class="channel-models">{{ (p.models || []).length }} 模型</span>
                  <NTag size="tiny" :type="healthColor(p.health_status)" round>
                    {{ healthLabel(p.health_status) }}
                  </NTag>
                </div>
              </label>
            </div>
          </div>
        </NFormItem>

        <NFormItem v-if="selectedProviders.length > 0" label="选择模型">
          <div style="width: 100%; display: flex; flex-direction: column; gap: 10px">
            <NInput
              v-model:value="modelSearchKeyword"
              placeholder="搜索模型名，点击模型切换选中..."
              clearable
            >
              <template #prefix>
                <NIcon><SearchOutline /></NIcon>
              </template>
            </NInput>

            <div v-for="grp in channelModels" :key="grp.provider_id" class="channel-model-group">
              <div class="cmg-header">
                <span class="cmg-name">{{ grp.provider_name }}</span>
                <span class="cmg-count">已选 {{ (formValue.selectedModelsMap[grp.provider_id] || []).length }}/{{ (selectedProviders.find(p => p.id === grp.provider_id)?.models || []).length }}</span>
              </div>
              <div v-if="grp.models.length === 0" class="cmg-empty">无匹配模型</div>
              <div v-else class="cmg-list">
                <div
                  v-for="m in grp.models"
                  :key="grp.provider_id + '-' + m"
                  class="cmg-item"
                  :class="{ selected: (formValue.selectedModelsMap[grp.provider_id] || []).includes(m) }"
                  @click="toggleModel(grp.provider_id, m)"
                >
                  <div class="cmg-check">
                    <span class="cmg-check-icon">{{ (formValue.selectedModelsMap[grp.provider_id] || []).includes(m) ? '✓' : '' }}</span>
                  </div>
                  <span class="cmg-model">{{ m }}</span>
                  <span class="cmg-remark">外部视为 {{ formValue.model_name || '同模型名' }}</span>
                </div>
              </div>
            </div>

            <div v-if="channelModels.length === 0 && modelSearchKeyword" style="font-size:13px;color:#94a3b8;padding:8px 0">
              无匹配模型
            </div>
          </div>
        </NFormItem>

        <NFormItem label="负载策略">
          <NSelect
            v-model:value="formValue.strategy"
            :options="[
              { label: '轮询 (Round Robin)', value: 'round_robin' },
              { label: '加权轮询 (Weighted)', value: 'weighted' },
              { label: '最少连接 (Least Conn)', value: 'least_conn' },
              { label: '故障转移 (Failover)', value: 'failover' },
            ]"
          />
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
          <NInput v-model:value="formValue.description" placeholder="模型描述，如 '最新 GPT-4 模型，支持多模态'" type="textarea" :rows="2" />
        </NFormItem>

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
      </AppFormModal>
    </template>
  </AppPageShell>
</template>

<style scoped>
/* toolbar overrides — ModelSquareView 使用更紧凑的间距 */
.toolbar {
  margin-bottom: 8px;
}
.toolbar-right {
  gap: 8px;
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

.mc-desc {
  font-size: 13px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 8px;
  line-height: 1.4;
}

.mc-stats {
  font-size: 13px;
  color: var(--text-color-2, #64748b);
  margin-bottom: 6px;
}

.mc-channels {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
}

.channel-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
  color: var(--accent, #0891b2);
  font-weight: 500;
  border: 1px solid rgba(8, 145, 178, 0.15);
}

.channel-badge.healthy {
  background: var(--success-soft, rgba(16, 185, 129, 0.1));
  color: var(--success, #10b981);
  border-color: rgba(16, 185, 129, 0.15);
}

.cb-models {
  font-size: 10px;
  opacity: 0.8;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.mc-caps {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}

.mc-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  padding-top: 10px;
  margin-top: 4px;
}

/* 渠道列表 */
.channel-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow-y: auto;
}

.channel-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  cursor: pointer;
  transition: all 0.15s;
}

.channel-item:hover {
  background: var(--hover-color, #f8fafc);
}

.channel-item.selected {
  border-color: var(--accent, #0891b2);
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
}

.channel-checkbox {
  width: 16px;
  height: 16px;
  cursor: pointer;
  accent-color: var(--accent, #0891b2);
}

.channel-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  flex: 1;
}

.channel-name {
  font-weight: 600;
  font-size: 13px;
  min-width: 60px;
}

.channel-protocols {
  display: flex;
  gap: 2px;
}

.channel-models {
  font-size: 12px;
  color: var(--text-color-2, #64748b);
  font-family: 'JetBrains Mono', 'Consolas', monospace;
}

/* 模型分组 */
.channel-model-group {
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 8px;
  overflow: hidden;
}

.cmg-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #f8fafc;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
}

.cmg-name {
  font-weight: 600;
  font-size: 13px;
}

.cmg-count {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.cmg-empty {
  padding: 12px;
  font-size: 13px;
  color: #94a3b8;
  text-align: center;
}

.cmg-list {
  display: flex;
  flex-direction: column;
}

.cmg-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
  transition: background 0.12s;
  border-bottom: 1px solid #f1f5f9;
}

.cmg-item:last-child {
  border-bottom: none;
}

.cmg-item:hover {
  background: #f8fafc;
}

.cmg-item.selected {
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
}

.cmg-check {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  border: 1.5px solid #cbd5e1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  color: white;
  flex-shrink: 0;
  transition: all 0.12s;
}

.cmg-item.selected .cmg-check {
  background: var(--accent, #0891b2);
  border-color: var(--accent, #0891b2);
}

.cmg-check-icon {
  line-height: 1;
}

.cmg-model {
  font-weight: 600;
  font-size: 13px;
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  flex: 1;
}

.cmg-remark {
  font-size: 11px;
  color: var(--text-color-3, #94a3b8);
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




</style>
