<script setup lang="ts">
import { ref, onMounted, computed, watch } from "vue";
import { useDataChangeSignal } from "../composables/useCrossStoreNotify";
import {
  NCard,
  NButton,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NTag,
  NGrid,
  NGi,
  NSteps,
  NStep,
  useMessage,
} from "naive-ui";
import { modelMappingsApi, type CreateModelMappingPayload } from "../api/model-mappings";
import { providersApi } from "../api/providers";
import type { ModelMapping, NewMappingChannel, Provider, SelectedModel } from "../api";
import { formatTokens } from "../utils/format";
import { healthStatusLabel, healthStatusType } from "../utils/health";
import { useConfirm } from "../utils/confirm";
import AppFormModal from "../components/AppFormModal.vue";
import AppPageShell from "../components/AppPageShell.vue";

const message = useMessage();
const { confirm } = useConfirm();

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

// 参与渠道的模型总数（分区统计 + footer 摘要共用）
const totalSelectedModels = computed(() =>
  selectedProviderIds.value.reduce(
    (sum, pid) => sum + (formValue.value.selectedModelsMap[pid] || []).length,
    0
  )
);

// 负载策略说明：让用户不用查文档就知道选哪个
const strategyHint = computed(() => {
  switch (formValue.value.strategy) {
    case "weighted":
      return "按权重随机分配请求，权重越高被选中的概率越大。";
    case "least_conn":
      return "优先选择当前并发请求最少的渠道，适合响应时长差异大的场景。";
    default:
      return "按权重顺序轮询各渠道，权重越高在轮次中出现越频繁。";
  }
});

