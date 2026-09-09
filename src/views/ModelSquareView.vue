<script setup lang="ts">
import { ref, onMounted, computed, watch } from "vue";
import { useDataChangeSignal } from "../composables/useCrossStoreNotify";
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
  NSteps,
  NStep,
  useMessage,
  useDialog,
} from "naive-ui";
import { SearchOutline } from "@vicons/ionicons5";
import { modelMappingsApi, type CreateModelMappingPayload } from "../api/model-mappings";
import { providersApi } from "../api/providers";
import type { ModelMapping, NewMappingChannel, Provider, SelectedModel } from "../api";
import { formatTokens } from "../utils/format";
import { healthStatusLabel, healthStatusType } from "../utils/health";
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
function createEmptyForm() {
  return {
    model_name: "",
    strategy: "round_robin",
    // 每个渠道选中的模型列表 { provider_id → SelectedModel[] }（勾选了模型的渠道即参与）
    selectedModelsMap: {} as Record<string, SelectedModel[]>,
    max_context_tokens: null as number | null,
    max_output_tokens: null as number | null,
    capabilities: [] as string[],
    description: "",
    enabled: true,
  };
}

const formValue = ref(createEmptyForm());

// 参与渠道 = 已勾选了模型的渠道（由 selectedModelsMap 派生，无需单独勾选）
const selectedProviderIds = computed(() =>
  Object.keys(formValue.value.selectedModelsMap).filter(
    (pid) => (formValue.value.selectedModelsMap[pid] || []).length > 0
  )
);

// 渠道模型区展开状态（默认收起，点击渠道头部展开/收起）
const expandedProviderIds = ref<Record<string, boolean>>({});

function toggleExpand(providerId: string) {
  expandedProviderIds.value = {
    ...expandedProviderIds.value,
    [providerId]: !expandedProviderIds.value[providerId],
  };
}

// 向导当前步骤：0=基础配置（名称+渠道+模型） 1=权重与参数
const currentStep = ref(0);

// 步骤 0 通过后允许进入下一步（名称 + 至少一个渠道勾选了模型）
const step0Valid = computed(
  () => !!formValue.value.model_name.trim() && selectedProviderIds.value.length > 0
);

function nextStep() {
  if (currentStep.value === 0 && !step0Valid.value) return;
  if (currentStep.value < 1) currentStep.value += 1;
}

function prevStep() {
  if (currentStep.value > 0) currentStep.value -= 1;
}

// 加权类策略才展示权重输入（最少连接无需加权）
const isWeightedStrategy = computed(
  () => formValue.value.strategy === "round_robin" || formValue.value.strategy === "weighted"
);

// 某渠道已选中的模型列表（带权重）
function selectedModelsOf(providerId: string): SelectedModel[] {
  return formValue.value.selectedModelsMap[providerId] || [];
}

// 设置模型权重（最小 1）
function setModelWeight(providerId: string, model: string, value: number | null) {
  const map = { ...formValue.value.selectedModelsMap };
  const models = (map[providerId] || []).map((m) =>
    m.name === model ? { ...m, weight: value && value > 0 ? value : 1 } : m
  );
  map[providerId] = models;
  formValue.value.selectedModelsMap = map;
}

// 模型模糊搜索关键字（筛选时匹配渠道自动展开）
const modelSearchKeyword = ref("");

// 是否处于筛选状态（关键字非空）
const isFiltering = computed(() => modelSearchKeyword.value.trim().length > 0);

// 已选渠道的提供者信息（供权重步骤展示）
const selectedProviders = computed(() =>
  allProviders.value.filter((p) => selectedProviderIds.value.includes(p.id))
);

// 各渠道过滤搜索词后的模型列表（缓存，避免模板中重复过滤计算）
const channelFilteredModels = computed(() => {
  const kw = modelSearchKeyword.value.trim().toLowerCase();
  const map: Record<string, string[]> = {};
  for (const p of allProviders.value) {
    const all = p.models || [];
    map[p.id] = kw ? all.filter((m) => m.toLowerCase().includes(kw)) : all;
  }
  return map;
});

// 某渠道过滤搜索词后的模型列表（查表）
function channelModelsOf(providerId: string): string[] {
  return channelFilteredModels.value[providerId] || [];
}

