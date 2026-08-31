<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { NButton, NInput, NTag, NModal, useMessage, useDialog } from "naive-ui";
import { api, type Preset, type AgentTypeInfo } from "../api";

const message = useMessage();
const dialog = useDialog();

// Agent Tab（对齐 cc-switch AppType，当前 5 个）
const agentTypes = ref<AgentTypeInfo[]>([]);
const activeTab = ref("claude_code");

// 数据
const presets = ref<Preset[]>([]);
const loading = ref(false);

// 新建/编辑
const showModal = ref(false);
const editingId = ref<string | null>(null);
const formName = ref("");
const formSettings = ref("");

// 初始化：加载 agent 类型，默认选第一个
async function loadAgentTypes() {
  try {
    agentTypes.value = await api.listAgentTypes();
    if (agentTypes.value.length > 0) {
      activeTab.value = agentTypes.value[0].id;
    }
  } catch (e: any) {
    message.error(e?.message || "加载 Agent 类型失败");
  }
}

async function loadPresets() {
  loading.value = true;
  try {
    presets.value = await api.listPresets(activeTab.value);
  } catch (e: any) {
    message.error(e?.message || "加载预设失败");
  } finally {
    loading.value = false;
  }
}

const currentPreset = computed(() => presets.value.find((p) => p.is_active) || null);

const tabLabel = (id: string) =>
  agentTypes.value.find((t) => t.id === id)?.name || id;

function openAdd() {
  editingId.value = null;
  formName.value = "";
  formSettings.value = "";
  showModal.value = true;
}

function openEdit(preset: Preset) {
  editingId.value = preset.id;
  formName.value = preset.name;
  formSettings.value = JSON.stringify(preset.settings_config, null, 2);
  showModal.value = true;
}

async function save() {
  if (!formName.value.trim()) {
    message.warning("请输入预设名称");
    return;
  }
  let settings: Record<string, unknown>;
  try {
    settings = JSON.parse(formSettings.value || "{}");
  } catch {
    message.error("配置 JSON 格式错误");
    return;
  }
  try {
    if (editingId.value) {
      await api.updatePreset(editingId.value, {
        name: formName.value.trim(),
        settings_config: settings,
      });
      message.success("已更新");
    } else {
      await api.createPreset({
        name: formName.value.trim(),
        agent_type: activeTab.value,
        settings_config: settings,
      });
      message.success("已创建");
    }
    showModal.value = false;
    await loadPresets();
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activate(preset: Preset) {
  try {
    const result = await api.switchPreset(activeTab.value, preset.id);
    message.success(`已切换到「${preset.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadPresets();
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function remove(preset: Preset) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除预设「${preset.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deletePreset(preset.id);
        message.success("已删除");
        await loadPresets();
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}

// Tab 切换时重新加载
watch(activeTab, () => {
  loadPresets();
});

onMounted(async () => {
  await loadAgentTypes();
  await loadPresets();
});
</script>

<template>
  <div class="presets-page">
    <!-- Agent Tab -->
    <div class="agent-tabs">
      <button
        v-for="t in agentTypes"
        :key="t.id"
        class="agent-tab"
        :class="{ active: activeTab === t.id }"
        @click="activeTab = t.id"
      >
        {{ t.name }}
      </button>
    </div>

    <div class="page-header">
      <div>
        <h2>{{ tabLabel(activeTab) }} 预设</h2>
        <p class="subtitle">
          {{
            currentPreset
              ? `当前激活：${currentPreset.name}`
              : "未激活任何预设（live 配置保持原状）"
          }}
        </p>
      </div>
      <NButton type="primary" @click="openAdd">+ 新建预设</NButton>
    </div>

    <!-- 预设列表 -->
    <div v-if="loading" class="empty-desc">加载中…</div>
    <div v-else-if="presets.length === 0" class="empty-state">
      <p class="empty-desc">暂无预设，点击右上角「新建预设」创建</p>
    </div>
    <div v-else class="preset-grid">
      <div
        v-for="p in presets"
        :key="p.id"
        class="preset-card"
        :class="{ active: p.is_active }"
      >
        <div class="preset-card-head">
          <span class="preset-name">{{ p.name }}</span>
          <NTag v-if="p.is_active" type="success" size="small">当前</NTag>
        </div>
        <div class="preset-actions">
          <NButton size="small" type="primary" ghost @click="activate(p)">
            {{ p.is_active ? "重新激活" : "激活" }}
          </NButton>
          <NButton size="small" @click="openEdit(p)">编辑</NButton>
          <NButton size="small" type="error" ghost @click="remove(p)">删除</NButton>
        </div>
      </div>
    </div>

    <!-- 新建/编辑弹窗 -->
    <NModal v-model:show="showModal" preset="card" :title="editingId ? '编辑预设' : '新建预设'" style="width: 560px">
      <div class="form-item">
        <label>名称</label>
        <NInput v-model:value="formName" placeholder="预设名称" />
      </div>
      <div class="form-item">
        <label>配置（JSON，切换时注入网关信息后写入 live）</label>
        <NInput v-model:value="formSettings" type="textarea" :rows="12" placeholder='{"model": "gpt-5.6-sol"}' />
      </div>
      <template #footer>
        <div class="modal-actions">
          <NButton size="small" @click="showModal = false">取消</NButton>
          <NButton size="small" type="primary" @click="save">保存</NButton>
        </div>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.presets-page {
  width: 100%;
}

.agent-tabs {
  display: flex;
  justify-content: center;
  gap: 6px;
  margin-bottom: 24px;
  padding: 4px;
  background: var(--surface-alt, #f1f5f9);
  border-radius: 12px;
}

.agent-tab {
  padding: 10px 20px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  min-width: 100px;
  transition: all 0.15s;
}

.agent-tab:hover {
  background: var(--hover-bg, #e2e8f0);
}

.agent-tab.active {
  background: #18a058;
  color: #fff;
  font-weight: 600;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 16px;
}

.page-header h2 {
  margin: 0 0 4px;
  font-size: 18px;
}

.subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--muted, #94a3b8);
}

.preset-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 12px;
}

.preset-card {
  padding: 14px;
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 10px;
  background: var(--card-bg, #fff);
  transition: all 0.15s;
}

.preset-card.active {
  border-color: #18a058;
  box-shadow: 0 0 0 1px rgba(24, 160, 88, 0.3);
}

.preset-card-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.preset-name {
  font-weight: 600;
  font-size: 14px;
}

.preset-actions {
  display: flex;
  gap: 6px;
}

.empty-state {
  padding: 48px 0;
  text-align: center;
}

.empty-desc {
  color: var(--muted, #94a3b8);
  font-size: 13px;
}

.form-item {
  margin-bottom: 12px;
}

.form-item label {
  display: block;
  font-size: 13px;
  margin-bottom: 6px;
  color: var(--muted, #64748b);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
