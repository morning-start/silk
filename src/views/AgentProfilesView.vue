<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import {
  NButton,
  NInput,
  NTag,
  NSelect,
  useMessage,
  useDialog,
  NModal,
} from "naive-ui";
import { api, type Profile, type AgentType, type SwitchResult, type ModelListingItem } from "../api";

const message = useMessage();
const dialog = useDialog();

interface AgentTab {
  type: AgentType;
  label: string;
  description: string;
}

const agentTabs: AgentTab[] = [
  { type: "claude_code", label: "Claude Code", description: "4 角色模型配置" },
  { type: "opencode", label: "OpenCode", description: "按来源分组展示" },
  { type: "codex", label: "Codex", description: "" },
  { type: "gemini_cli", label: "Gemini CLI", description: "" },
  { type: "hermes", label: "Hermes", description: "" },
];

const activeTab = ref<AgentType>("claude_code");

// 数据
const allModels = ref<ModelListingItem[]>([]);
const profiles = ref<Profile[]>([]);
const loading = ref(false);

async function loadData() {
  loading.value = true;
  try {
    const [models, pr] = await Promise.all([
      api.listAllModels(),
      api.listProfiles(activeTab.value),
    ]);
    allModels.value = models;
    profiles.value = pr;
  } catch (e: any) {
    message.error(e?.message || "加载数据失败");
  } finally {
    loading.value = false;
  }
}

// 切换 Tab 时重新加载 Profile
watch(activeTab, (newTab) => {
  if (newTab) {
    loadProfiles(newTab);
  }
});

async function loadProfiles(agentType: AgentType) {
  try {
    profiles.value = await api.listProfiles(agentType);
  } catch (e: any) {
    message.error(e?.message || "加载配置失败");
  }
}

onMounted(loadData);

// ========================================================================
// Claude Code — 4 角色 (Sonnet / Opus / Fable / Haiku)
// ========================================================================

const ROLE_LABELS: Record<string, string> = {
  sonnet: "Sonnet",
  opus: "Opus",
  fable: "Fable",
  haiku: "Haiku",
};

const ROLE_ORDER = ["sonnet", "opus", "fable", "haiku"] as const;

interface ClaudeRoles {
  sonnet: string;
  opus: string;
  fable: string;
  haiku: string;
}

const claudeProfiles = computed(() =>
  profiles.value.filter((p) => p.agent_type === "claude_code")
);

// 解析 Claude Code 配置的 roles
function parseClaudeRoles(configJson: string): ClaudeRoles {
  try {
    const parsed = JSON.parse(configJson);
    const roles = parsed.roles || {};
    return {
      sonnet: roles.sonnet || "",
      opus: roles.opus || "",
      fable: roles.fable || "",
      haiku: roles.haiku || "",
    };
  } catch {
    return { sonnet: "", opus: "", fable: "", haiku: "" };
  }
}

// 全量模型选择下拉选项（后端统一返回，silk 在前，其余按渠道排序）
const allModelOptions = computed(() =>
  allModels.value.map((item) => ({
    label: item.model_mapping_id
      ? `${item.id} (模型池)`
      : `${item.id} (${item.owned_by})`,
    value: item.model_mapping_id ?? item.id,
  }))
);

function getModelName(value: string): string {
  const found = allModels.value.find(
    (m) => (m.model_mapping_id ?? m.id) === value
  );
  return found?.id ?? value;
}

// 新建/编辑
const showClaudeModal = ref(false);
const editingClaudeId = ref<string | null>(null);
const claudeFormName = ref("");
const claudeFormRoles = ref<ClaudeRoles>({
  sonnet: "",
  opus: "",
  fable: "",
  haiku: "",
});

function openAddClaude() {
  editingClaudeId.value = null;
  claudeFormName.value = "";
  claudeFormRoles.value = { sonnet: "", opus: "", fable: "", haiku: "" };
  showClaudeModal.value = true;
}

function openEditClaude(profile: Profile) {
  editingClaudeId.value = profile.id;
  claudeFormName.value = profile.name;
  claudeFormRoles.value = parseClaudeRoles(profile.config_json);
  showClaudeModal.value = true;
}