// 某渠道的某模型是否已选中
function isModelSelected(providerId: string, model: string): boolean {
  return selectedModelsOf(providerId).some((sm) => sm.name === model);
}

// 切换选中/取消某个渠道的某个模型（选中时默认权重 1；取消到空时该渠道不再参与）
function toggleModel(providerId: string, model: string) {
  const map = { ...formValue.value.selectedModelsMap };
  if (!map[providerId]) map[providerId] = [];
  const idx = map[providerId].findIndex((m) => m.name === model);
  if (idx >= 0) {
    const rest = map[providerId].filter((m) => m.name !== model);
    if (rest.length === 0) {
      delete map[providerId];
    } else {
      map[providerId] = rest;
    }
  } else {
    map[providerId] = [...map[providerId], { name: model, weight: 1 }];
  }
  formValue.value.selectedModelsMap = map;
}

// 切换选中/取消某个模型能力
function toggleCapability(value: string, checked: boolean) {
  if (checked) {
    formValue.value.capabilities.push(value);
  } else {
    formValue.value.capabilities = formValue.value.capabilities.filter((c) => c !== value);
  }
}

// 完整标签映射（含已下线的 code，兼容旧数据展示）
const capabilityLabelMap: Record<string, string> = {
  thinking: "思考",
  vision: "识图",
  text: "文本",
  code: "代码",
  image_gen: "生图",
  audio: "语音",
};

// 模型能力选项（当前可勾选的能力子集，从标签映射派生，避免重复维护 label）
const capabilityOptions = ["thinking", "vision", "text", "image_gen", "audio"].map((value) => ({
  value,
  label: capabilityLabelMap[value],
}));

// 档位标签 → token 数：k = ×1024，M = ×1024²（如 "128k" → 131072，"1M" → 1048576）
function tokenFromLabel(label: string): number {
  const unit = label.slice(-1).toLowerCase();
  const num = Number(label.slice(0, -1));
  if (unit === "m") return Math.round(num * 1024 * 1024);
  if (unit === "k") return Math.round(num * 1024);
  return Math.round(num);
}

// 上下文 / 最大输出 快捷档位配置（点击填入，再点取消）
type TokenFieldKey = "max_context_tokens" | "max_output_tokens";

interface TokenFieldConfig {
  key: TokenFieldKey;
  label: string;
  options: { label: string; value: number }[];
}

const tokenFields: TokenFieldConfig[] = [
  {
    key: "max_context_tokens",
    label: "上下文",
    options: ["128k", "256k", "512k", "1M"].map((label) => ({
      label,
      value: tokenFromLabel(label),
    })),
  },
  {
    key: "max_output_tokens",
    label: "最大输出",
    options: ["16k", "32k", "64k", "128k"].map((label) => ({
      label,
      value: tokenFromLabel(label),
    })),
  },
];

function toggleTokenValue(key: TokenFieldKey, value: number) {
  const form = formValue.value;
  form[key] = form[key] === value ? null : value;
}

function capabilityLabel(val: string): string {
  return capabilityLabelMap[val] || val;
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
      modelMappingsApi.list(),
      providersApi.list(),
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
  formValue.value = createEmptyForm();
  modelSearchKeyword.value = "";
  expandedProviderIds.value = {};
  currentStep.value = 0;
}

function handleAdd() {
  resetForm();
  showModal.value = true;
}