// footer 常驻摘要：滚到权重/规格深处仍能看到这次在配什么
const modalSummary = computed(() => [
  formValue.value.model_name.trim() || "未命名",
  `${selectedProviderIds.value.length} 渠道`,
  `${totalSelectedModels.value} 模型`,
]);

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
  confirm({
    title: "删除模型映射",
    description: "该模型名将不再指向任何渠道。",
    targetLabel: "模型",
    target: row.model_name,
    impacts: [
      "客户端使用该模型名的请求将无法命中链路",
      "此操作不可撤销",
    ],
    positiveText: "删除",
    destructive: true,
    onConfirm: async () => {
      await modelMappingsApi.remove(row.id);
      mappings.value = mappings.value.filter((m) => m.id !== row.id);
      message.success("删除成功");
    },
    onError: () => message.error("删除失败"),
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
            <span class="mc-channels-count" v-if="item.channels">
              {{ item.channels.length }} 渠道
            </span>
          </div>

          <div class="mc-desc" v-if="item.description">{{ item.description }}</div>

          <div class="mc-specs" v-if="item.max_context_tokens || item.max_output_tokens || item.max_input_tokens">
            <template v-if="item.max_input_tokens">
              <span>输入 <span class="num">{{ formatTokens(item.max_input_tokens) }}</span></span>
              <span class="sep">·</span>
            </template>
            <template v-if="item.max_context_tokens">
              <span>上下文 <span class="num">{{ formatTokens(item.max_context_tokens) }}</span></span>
              <span v-if="item.max_context_tokens && item.max_output_tokens" class="sep">·</span>
            </template>
            <template v-if="item.max_output_tokens">
              <span>输出 <span class="num">{{ formatTokens(item.max_output_tokens) }}</span></span>
            </template>
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
        width="680px"
        :summary="modalSummary"
        @submit="handleSubmit"
      >
        <NSteps :current="currentStep" size="small" class="m-steps">
          <NStep title="基础配置" description="名称、渠道与模型" />
          <NStep title="权重与参数" description="模型权重与规格" />
        </NSteps>

        <Transition name="step-fade" mode="out-in">
          <div :key="currentStep" class="m-body">
            <!-- 步骤 0：模型信息 / 渠道与模型 / 负载策略 -->
            <template v-if="currentStep === 0">
              <section class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">模型信息</span>
                  <span class="m-sec-meta">{{ selectedProviderIds.length }} 渠道 · {{ totalSelectedModels }} 模型</span>
                </div>
                <div class="m-grid" style="--m-cols: 4">
                  <div class="m-field m-span-3">
                    <label class="m-label">模型名称<i class="m-req">*</i></label>
                    <NInput v-model:value="formValue.model_name" placeholder="例如：gpt-4、claude-3-opus" />
                  </div>
                  <div class="m-field">
                    <label class="m-label">启用</label>
                    <div class="m-switch">
                      <NSwitch v-model:value="formValue.enabled" />
                      <span class="m-note">{{ formValue.enabled ? "对外可用" : "已停用" }}</span>
                    </div>
                  </div>
                </div>
              </section>

              <section class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">渠道与模型</span>
                  <NInput
                    v-model:value="modelSearchKeyword"
                    placeholder="搜索模型名"
                    size="small"
                    clearable
                    class="m-sec-search m-sec-action"
                  />
                </div>
                <p class="m-sec-desc">展开渠道并勾选模型，被勾中的渠道即参与本模型的负载均衡。</p>

                <div v-if="allProviders.length === 0" class="m-empty">暂无可用渠道，请先在「渠道管理」中添加</div>

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
              </section>

              <section class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">负载策略</span>
                  <span class="m-sec-meta">多渠道路由方式</span>
                </div>
                <div class="m-grid">
                  <div class="m-field">
                    <NSelect
                      v-model:value="formValue.strategy"
                      :options="[
                        { label: '加权轮询 (Weighted Round Robin)', value: 'round_robin' },
                        { label: '加权随机 (Weighted Random)', value: 'weighted' },
                        { label: '最少连接 (Least Conn)', value: 'least_conn' },
                      ]"
                    />
                  </div>
                </div>
                <p class="m-sec-desc">{{ strategyHint }}</p>
              </section>
            </template>

            <!-- 步骤 1：权重 / 规格 / 描述与能力 -->
            <template v-else>
              <section v-if="isWeightedStrategy" class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">模型权重</span>
                  <span class="m-sec-meta">{{ selectedProviderIds.length }} 渠道 · {{ totalSelectedModels }} 模型</span>
                </div>
                <div class="m-list">
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
                </div>
                <p class="m-sec-desc">
                  权重决定流量分配比例，默认 1；数值越大，被选中的概率/频次越高。
                </p>
              </section>

              <section class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">模型规格</span>
                  <span class="m-sec-meta">留空则使用最佳默认值</span>
                </div>
                <div class="m-grid">
                  <div v-for="field in tokenFields" :key="field.key" class="m-field">
                    <label class="m-label">{{ field.label }}</label>
                    <div class="token-field">
                      <NInputNumber v-model:value="formValue[field.key]" placeholder="留空用默认值" :min="0" style="width: 100%" />
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
                  </div>
                </div>
              </section>

              <section class="m-section">
                <div class="m-sec-head">
                  <span class="m-sec-title">描述与能力</span>
                </div>
                <div class="m-field">
                  <NInput v-model:value="formValue.description" placeholder="模型描述，如 '最新 GPT-4 模型，支持多模态'" type="textarea" :rows="2" />
                </div>
                <div class="m-field">
                  <label class="m-label">模型能力</label>
                  <div class="cap-checkboxes">
                    <label v-for="cap in capabilityOptions" :key="cap.value" class="cap-checkbox">
                      <input type="checkbox" :value="cap.value" :checked="formValue.capabilities.includes(cap.value)"
                        @change="(e: any) => toggleCapability(cap.value, e.target.checked)"
                      />
                      {{ cap.label }}
                    </label>
                  </div>
                </div>
              </section>
            </template>
          </div>
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
  border-radius: var(--radius-sm, 6px);
  transition: border-color var(--transition);
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
}

.model-card:hover {
  border-color: var(--muted, #a3a3a3);
}

.model-card.disabled {
  opacity: 0.5;
}

.mc-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.mc-name-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mc-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  letter-spacing: -0.01em;
}