async function saveClaudeConfig() {
  if (!claudeFormName.value.trim()) {
    message.warning("请输入配置名称");
    return;
  }
  const emptyRole = (Object.keys(claudeFormRoles.value) as (keyof ClaudeRoles)[]).find(
    (k) => !claudeFormRoles.value[k]
  );
  if (emptyRole) {
    message.warning(`请为 ${ROLE_LABELS[emptyRole]} 选择一个模型`);
    return;
  }

  const configJson = JSON.stringify({
    roles: { ...claudeFormRoles.value },
  });

  try {
    if (editingClaudeId.value) {
      await api.updateProfile(editingClaudeId.value, {
        name: claudeFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已更新");
    } else {
      await api.createProfile({
        agent_type: "claude_code",
        name: claudeFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已创建");
    }
    showClaudeModal.value = false;
    await loadProfiles("claude_code");
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activateClaude(profile: Profile) {
  try {
    const result: SwitchResult = await api.switchProfile("claude_code", profile.id);
    message.success(`已切换到「${profile.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadProfiles("claude_code");
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function deleteClaude(profile: Profile) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除配置「${profile.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteProfile(profile.id);
        message.success("已删除");
        await loadProfiles("claude_code");
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}

// ========================================================================
// OpenCode — Profile 卡片 + 按 owned_by 分组的模型多选（累加模式）
// ========================================================================

interface ModelOwnerGroup {
  owner: string;
  ownerLabel: string;
  models: string[];
}

const modelOwnerGroups = computed<ModelOwnerGroup[]>(() => {
  const groups: ModelOwnerGroup[] = [];
  const byOwner = new Map<string, string[]>();

  for (const item of allModels.value) {
    const list = byOwner.get(item.owned_by);
    if (list) {
      list.push(item.id);
    } else {
      byOwner.set(item.owned_by, [item.id]);
    }
  }

  for (const [owner, models] of byOwner) {
    const label = owner === "silk" ? "模型池 (Silk)" : owner;
    groups.push({ owner, ownerLabel: label, models });
  }

  return groups;
});

const opencodeProfiles = computed(() =>
  profiles.value.filter((p) => p.agent_type === "opencode")
);

// 解析 OpenCode 配置的 enabled_models
function parseOpenCodeEnabled(configJson: string): Record<string, string[]> {
  try {
    const parsed = JSON.parse(configJson);
    return parsed.enabled_models || {};
  } catch {
    return {};
  }
}

const showOpenCodeModal = ref(false);
const editingOpenCodeId = ref<string | null>(null);
const openCodeFormName = ref("");
const openCodeFormEnabled = ref<Record<string, string[]>>({});

function openAddOpenCode() {
  editingOpenCodeId.value = null;
  openCodeFormName.value = "";
  openCodeFormEnabled.value = {};
  showOpenCodeModal.value = true;
}

function openEditOpenCode(profile: Profile) {
  editingOpenCodeId.value = profile.id;
  openCodeFormName.value = profile.name;
  openCodeFormEnabled.value = parseOpenCodeEnabled(profile.config_json);
  showOpenCodeModal.value = true;
}

function toggleOpenCodeModel(owner: string, modelId: string) {
  if (!openCodeFormEnabled.value[owner]) {
    openCodeFormEnabled.value[owner] = [];
  }
  const list = openCodeFormEnabled.value[owner];
  const idx = list.indexOf(modelId);
  if (idx >= 0) {
    list.splice(idx, 1);
  } else {
    list.push(modelId);
  }
}

function selectAllInGroup(owner: string, models: string[]) {
  openCodeFormEnabled.value[owner] = [...models];
}

function deselectAllInGroup(owner: string) {
  openCodeFormEnabled.value[owner] = [];
}

async function saveOpenCodeConfig() {
  if (!openCodeFormName.value.trim()) {
    message.warning("请输入配置名称");
    return;
  }
  const configJson = JSON.stringify({
    _silk_provider_id: openCodeFormName.value.trim(),
    enabled_models: openCodeFormEnabled.value,
  });

  try {
    if (editingOpenCodeId.value) {
      await api.updateProfile(editingOpenCodeId.value, {
        name: openCodeFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已更新");
    } else {
      await api.createProfile({
        agent_type: "opencode",
        name: openCodeFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已创建");
    }
    showOpenCodeModal.value = false;
    await loadProfiles("opencode");
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activateOpenCode(profile: Profile) {
  try {
    const result: SwitchResult = await api.switchProfile("opencode", profile.id);
    message.success(`已切换到「${profile.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadProfiles("opencode");
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function deleteOpenCode(profile: Profile) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除配置「${profile.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteProfile(profile.id);
        message.success("已删除");
        await loadProfiles("opencode");
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}

function openCodeModelCount(profile: Profile): number {
  const enabled = parseOpenCodeEnabled(profile.config_json);
  return Object.values(enabled).reduce((s, v) => s + v.length, 0);
}

// ========================================================================
// Codex — TOML 配置（model_provider / model / wire_api）
// ========================================================================

interface CodexConfig {
  model_provider: string;
  model: string;
  wire_api: string;
}

const codexProfiles = computed(() =>
  profiles.value.filter((p) => p.agent_type === "codex")
);

// 宽松解析 TOML 顶层键值
function parseCodexConfig(configToml: string): CodexConfig {
  const out: CodexConfig = { model_provider: "", model: "", wire_api: "responses" };
  for (const line of configToml.split("\n")) {
    const m = line.match(/^\s*([\w_]+)\s*=\s*"([^"]*)"\s*$/);
    if (m && (m[1] === "model_provider" || m[1] === "model" || m[1] === "wire_api")) {
      out[m[1]] = m[2];
    }
  }
  return out;
}

function buildCodexToml(cfg: CodexConfig): string {
  const lines = [
    `model_provider = "${cfg.model_provider}"`,
    `model = "${cfg.model}"`,
    `wire_api = "${cfg.wire_api}"`,
  ];
  return lines.join("\n");
}

const showCodexModal = ref(false);
const editingCodexId = ref<string | null>(null);
const codexFormName = ref("");
const codexForm = ref<CodexConfig>({ model_provider: "custom", model: "", wire_api: "responses" });

function openAddCodex() {
  editingCodexId.value = null;
  codexFormName.value = "";
  codexForm.value = { model_provider: "custom", model: "", wire_api: "responses" };
  showCodexModal.value = true;
}

function openEditCodex(profile: Profile) {
  editingCodexId.value = profile.id;
  codexFormName.value = profile.name;
  codexForm.value = parseCodexConfig(profile.config_json);
  showCodexModal.value = true;
}

async function saveCodexConfig() {
  if (!codexFormName.value.trim()) {
    message.warning("请输入配置名称");
    return;
  }
  if (!codexForm.value.model_provider.trim()) {
    message.warning("请输入 Model Provider");
    return;
  }
  if (!codexForm.value.model.trim()) {
    message.warning("请选择模型");
    return;
  }
  const configJson = buildCodexToml(codexForm.value);

  try {
    if (editingCodexId.value) {
      await api.updateProfile(editingCodexId.value, {
        name: codexFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已更新");
    } else {
      await api.createProfile({
        agent_type: "codex",
        name: codexFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已创建");
    }
    showCodexModal.value = false;
    await loadProfiles("codex");
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activateCodex(profile: Profile) {
  try {
    const result: SwitchResult = await api.switchProfile("codex", profile.id);
    message.success(`已切换到「${profile.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadProfiles("codex");
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function deleteCodex(profile: Profile) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除配置「${profile.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteProfile(profile.id);
        message.success("已删除");
        await loadProfiles("codex");
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}

// ========================================================================
// Gemini CLI — env 配置（GEMINI_MODEL 等）
// ========================================================================

const geminiProfiles = computed(() =>
  profiles.value.filter((p) => p.agent_type === "gemini_cli")
);

function parseGeminiConfig(configJson: string): string {
  try {
    const parsed = JSON.parse(configJson);
    return parsed.GEMINI_MODEL || "";
  } catch {
    return "";
  }
}

const showGeminiModal = ref(false);
const editingGeminiId = ref<string | null>(null);
const geminiFormName = ref("");
const geminiFormModel = ref("");

function openAddGemini() {
  editingGeminiId.value = null;
  geminiFormName.value = "";
  geminiFormModel.value = "";
  showGeminiModal.value = true;
}

function openEditGemini(profile: Profile) {
  editingGeminiId.value = profile.id;
  geminiFormName.value = profile.name;
  geminiFormModel.value = parseGeminiConfig(profile.config_json);
  showGeminiModal.value = true;
}

async function saveGeminiConfig() {
  if (!geminiFormName.value.trim()) {
    message.warning("请输入配置名称");
    return;
  }
  if (!geminiFormModel.value) {
    message.warning("请选择模型");
    return;
  }
  const configJson = JSON.stringify({ GEMINI_MODEL: geminiFormModel.value });

  try {
    if (editingGeminiId.value) {
      await api.updateProfile(editingGeminiId.value, {
        name: geminiFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已更新");
    } else {
      await api.createProfile({
        agent_type: "gemini_cli",
        name: geminiFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已创建");
    }
    showGeminiModal.value = false;
    await loadProfiles("gemini_cli");
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activateGemini(profile: Profile) {
  try {
    const result: SwitchResult = await api.switchProfile("gemini_cli", profile.id);
    message.success(`已切换到「${profile.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadProfiles("gemini_cli");
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function deleteGemini(profile: Profile) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除配置「${profile.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteProfile(profile.id);
        message.success("已删除");
        await loadProfiles("gemini_cli");
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}

// ========================================================================
// Hermes — YAML 配置（_silk_provider_id + models 列表）
// ========================================================================

const hermesProfiles = computed(() =>
  profiles.value.filter((p) => p.agent_type === "hermes")
);

// Hermes 的 config_json 是 YAML（JSON 文本是合法 YAML 子集），models 为 dict
function parseHermesModels(configYaml: string): string[] {
  try {
    const parsed = JSON.parse(configYaml);
    const models = parsed.models;
    if (models && typeof models === "object" && !Array.isArray(models)) {
      return Object.keys(models);
    }
    return [];
  } catch {
    return [];
  }
}

function buildHermesYaml(name: string, modelIds: string[]): string {
  const models: Record<string, object> = {};
  for (const id of modelIds) {
    models[id] = {};
  }
  return JSON.stringify({ _silk_provider_id: name, models });
}

const showHermesModal = ref(false);
const editingHermesId = ref<string | null>(null);
const hermesFormName = ref("");
const hermesFormModels = ref<string[]>([]);

function openAddHermes() {
  editingHermesId.value = null;
  hermesFormName.value = "";
  hermesFormModels.value = [];
  showHermesModal.value = true;
}

function openEditHermes(profile: Profile) {
  editingHermesId.value = profile.id;
  hermesFormName.value = profile.name;
  hermesFormModels.value = parseHermesModels(profile.config_json);
  showHermesModal.value = true;
}

async function saveHermesConfig() {
  if (!hermesFormName.value.trim()) {
    message.warning("请输入配置名称");
    return;
  }
  if (hermesFormModels.value.length === 0) {
    message.warning("请至少选择一个模型");
    return;
  }
  const configJson = buildHermesYaml(hermesFormName.value.trim(), hermesFormModels.value);

  try {
    if (editingHermesId.value) {
      await api.updateProfile(editingHermesId.value, {
        name: hermesFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已更新");
    } else {
      await api.createProfile({
        agent_type: "hermes",
        name: hermesFormName.value.trim(),
        config_json: configJson,
      });
      message.success("已创建");
    }
    showHermesModal.value = false;
    await loadProfiles("hermes");
  } catch (e: any) {
    message.error(e?.message || "操作失败");
  }
}

async function activateHermes(profile: Profile) {
  try {
    const result: SwitchResult = await api.switchProfile("hermes", profile.id);
    message.success(`已切换到「${profile.name}」`);
    for (const w of result.warnings) {
      message.warning(w);
    }
    await loadProfiles("hermes");
  } catch (e: any) {
    message.error(e?.message || "切换失败");
  }
}

function deleteHermes(profile: Profile) {
  dialog.warning({
    title: "确认删除",
    content: `确定要删除配置「${profile.name}」吗？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteProfile(profile.id);
        message.success("已删除");
        await loadProfiles("hermes");
      } catch (e: any) {
        message.error(e?.message || "删除失败");
      }
    },
  });
}
</script>

<template>
  <div class="profiles-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">预设管理</h2>
      </div>
    </div>

    <!-- Agent 选项卡 -->
    <div class="agent-tabs">
      <button
        v-for="tab in agentTabs"
        :key="tab.type"
        class="agent-tab"
        :class="{ active: activeTab === tab.type }"
        @click="activeTab = tab.type"
      >
        <span class="agent-tab-label">{{ tab.label }}</span>
        <span class="agent-tab-desc">{{ tab.description }}</span>
      </button>
    </div>

    <!-- ================================================================ -->
    <!-- Claude Code                                                       -->
    <!-- ================================================================ -->
    <div v-if="activeTab === 'claude_code'" class="tab-content">
      <div v-if="loading" class="loading-state">加载模型中…</div>

      <template v-else>
        <div class="claude-configs">
          <div
            v-for="profile in claudeProfiles"
            :key="profile.id"
            class="config-card"
            :class="{ active: profile.is_active }"
          >
            <div class="config-card-header">
              <div class="config-card-name">
                <span class="config-name-text">{{ profile.name }}</span>
                <NTag v-if="profile.is_active" size="tiny" type="success">当前</NTag>
              </div>
            </div>

            <div class="role-list">
              <div v-for="role in ROLE_ORDER" :key="role" class="role-row">
                <span class="role-label">{{ ROLE_LABELS[role] }}</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ getModelName(parseClaudeRoles(profile.config_json)[role]) }}</span>
              </div>
            </div>

            <div class="config-card-actions">
              <NButton
                v-if="!profile.is_active"
                size="tiny"
                type="primary"
                @click="activateClaude(profile)"
              >
                激活
              </NButton>
              <NButton size="tiny" quaternary @click="openEditClaude(profile)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="deleteClaude(profile)">删除</NButton>
            </div>
          </div>
        </div>

        <div v-if="claudeProfiles.length === 0" class="empty-state">
          <div class="empty-icon" style="opacity: 0.4">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 48px; height: 48px">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
              <circle cx="9" cy="7" r="4"/>
              <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
              <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
            </svg>
          </div>
          <h3 class="empty-title">暂无 Claude Code 配置</h3>
          <p class="empty-desc">为 Sonnet / Opus / Fable / Haiku 四个角色分别指定模型</p>
          <NButton type="primary" size="small" @click="openAddClaude">+ 新建配置</NButton>
        </div>

        <div v-if="claudeProfiles.length > 0" class="add-config-bar">
          <NButton type="primary" size="small" @click="openAddClaude">+ 新建配置</NButton>
        </div>
      </template>
    </div>

    <!-- ================================================================ -->
    <!-- OpenCode — 配置卡片（累加模式）                                   -->
    <!-- ================================================================ -->
    <div v-if="activeTab === 'opencode'" class="tab-content">
      <div v-if="loading" class="loading-state">加载模型中…</div>

      <template v-else>
        <div class="claude-configs">
          <div
            v-for="profile in opencodeProfiles"
            :key="profile.id"
            class="config-card"
            :class="{ active: profile.is_active }"
          >
            <div class="config-card-header">
              <div class="config-card-name">
                <span class="config-name-text">{{ profile.name }}</span>
                <NTag v-if="profile.is_active" size="tiny" type="success">当前</NTag>
              </div>
            </div>

            <div class="role-list">
              <div class="role-row">
                <span class="role-label">模型数</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ openCodeModelCount(profile) }} 个模型</span>
              </div>
            </div>

            <div class="config-card-actions">
              <NButton
                v-if="!profile.is_active"
                size="tiny"
                type="primary"
                @click="activateOpenCode(profile)"
              >
                激活
              </NButton>
              <NButton size="tiny" quaternary @click="openEditOpenCode(profile)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="deleteOpenCode(profile)">删除</NButton>
            </div>
          </div>
        </div>

        <div v-if="opencodeProfiles.length === 0" class="empty-state">
          <div class="empty-icon" style="opacity: 0.4">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 48px; height: 48px">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
              <circle cx="9" cy="7" r="4"/>
              <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
              <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
            </svg>
          </div>
          <h3 class="empty-title">暂无 OpenCode 配置</h3>
          <p class="empty-desc">按来源（模型池 / Provider）勾选可用的模型组合</p>
          <NButton type="primary" size="small" @click="openAddOpenCode">+ 新建配置</NButton>
        </div>

        <div v-if="opencodeProfiles.length > 0" class="add-config-bar">
          <NButton type="primary" size="small" @click="openAddOpenCode">+ 新建配置</NButton>
        </div>
      </template>
    </div>

    <!-- ================================================================ -->
    <!-- Codex — TOML 配置（model_provider / model / wire_api）            -->
    <!-- ================================================================ -->
    <div v-if="activeTab === 'codex'" class="tab-content">
      <div v-if="loading" class="loading-state">加载模型中…</div>

      <template v-else>
        <div class="claude-configs">
          <div
            v-for="profile in codexProfiles"
            :key="profile.id"
            class="config-card"
            :class="{ active: profile.is_active }"
          >
            <div class="config-card-header">
              <div class="config-card-name">
                <span class="config-name-text">{{ profile.name }}</span>
                <NTag v-if="profile.is_active" size="tiny" type="success">当前</NTag>
              </div>
            </div>

            <div class="role-list">
              <div class="role-row">
                <span class="role-label">Provider</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ parseCodexConfig(profile.config_json).model_provider || "custom" }}</span>
              </div>
              <div class="role-row">
                <span class="role-label">Model</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ parseCodexConfig(profile.config_json).model }}</span>
              </div>
              <div class="role-row">
                <span class="role-label">Wire API</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ parseCodexConfig(profile.config_json).wire_api }}</span>
              </div>
            </div>

            <div class="config-card-actions">
              <NButton
                v-if="!profile.is_active"
                size="tiny"
                type="primary"
                @click="activateCodex(profile)"
              >
                激活
              </NButton>
              <NButton size="tiny" quaternary @click="openEditCodex(profile)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="deleteCodex(profile)">删除</NButton>
            </div>
          </div>
        </div>

        <div v-if="codexProfiles.length === 0" class="empty-state">
          <div class="empty-icon" style="opacity: 0.4">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 48px; height: 48px">
              <circle cx="12" cy="12" r="9"/>
              <path d="M8 12h8"/>
              <path d="M12 8v8"/>
            </svg>
          </div>
          <h3 class="empty-title">暂无 Codex 配置</h3>
          <p class="empty-desc">指定 model_provider / model / wire_api，激活后写入 ~/.codex/config.toml</p>
          <NButton type="primary" size="small" @click="openAddCodex">+ 新建配置</NButton>
        </div>

        <div v-if="codexProfiles.length > 0" class="add-config-bar">
          <NButton type="primary" size="small" @click="openAddCodex">+ 新建配置</NButton>
        </div>
      </template>
    </div>

    <!-- ================================================================ -->
    <!-- Gemini CLI — env 配置                                             -->
    <!-- ================================================================ -->
    <div v-if="activeTab === 'gemini_cli'" class="tab-content">
      <div v-if="loading" class="loading-state">加载模型中…</div>

      <template v-else>
        <div class="claude-configs">
          <div
            v-for="profile in geminiProfiles"
            :key="profile.id"
            class="config-card"
            :class="{ active: profile.is_active }"
          >
            <div class="config-card-header">
              <div class="config-card-name">
                <span class="config-name-text">{{ profile.name }}</span>
                <NTag v-if="profile.is_active" size="tiny" type="success">当前</NTag>
              </div>
            </div>

            <div class="role-list">
              <div class="role-row">
                <span class="role-label">Model</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ parseGeminiConfig(profile.config_json) || "—" }}</span>
              </div>
            </div>

            <div class="config-card-actions">
              <NButton
                v-if="!profile.is_active"
                size="tiny"
                type="primary"
                @click="activateGemini(profile)"
              >
                激活
              </NButton>
              <NButton size="tiny" quaternary @click="openEditGemini(profile)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="deleteGemini(profile)">删除</NButton>
            </div>
          </div>
        </div>

        <div v-if="geminiProfiles.length === 0" class="empty-state">
          <div class="empty-icon" style="opacity: 0.4">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 48px; height: 48px">
              <circle cx="12" cy="12" r="9"/>
              <path d="M8 12h8"/>
              <path d="M12 8v8"/>
            </svg>
          </div>
          <h3 class="empty-title">暂无 Gemini CLI 配置</h3>
          <p class="empty-desc">选择默认模型，激活后写入 ~/.gemini/settings.json 的 env</p>
          <NButton type="primary" size="small" @click="openAddGemini">+ 新建配置</NButton>
        </div>

        <div v-if="geminiProfiles.length > 0" class="add-config-bar">
          <NButton type="primary" size="small" @click="openAddGemini">+ 新建配置</NButton>
        </div>
      </template>
    </div>

    <!-- ================================================================ -->
    <!-- Hermes — YAML 配置（_silk_provider_id + models）                  -->
    <!-- ================================================================ -->
    <div v-if="activeTab === 'hermes'" class="tab-content">
      <div v-if="loading" class="loading-state">加载模型中…</div>

      <template v-else>
        <div class="claude-configs">
          <div
            v-for="profile in hermesProfiles"
            :key="profile.id"
            class="config-card"
            :class="{ active: profile.is_active }"
          >
            <div class="config-card-header">
              <div class="config-card-name">
                <span class="config-name-text">{{ profile.name }}</span>
                <NTag v-if="profile.is_active" size="tiny" type="success">当前</NTag>
              </div>
            </div>

            <div class="role-list">
              <div class="role-row">
                <span class="role-label">模型数</span>
                <span class="role-arrow">→</span>
                <span class="role-model">{{ parseHermesModels(profile.config_json).length }} 个模型</span>
              </div>
            </div>

            <div class="config-card-actions">
              <NButton
                v-if="!profile.is_active"
                size="tiny"
                type="primary"
                @click="activateHermes(profile)"
              >
                激活
              </NButton>
              <NButton size="tiny" quaternary @click="openEditHermes(profile)">编辑</NButton>
              <NButton size="tiny" quaternary type="error" @click="deleteHermes(profile)">删除</NButton>
            </div>
          </div>
        </div>

        <div v-if="hermesProfiles.length === 0" class="empty-state">
          <div class="empty-icon" style="opacity: 0.4">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 48px; height: 48px">
              <circle cx="12" cy="12" r="9"/>
              <path d="M8 12h8"/>
              <path d="M12 8v8"/>
            </svg>
          </div>
          <h3 class="empty-title">暂无 Hermes 配置</h3>
          <p class="empty-desc">选择可用模型，激活后写入 ~/.hermes/config.yaml 的 custom_providers</p>
          <NButton type="primary" size="small" @click="openAddHermes">+ 新建配置</NButton>
        </div>

        <div v-if="hermesProfiles.length > 0" class="add-config-bar">
          <NButton type="primary" size="small" @click="openAddHermes">+ 新建配置</NButton>
        </div>
      </template>
    </div>

    <!-- ================================================================ -->
    <!-- Claude Code 新建/编辑弹窗                                          -->
    <!-- ================================================================ -->
    <NModal
      v-model:show="showClaudeModal"
      preset="card"
      :title="editingClaudeId ? '编辑配置' : '新建配置'"
      style="max-width: 520px"
      :bordered="false"
    >
      <div class="claude-modal">
        <div class="claude-modal-field">
          <label class="claude-modal-label">配置名称</label>
          <NInput v-model:value="claudeFormName" placeholder="如：日常编码" />
        </div>

        <div class="roles-form">
          <div v-for="role in ROLE_ORDER" :key="role" class="role-field">
            <label class="role-field-label">{{ ROLE_LABELS[role] }}</label>
            <NSelect
              :value="claudeFormRoles[role]"
              :options="allModelOptions"
              filterable
              placeholder="选择模型…"
              @update:value="(val: string) => { claudeFormRoles[role] = val; }"
            />
          </div>
        </div>

        <div class="claude-modal-actions">
          <NButton @click="showClaudeModal = false">取消</NButton>
          <NButton
            type="primary"
            :disabled="!claudeFormName.trim() || ROLE_ORDER.some((r) => !claudeFormRoles[r])"
            @click="saveClaudeConfig"
          >
            {{ editingClaudeId ? "保存" : "创建" }}
          </NButton>
        </div>
      </div>
    </NModal>

    <!-- ================================================================ -->
    <!-- OpenCode 新建/编辑弹窗（按 owned_by 分组勾选模型）                  -->
    <!-- ================================================================ -->
    <NModal
      v-model:show="showOpenCodeModal"
      preset="card"
      :title="editingOpenCodeId ? '编辑配置' : '新建配置'"
      style="max-width: 560px"
      :bordered="false"
    >
      <div class="claude-modal">
        <div class="claude-modal-field">
          <label class="claude-modal-label">配置名称</label>
          <NInput v-model:value="openCodeFormName" placeholder="如：日常编码" />
        </div>

        <div class="opencode-hint">
          模型按来源分组展示，结构与网关 <code>/v1/models</code> 返回一致。
        </div>

        <div class="owner-groups">
          <div v-for="group in modelOwnerGroups" :key="group.owner" class="owner-group">
            <div class="group-header">
              <div class="group-title">
                <span class="group-label">{{ group.ownerLabel }}</span>
                <span class="group-count">{{ group.models.length }}</span>
              </div>
              <div class="group-actions">
                <NButton size="tiny" quaternary @click="selectAllInGroup(group.owner, group.models)">全选</NButton>
                <NButton size="tiny" quaternary @click="deselectAllInGroup(group.owner)">取消</NButton>
              </div>
            </div>

            <div class="group-models">
              <label
                v-for="mid in group.models"
                :key="group.owner + '-' + mid"
                class="model-check-item"
                :class="{ checked: (openCodeFormEnabled[group.owner] || []).includes(mid) }"
              >
                <input
                  type="checkbox"
                  class="model-checkbox"
                  :checked="(openCodeFormEnabled[group.owner] || []).includes(mid)"
                  @change="toggleOpenCodeModel(group.owner, mid)"
                />
                <span class="model-check-name">{{ mid }}</span>
              </label>
              <div v-if="group.models.length === 0" class="group-empty">无模型</div>
            </div>
          </div>
        </div>

        <div v-if="modelOwnerGroups.length === 0" class="empty-state">
          <p class="empty-desc">暂无可用模型，请先在"渠道"中添加 Provider，或在"模型"中创建模型映射</p>
        </div>

        <div class="claude-modal-actions">
          <NButton @click="showOpenCodeModal = false">取消</NButton>
          <NButton
            type="primary"
            :disabled="!openCodeFormName.trim()"
            @click="saveOpenCodeConfig"
          >
            {{ editingOpenCodeId ? "保存" : "创建" }}
          </NButton>
        </div>
      </div>
    </NModal>

    <!-- ================================================================ -->
    <!-- Codex 新建/编辑弹窗                                                -->
    <!-- ================================================================ -->
    <NModal
      v-model:show="showCodexModal"
      preset="card"
      :title="editingCodexId ? '编辑配置' : '新建配置'"
      style="max-width: 520px"
      :bordered="false"
    >
      <div class="claude-modal">
        <div class="claude-modal-field">
          <label class="claude-modal-label">配置名称</label>
          <NInput v-model:value="codexFormName" placeholder="如：日常编码" />
        </div>

        <div class="claude-modal-field">
          <label class="claude-modal-label">Model Provider</label>
          <NInput v-model:value="codexForm.model_provider" placeholder="custom（内置 openai 会自动改址 openai_base_url）" />
        </div>

        <div class="claude-modal-field">
          <label class="claude-modal-label">模型</label>
          <NSelect
            v-model:value="codexForm.model"
            :options="allModelOptions"
            filterable
            placeholder="选择模型…"
          />
        </div>

        <div class="claude-modal-field">
          <label class="claude-modal-label">Wire API</label>
          <NSelect
            v-model:value="codexForm.wire_api"
            :options="[
              { label: 'responses', value: 'responses' },
              { label: 'chat', value: 'chat' },
            ]"
            placeholder="选择 API 类型…"
          />
        </div>

        <div class="claude-modal-actions">
          <NButton @click="showCodexModal = false">取消</NButton>
          <NButton
            type="primary"
            :disabled="!codexFormName.trim() || !codexForm.model_provider.trim() || !codexForm.model.trim()"
            @click="saveCodexConfig"
          >
            {{ editingCodexId ? "保存" : "创建" }}
          </NButton>
        </div>
      </div>
    </NModal>

    <!-- ================================================================ -->
    <!-- Gemini CLI 新建/编辑弹窗                                           -->
    <!-- ================================================================ -->
    <NModal
      v-model:show="showGeminiModal"
      preset="card"
      :title="editingGeminiId ? '编辑配置' : '新建配置'"
      style="max-width: 520px"
      :bordered="false"
    >
      <div class="claude-modal">
        <div class="claude-modal-field">
          <label class="claude-modal-label">配置名称</label>
          <NInput v-model:value="geminiFormName" placeholder="如：Gemini 日常" />
        </div>

        <div class="claude-modal-field">
          <label class="claude-modal-label">模型</label>
          <NSelect
            v-model:value="geminiFormModel"
            :options="allModelOptions"
            filterable
            placeholder="选择模型…"
          />
        </div>

        <div class="claude-modal-actions">
          <NButton @click="showGeminiModal = false">取消</NButton>
          <NButton
            type="primary"
            :disabled="!geminiFormName.trim() || !geminiFormModel"
            @click="saveGeminiConfig"
          >
            {{ editingGeminiId ? "保存" : "创建" }}
          </NButton>
        </div>
      </div>
    </NModal>

    <!-- ================================================================ -->
    <!-- Hermes 新建/编辑弹窗                                               -->
    <!-- ================================================================ -->
    <NModal
      v-model:show="showHermesModal"
      preset="card"
      :title="editingHermesId ? '编辑配置' : '新建配置'"
      style="max-width: 520px"
      :bordered="false"
    >
      <div class="claude-modal">
        <div class="claude-modal-field">
          <label class="claude-modal-label">配置名称</label>
          <NInput v-model:value="hermesFormName" placeholder="如：Hermes 日常" />
        </div>

        <div class="claude-modal-field">
          <label class="claude-modal-label">模型（可多选）</label>
          <NSelect
            v-model:value="hermesFormModels"
            :options="allModelOptions"
            multiple
            filterable
            placeholder="选择模型…"
          />
        </div>

        <div class="claude-modal-actions">
          <NButton @click="showHermesModal = false">取消</NButton>
          <NButton
            type="primary"
            :disabled="!hermesFormName.trim() || hermesFormModels.length === 0"
            @click="saveHermesConfig"
          >
            {{ editingHermesId ? "保存" : "创建" }}
          </NButton>
        </div>
      </div>
    </NModal>
  </div>
</template>

<style scoped>
.profiles-page {
  width: 100%;
}

/* ===== Agent 选项卡 ===== */
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
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 10px 20px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
  font-family: inherit;
  min-width: 100px;
}

.agent-tab:hover {
  background: var(--hover-bg, #e2e8f0);
}

.agent-tab.active {
  background: var(--surface, #ffffff);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}

.agent-tab-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--fg, #0f172a);
}

.agent-tab-desc {
  font-size: 11px;
  color: var(--muted, #94a3b8);
  white-space: nowrap;
}

.tab-content {
  min-height: 300px;
}

.loading-state {
  text-align: center;
  padding: 48px;
  color: var(--muted, #94a3b8);
  font-size: 14px;
}

/* ===== Claude Code 配置卡片 ===== */
.claude-configs {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 16px;
}

.config-card {
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 12px;
  padding: 16px;
  transition: all 0.15s;
  background: var(--card-bg, #ffffff);
}

.config-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.config-card.active {
  border-color: var(--accent, #0891b2);
  box-shadow: 0 0 0 1px var(--accent, #0891b2);
}

.config-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.config-card-name {
  display: flex;
  align-items: center;
  gap: 8px;
}

.config-name-text {
  font-size: 15px;
  font-weight: 600;
}

/* 角色列表 */
.role-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.role-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border-radius: 6px;
  background: var(--surface-alt, #f8fafc);
  font-size: 13px;
}

.role-label {
  font-weight: 700;
  min-width: 56px;
  color: var(--accent, #0891b2);
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
}

.role-arrow {
  color: var(--muted, #94a3b8);
  font-size: 12px;
}

.role-model {
  font-family: "JetBrains Mono", monospace;
  color: var(--fg, #0f172a);
  font-weight: 500;
}

.config-card-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  padding-top: 10px;
}

.add-config-bar {
  display: flex;
  justify-content: center;
  padding: 16px 0;
}

/* ===== OpenCode 按来源分组 ===== */
.opencode-hint {
  font-size: 13px;
  color: var(--muted, #94a3b8);
  margin-bottom: 16px;
  padding: 12px 16px;
  background: var(--surface-alt, #f1f5f9);
  border-radius: 8px;
}

.opencode-hint code {
  font-family: "JetBrains Mono", monospace;
  background: var(--border-color, #e2e8f0);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 12px;
}

.owner-groups {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 20px;
}

.owner-group {
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 12px;
  overflow: hidden;
}

.owner-group .group-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 16px;
  background: var(--surface-alt, #f1f5f9);
  border-bottom: 1px solid var(--border-color, #e2e8f0);
}

.owner-group .group-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.owner-group .group-label {
  font-size: 14px;
  font-weight: 600;
}

.owner-group .group-count {
  font-size: 12px;
  color: var(--muted, #94a3b8);
  font-family: "JetBrains Mono", monospace;
}

.owner-group .group-actions {
  display: flex;
  gap: 4px;
}

.owner-group .group-models {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 10px 14px;
}

.model-check-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
  border: 1px solid transparent;
  user-select: none;
}

.model-check-item:hover {
  background: var(--hover-bg, #f8fafc);
  border-color: var(--border-color, #e2e8f0);
}

.model-check-item.checked {
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
  border-color: var(--accent, #0891b2);
}

.model-checkbox {
  width: 15px;
  height: 15px;
  accent-color: var(--accent, #0891b2);
  flex-shrink: 0;
  cursor: pointer;
}

.model-check-name {
  font-size: 13px;
  font-weight: 600;
  font-family: "JetBrains Mono", monospace;
  white-space: nowrap;
}

.group-empty {
  padding: 12px;
  font-size: 13px;
  color: var(--muted, #94a3b8);
  text-align: center;
}

.opencode-save-bar {
  display: flex;
  justify-content: center;
  padding: 8px 0 24px;
}

/* ===== Claude Modal ===== */
.claude-modal {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.claude-modal-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.claude-modal-label {
  font-size: 13px;
  font-weight: 600;
}

.roles-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.role-field {
  display: flex;
  align-items: center;
  gap: 12px;
}

.role-field-label {
  font-weight: 700;
  min-width: 60px;
  font-size: 13px;
  color: var(--accent, #0891b2);
  font-family: "JetBrains Mono", monospace;
}

.role-field .n-select {
  flex: 1;
}

.claude-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--border-color, #e2e8f0);
}
</style>