function handleEdit(row: ModelMapping) {
  editingId.value = row.id;
  // 从 channels 回填 selectedModelsMap（勾选了模型的渠道即参与，由 computed 派生）
  const modelsMap: Record<string, SelectedModel[]> = {};
  for (const c of row.channels || []) {
    if (c.selected_models && c.selected_models.length > 0) {
      modelsMap[c.provider_id] = c.selected_models.map((m) => ({
        name: m.name,
        weight: m.weight || 1,
      }));
    }
  }
  formValue.value = {
    model_name: row.model_name,
    strategy: row.strategy || "round_robin",
    selectedModelsMap: modelsMap,
    max_context_tokens: row.max_context_tokens,
    max_output_tokens: row.max_output_tokens,
    capabilities: row.capabilities || [],
    description: row.description || "",
    enabled: row.enabled,
  };
  // 编辑时默认展开已勾选了模型的渠道（用户可直接看到已选内容）
  const expanded: Record<string, boolean> = {};
  for (const pid of Object.keys(modelsMap)) {
    expanded[pid] = true;
  }
  expandedProviderIds.value = expanded;
  modelSearchKeyword.value = "";
  currentStep.value = 0;
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
        await modelMappingsApi.remove(row.id);
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
    // selectedProviderIds 已保证每个渠道都有选中模型
    const channels: NewMappingChannel[] = selectedProviderIds.value.map((pid) => ({
      provider_id: pid,
      selected_models: selectedModelsOf(pid),
    }));

    const payload: CreateModelMappingPayload = {
      model_name: formValue.value.model_name,
      strategy: formValue.value.strategy,
      max_context_tokens: formValue.value.max_context_tokens,
      max_output_tokens: formValue.value.max_output_tokens,
      capabilities: formValue.value.capabilities.length > 0 ? formValue.value.capabilities : undefined,
      description: formValue.value.description || undefined,
      enabled: formValue.value.enabled,
      channels: channels.length > 0 ? channels : undefined,
    };

    if (editingId.value) {
      const updated = await modelMappingsApi.update(editingId.value, payload);
      const idx = mappings.value.findIndex((m) => m.id === editingId.value);
      if (idx >= 0) mappings.value[idx] = updated;
      message.success("更新成功");
    } else {
      const created = await modelMappingsApi.create(payload);
      mappings.value.unshift(created);
      message.success("创建成功");
    }
    showModal.value = false;
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

// 卡片显示渠道的选中模型摘要
function channelModelSummary(selected: SelectedModel[]): string {
  if (!selected || selected.length === 0) return "";
  const names = selected.map((m) => m.name);
  if (names.length <= 2) return names.join(", ");
  return `${names.slice(0, 2).join(", ")} +${names.length - 2}`;
}

onMounted(loadData);

// 数据流：后端写操作后 emit "groups"/"providers" → 桥接为前端信号 → 此处失效重拉
const groupsSignal = useDataChangeSignal("groups");
const providersSignal = useDataChangeSignal("providers");
watch(
  [groupsSignal, providersSignal],
  () => loadData(),
  { flush: "post" },
);
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
        @submit="handleSubmit"
      >
        <NSteps :current="currentStep" size="small" style="margin-bottom: 16px">
          <NStep title="基础配置" description="名称、渠道与模型" />
          <NStep title="权重与参数" description="模型权重与规格" />
        </NSteps>

        <Transition name="step-fade" mode="out-in">
          <NForm :key="currentStep" :model="formValue" label-placement="left" label-width="90">
            <!-- 步骤 0：基础配置（名称 + 渠道 + 模型） -->
            <template v-if="currentStep === 0">
              <NFormItem label="模型名称" required>
                <NInput v-model:value="formValue.model_name" placeholder="例如：gpt-4、claude-3-opus" />
              </NFormItem>

              <NFormItem label="渠道与模型">
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

                  <div v-if="allProviders.length === 0" style="font-size: 13px; color: #94a3b8; padding: 8px 0">
                    暂无可用渠道，请先在「渠道管理」中添加
                  </div>

                  <div v-else class="channel-list">
                    <div
                      v-for="p in allProviders"
                      :key="p.id"
                      class="channel-item"
                      :class="{
                        selected: selectedProviderIds.includes(p.id),
                        'no-match': isFiltering && channelModelsOf(p.id).length === 0,
                      }"
                    >
                      <div class="channel-item-head" @click="toggleExpand(p.id)">
                        <span class="channel-arrow" :class="{ expanded: expandedProviderIds[p.id] }">▸</span>
                        <div class="channel-info">
                          <span class="channel-name">{{ p.name }}</span>
                          <span class="channel-protocols">
                            <NTag v-for="proto in (p.protocols || [])" :key="proto" size="tiny" round style="margin-right: 2px">
                              {{ proto }}
                            </NTag>
                          </span>
                          <span class="channel-models">{{ (p.models || []).length }} 模型</span>
                          <NTag size="tiny" :type="healthStatusType(p.health_status)" round>
                            {{ healthStatusLabel(p.health_status) }}
                          </NTag>
                        </div>
                        <span
                          v-if="selectedProviderIds.includes(p.id)"
                          class="channel-expand-hint"
                        >已选 {{ selectedModelsOf(p.id).length }} 个</span>
                      </div>

                      <!-- 该渠道的模型勾选区（点击头部展开/收起；筛选时匹配渠道自动展开） -->
                      <div v-if="expandedProviderIds[p.id] || (isFiltering && channelModelsOf(p.id).length > 0)" class="cmg-list">
                        <div
                          v-for="m in channelModelsOf(p.id)"
                          :key="p.id + '-' + m"
                          class="cmg-item"
                          :class="{ selected: isModelSelected(p.id, m) }"
                          @click="toggleModel(p.id, m)"
                        >
                          <div class="cmg-check">
                            <span class="cmg-check-icon">{{ isModelSelected(p.id, m) ? '✓' : '' }}</span>
                          </div>
                          <span class="cmg-model">{{ m }}</span>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </NFormItem>

              <NFormItem label="负载策略">
                <NSelect
                  v-model:value="formValue.strategy"
                  :options="[
                    { label: '加权轮询 (Weighted Round Robin)', value: 'round_robin' },
                    { label: '加权随机 (Weighted Random)', value: 'weighted' },
                    { label: '最少连接 (Least Conn)', value: 'least_conn' },
                  ]"
                />
              </NFormItem>
            </template>

            <!-- 步骤 2：权重与参数 -->
            <template v-else>
              <NFormItem v-if="isWeightedStrategy" label="模型权重">
                <div style="width: 100%; display: flex; flex-direction: column; gap: 10px">
                  <div
                    v-for="grp in selectedProviders"
                    :key="grp.id"
                    class="channel-model-group"
                  >
                    <div class="cmg-header">
                      <span class="cmg-name">{{ grp.name }}</span>
                      <span class="cmg-count">已选 {{ selectedModelsOf(grp.id).length }} 个模型</span>
                    </div>
                    <div v-if="selectedModelsOf(grp.id).length === 0" class="cmg-empty">
                      该渠道未选择模型（将使用模型映射的同名模型，权重 1）
                    </div>
                    <div v-else class="cmg-list">
                      <div v-for="sm in selectedModelsOf(grp.id)" :key="grp.id + '-' + sm.name" class="cmg-item weight-item">
                        <span class="cmg-model">{{ sm.name }}</span>
                        <span class="cmg-weight">
                          <span class="cmg-weight-label">权重</span>
                          <NInputNumber
                            :value="sm.weight"
                            :min="1"
                            size="small"
                            style="width: 72px"
                            @update:value="(v) => setModelWeight(grp.id, sm.name, v)"
                          />
                        </span>
                      </div>
                    </div>
                  </div>
                  <div style="font-size: 12px; color: #94a3b8">
                    权重决定流量分配比例，默认 1；数值越大，被选中的概率/频次越高。
                  </div>
                </div>
              </NFormItem>

              <div class="form-row">
                <NFormItem v-for="field in tokenFields" :key="field.key" :label="field.label" style="flex: 1">
                  <div class="token-field">
                    <NInputNumber v-model:value="formValue[field.key]" placeholder="请输入数值，留空则使用最佳默认值" :min="0" style="width: 100%" />
                    <div class="token-options">
                      <button
                        v-for="opt in field.options"
                        :key="opt.value"
                        type="button"
                        class="token-option"
                        :class="{ active: formValue[field.key] === opt.value }"
                        @click="toggleTokenValue(field.key, opt.value)"
                      >{{ opt.label }}</button>
                    </div>
                  </div>
                </NFormItem>
              </div>

              <NFormItem label="描述">
                <NInput v-model:value="formValue.description" placeholder="模型描述，如 '最新 GPT-4 模型，支持多模态'" type="textarea" :rows="2" />
              </NFormItem>

              <NFormItem label="模型能力">
                <div class="cap-checkboxes">
                  <label v-for="cap in capabilityOptions" :key="cap.value" class="cap-checkbox">
                    <input type="checkbox" :value="cap.value" :checked="formValue.capabilities.includes(cap.value)"
                      @change="(e: any) => toggleCapability(cap.value, e.target.checked)"
                    />
                    {{ cap.label }}
                  </label>
                </div>
              </NFormItem>

              <NFormItem label="启用">
                <NSwitch v-model:value="formValue.enabled" />
              </NFormItem>
            </template>
          </NForm>
        </Transition>

        <template #footer>
          <div class="modal-footer">
            <NButton @click="showModal = false">取消</NButton>
            <div class="modal-footer-right">
              <NButton v-if="currentStep > 0" @click="prevStep">上一步</NButton>
              <template v-if="currentStep < 1">
                <NButton type="primary" :disabled="!step0Valid" @click="nextStep">
                  下一步
                </NButton>
              </template>
              <template v-else>
                <NButton type="primary" :disabled="!step0Valid" @click="handleSubmit">
                  {{ editingId ? '保存修改' : '确认添加' }}
                </NButton>
              </template>
            </div>
          </div>
        </template>
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
  gap: 8px;
  max-height: 280px;
  overflow-y: auto;
  /* 禁止子项被 flex 压缩（否则超出容器高度时行会被压扁裁切，而不是滚动） */
  align-items: stretch;
}