.mc-desc {
  font-size: 13px;
  color: var(--fg-2, #171717);
  margin-bottom: 8px;
  line-height: 1.4;
}

.mc-stats {
  font-size: 12px;
  color: var(--muted, #737373);
  margin-bottom: 6px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
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
  border-radius: 999px;
  background: color-mix(in srgb, var(--fg) 8%, var(--surface));
  color: var(--fg);
  font-weight: 600;
  border: 1px solid color-mix(in srgb, var(--fg) 14%, var(--surface));
}

.channel-badge.healthy {
  background: color-mix(in srgb, var(--success) 12%, var(--surface));
  color: var(--success);
  border-color: color-mix(in srgb, var(--success) 20%, var(--surface));
}

.cb-models {
  font-size: 10px;
  opacity: 0.75;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

.mc-specs {
  font-size: 12px;
  color: var(--muted, #737373);
  margin-bottom: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

.sep {
  color: var(--border, #e5e5e5);
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
  border-top: 1px solid var(--border-soft, #ededed);
  padding-top: 10px;
  margin-top: 4px;
}

/* 渠道列表 */
.channel-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow-y: auto;
  align-items: stretch;
}

.channel-item {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  border-radius: var(--radius-sm, 6px);
  border: 1px solid var(--border, #e5e5e5);
  transition: border-color var(--transition);
  overflow: hidden;
  background: var(--card-bg, #ffffff);
}

.channel-item:hover {
  border-color: var(--muted, #a3a3a3);
}

/* 渠道头部行（信息展示，点击展开/收起模型区） */
.channel-item-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  min-height: 40px;
  cursor: pointer;
  transition: background var(--transition);
  user-select: none;
}

.channel-item-head:hover {
  background: var(--surface-alt, #fafafa);
}

/* 展开箭头（收起时右指，展开时旋转下指） */
.channel-arrow {
  font-size: 12px;
  color: var(--muted, #737373);
  transition: transform var(--transition);
  flex-shrink: 0;
}

.channel-arrow.expanded {
  transform: rotate(90deg);
}

/* 头部右侧：已选模型数量 */
.channel-expand-hint {
  font-size: 12px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin-left: auto;
  flex-shrink: 0;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

/* 筛选无匹配渠道：压缩为矮条，淡化提示无匹配 */
.channel-item.no-match {
  opacity: 0.4;
  border-color: var(--border, #e5e5e5);
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
  border-top: 1px solid var(--border-soft, #ededed);
  background: var(--surface-alt, #fafafa);
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
  color: var(--fg, #0a0a0a);
}

.channel-protocols {
  display: flex;
  gap: 2px;
}

.channel-models {
  font-size: 12px;
  color: var(--muted, #737373);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

/* 模型分组 */
.channel-model-group {
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-sm, 6px);
  overflow: hidden;
}

.cmg-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: var(--surface-alt, #fafafa);
  border-bottom: 1px solid var(--border-soft, #ededed);
}

.cmg-name {
  font-weight: 600;
  font-size: 13px;
  color: var(--fg, #0a0a0a);
}

.cmg-count {
  font-size: 12px;
  color: var(--muted, #737373);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

.cmg-weight {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}

.cmg-weight-label {
  font-size: 12px;
  color: var(--muted, #737373);
}

.cmg-empty {
  padding: 12px;
  font-size: 13px;
  color: var(--muted, #737373);
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
  transition: background var(--transition);
  border-bottom: 1px solid var(--border-soft, #ededed);
}

.cmg-item:last-child {
  border-bottom: none;
}

.cmg-item:hover {
  background: var(--surface-alt, #fafafa);
}

.cmg-item.selected {
  background: color-mix(in srgb, var(--accent) 6%, transparent);
}

.cmg-check {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--border, #e5e5e5);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  color: white;
  flex-shrink: 0;
  transition: all var(--transition);
}

.cmg-item.selected .cmg-check {
  background: var(--fg, #0a0a0a);
  border-color: var(--fg, #0a0a0a);
}

.cmg-check-icon {
  line-height: 1;
}

.cmg-model {
  font-weight: 600;
  font-size: 13px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  flex: 1;
  color: var(--fg-2, #171717);
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
  color: var(--fg-2, #171717);
}

/* 向导底部按钮（覆盖 AppFormModal 默认 footer） */
/* 步骤条：固定在滚动区之外，切步骤时位置不跳动 */
.m-steps {
  margin-bottom: 16px;
}

.modal-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--border-soft, #ededed);
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
  transition: opacity var(--transition), transform var(--transition);
}

.step-fade-enter-from {
  opacity: 0;
  transform: translateY(4px);
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
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px solid var(--border, #e5e5e5);
  background: transparent;
  color: var(--muted, #737373);
  cursor: pointer;
  transition: all var(--transition);
  font-weight: 500;
}

.token-option:hover {
  border-color: var(--fg, #0a0a0a);
  color: var(--fg, #0a0a0a);
}

.token-option.active {
  background: var(--fg, #0a0a0a);
  border-color: var(--fg, #0a0a0a);
  color: var(--surface, #ffffff);
  font-weight: 600;
}
</style>