.channel-item {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  transition: all 0.15s;
  overflow: hidden;
  background: var(--card-bg, #ffffff);
}

/* 渠道头部行（信息展示，点击展开/收起模型区） */
.channel-item-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  min-height: 42px;
  cursor: pointer;
  transition: background 0.12s;
  user-select: none;
}

.channel-item-head:hover {
  background: var(--hover-bg, #f8fafc);
}

/* 展开箭头（收起时右指，展开时旋转下指） */
.channel-arrow {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
  transition: transform 0.15s ease;
  flex-shrink: 0;
}

.channel-arrow.expanded {
  transform: rotate(90deg);
}

/* 头部右侧：已选模型数量 */
.channel-expand-hint {
  font-size: 12px;
  font-weight: 600;
  color: var(--accent, #0891b2);
  margin-left: auto;
  flex-shrink: 0;
}

/* 筛选无匹配渠道：压缩为矮条，淡化提示无匹配（无需文字说明） */
.channel-item.no-match {
  opacity: 0.45;
  border-color: var(--border-color, #e2e8f0);
}

.channel-item.no-match .channel-item-head {
  padding: 6px 12px;
  min-height: 0;
  cursor: default;
}

.channel-item.no-match .channel-item-head:hover {
  background: transparent;
}

.channel-item.no-match .channel-info {
  gap: 4px;
}

/* 勾选渠道后内嵌的模型勾选区 */
.channel-item .cmg-list {
  border-top: 1px solid var(--border-color, #e2e8f0);
  background: var(--surface-alt, #f1f5f9);
}

.channel-item .cmg-item {
  padding-left: 20px;
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
  background: var(--surface-alt, #f1f5f9);
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

.cmg-weight {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}

.cmg-weight-label {
  font-size: 12px;
  color: var(--text-color-2, #64748b);
}

.cmg-empty {
  padding: 12px;
  font-size: 13px;
  color: var(--text-color-3, #94a3b8);
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
  border-bottom: 1px solid var(--border-color, #e2e8f0);
}

.cmg-item:last-child {
  border-bottom: none;
}

.cmg-item:hover {
  background: var(--hover-bg, #f8fafc);
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

/* 向导底部按钮（覆盖 AppFormModal 默认 footer） */
.modal-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

/* 右侧导航按钮组（上一步/下一步/确认） */
.modal-footer-right {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* 步骤切换过渡 */
.step-fade-enter-active,
.step-fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.step-fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}

.step-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* 步骤 1：权重行 */
.weight-item {
  justify-content: space-between;
}

/* 上下文 / 最大输出 快捷档位 */
.token-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
}

.token-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.token-option {
  font-size: 12px;
  padding: 2px 10px;
  border-radius: 4px;
  border: 1px solid var(--border-color, #e2e8f0);
  background: transparent;
  color: var(--text-color-2, #64748b);
  cursor: pointer;
  transition: all 0.15s;
}

.token-option:hover {
  border-color: var(--accent, #0891b2);
  color: var(--accent, #0891b2);
}

.token-option.active {
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
  border-color: var(--accent, #0891b2);
  color: var(--accent, #0891b2);
  font-weight: 500;
}
</style>
