<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import {
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSwitch,
  NButton,
  NText,
  NTag,
  NCard,
  NSpace,
  NAlert,
  NModal,
  useMessage,
  useDialog,
} from "naive-ui";
import { useGatewayStore } from "../stores/gateway";
import { storeToRefs } from "pinia";
import { api, type GatewayKey } from "../api";

const gatewayStore = useGatewayStore();
const { status, loading } = storeToRefs(gatewayStore);
const message = useMessage();
const dialog = useDialog();

const formRef = ref<any>(null);
const formValue = ref({
  bind_host: "127.0.0.1",
  bind_port: 9876,
  allow_remote: false,
  log_retention_days: 30,
  launch_at_startup: false,
  close_to_tray: true,
  auto_start_gateway: false,
  default_provider_id: "",
  default_route_id: "",
});

// Gateway Keys
const keys = ref<GatewayKey[]>([]);
const showAddKey = ref(false);
const newKeyName = ref("");
const newKeyMaxConcurrent = ref(10);
const newKeyCreated = ref<{ name: string; key: string } | null>(null);
const showNewKeyModal = ref(false);

async function loadKeys() {
  try {
    keys.value = await api.listGatewayKeys();
  } catch {
    // ignore
  }
}

async function toggleKey(key: GatewayKey) {
  try {
    await api.updateGatewayKey(key.id, { enabled: !key.enabled });
    message.success(key.enabled ? "已禁用" : "已启用");
    await loadKeys();
  } catch {
    message.error("操作失败");
  }
}

async function deleteKey(id: string) {
  dialog.warning({
    title: "确认删除",
    content: "删除后无法恢复，确定要删除此 Key 吗？",
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await api.deleteGatewayKey(id);
        keys.value = keys.value.filter((k) => k.id !== id);
        message.success("已删除");
      } catch {
        message.error("删除失败");
      }
    },
  });
}

async function addKey() {
  if (!newKeyName.value) {
    message.warning("请填写名称");
    return;
  }
  try {
    const result = await api.createGatewayKey({
      name: newKeyName.value,
      key_value: "",  // 空值触发后端自动生成
      max_concurrent: newKeyMaxConcurrent.value,
    });
    message.success("添加成功");
    newKeyCreated.value = {
      name: result.key.name,
      key: result.plain_key,
    };
    showNewKeyModal.value = true;
    newKeyName.value = "";
    newKeyMaxConcurrent.value = 10;
    showAddKey.value = false;
    await loadKeys();
  } catch {
    message.error("添加失败");
  }
}

function copyKey(text: string) {
  navigator.clipboard.writeText(text).then(() => {
    message.success("已复制到剪贴板");
  }).catch(() => {
    message.error("复制失败");
  });
}

async function handleSave() {
  try {
    // 空字符串转 null，避免覆盖已有值
    const payload = {
      ...formValue.value,
      default_provider_id: formValue.value.default_provider_id || null,
      default_route_id: formValue.value.default_route_id || null,
    };
    await gatewayStore.updateSettings(payload);
    message.success("设置已保存");
  } catch {
    message.error("保存失败");
  }
}

async function handleExportConfig() {
  try {
    const filePath = await save({
      title: "导出 Silk 配置",
      defaultPath: "silk_config_export.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!filePath) return;
    const result = await api.exportAppConfig({ file_path: filePath });
    message.success(`配置已导出到 ${result.file_path}`);
  } catch {
    message.error("导出配置失败");
  }
}

async function handleBackupDatabase() {
  try {
    const filePath = await save({
      title: "备份 Silk 数据库",
      defaultPath: "silk_database_backup.db",
      filters: [{ name: "SQLite", extensions: ["db"] }],
    });
    if (!filePath) return;
    const result = await api.backupDatabase({ file_path: filePath });
    message.success(`数据库已备份到 ${result.file_path}`);
  } catch {
    message.error("备份数据库失败");
  }
}

async function handleRestoreDatabase() {
  try {
    const accepted = await confirm(
      "恢复数据库会覆盖当前的渠道、路由、模型映射、日志和网关 Key。是否继续？",
      { title: "恢复数据库", kind: "warning", okLabel: "继续", cancelLabel: "取消" }
    );
    if (!accepted) return;

    const filePath = await open({
      title: "选择数据库备份文件",
      multiple: false,
      directory: false,
      filters: [{ name: "SQLite", extensions: ["db"] }],
    });
    if (!filePath || Array.isArray(filePath)) return;

    const result = await api.restoreDatabase({ file_path: filePath });
    message.success(`数据库已从 ${result.file_path} 恢复`);
    await gatewayStore.fetchStatus();
    await loadKeys();
  } catch {
    message.error("恢复数据库失败");
  }
}

async function handleImportConfig() {
  try {
    const accepted = await confirm(
      "导入配置会覆盖当前的渠道、路由、模型映射与网关 Key。是否继续？",
      { title: "导入配置", kind: "warning", okLabel: "继续", cancelLabel: "取消" }
    );
    if (!accepted) return;

    const filePath = await open({
      title: "选择 Silk 配置文件",
      multiple: false,
      directory: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!filePath || Array.isArray(filePath)) return;

    const result = await api.importAppConfig({ file_path: filePath });
    message.success(`配置已从 ${result.file_path} 导入`);
    await gatewayStore.fetchStatus();
    await loadKeys();
  } catch {
    message.error("导入配置失败");
  }
}

watch(
  status,
  (newStatus) => {
    if (newStatus?.settings) {
      const s = newStatus.settings;
      formValue.value = {
        bind_host: s.bind_host,
        bind_port: s.bind_port,
        allow_remote: s.allow_remote,
        log_retention_days: s.log_retention_days,
        launch_at_startup: s.launch_at_startup,
        close_to_tray: s.close_to_tray,
        auto_start_gateway: s.auto_start_gateway,
        default_provider_id: s.default_provider_id || "",
        default_route_id: s.default_route_id || "",
      };
    }
  },
  { immediate: true }
);

onMounted(() => {
  gatewayStore.fetchStatus();
  loadKeys();
});
</script>

<template>
  <div class="settings-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">设置</h2>
      </div>
      <div class="toolbar-right">
        <NButton type="primary" size="small" @click="handleSave" :loading="loading">保存更改</NButton>
      </div>
    </div>

    <NAlert type="info" :bordered="false" class="settings-alert">
      Silk 的设置优先服务本地桌面使用。这里保留网关基础能力，并补充关闭窗口与自动启动网关等桌面行为。
    </NAlert>

    <!-- 网关基础 -->
    <NCard :bordered="false" class="settings-card" size="small" title="网关基础">
      <NForm ref="formRef" :model="formValue" label-placement="left" label-width="100">
        <div class="form-row">
          <NFormItem label="监听地址" style="flex: 1">
            <NInput v-model:value="formValue.bind_host" placeholder="127.0.0.1" />
          </NFormItem>
          <NFormItem label="监听端口" style="flex: 1">
            <NInputNumber v-model:value="formValue.bind_port" :min="1" :max="65535" style="width: 100%" />
          </NFormItem>
        </div>
        <div class="form-row">
          <NFormItem label="允许远程访问" style="flex: 1">
            <NSwitch v-model:value="formValue.allow_remote" />
          </NFormItem>
          <NFormItem label="日志保留天数" style="flex: 1">
            <NInputNumber v-model:value="formValue.log_retention_days" :min="1" :max="3650" style="width: 100%" />
          </NFormItem>
        </div>
      </NForm>
    </NCard>

    <NCard :bordered="false" class="settings-card" size="small" title="桌面行为">
      <NForm :model="formValue" label-placement="left" label-width="120">
        <div class="form-row">
          <NFormItem label="开机自启" style="flex: 1">
            <NSwitch v-model:value="formValue.launch_at_startup" />
          </NFormItem>
          <NFormItem label="关闭到后台" style="flex: 1">
            <NSwitch v-model:value="formValue.close_to_tray" />
          </NFormItem>
        </div>
        <div class="form-row">
          <NFormItem label="启动后自动开网关" style="flex: 1">
            <NSwitch v-model:value="formValue.auto_start_gateway" />
          </NFormItem>
        </div>
        <NText depth="3" class="settings-help">
          开启“开机自启”后，Silk 会注册到系统启动项；开启“关闭到后台”后，关闭窗口会隐藏应用而不是直接退出；开启“启动后自动开网关”后，Silk 启动时会自动恢复本地网关。
        </NText>
      </NForm>
    </NCard>

    <NCard :bordered="false" class="settings-card" size="small" title="配置与数据">
      <div class="data-actions">
        <div class="data-action">
          <div>
            <div class="data-action-title">导出配置</div>
            <div class="data-action-desc">导出当前网关设置、渠道、路由、模型映射与网关 Key。</div>
          </div>
          <NButton size="small" @click="handleExportConfig">导出配置</NButton>
        </div>
        <div class="data-action">
          <div>
            <div class="data-action-title">导入配置</div>
            <div class="data-action-desc">从已有配置文件恢复 Silk 配置，不会清理历史日志。</div>
          </div>
          <NButton size="small" @click="handleImportConfig">导入配置</NButton>
        </div>
        <div class="data-action">
          <div>
            <div class="data-action-title">备份数据库</div>
            <div class="data-action-desc">生成当前 SQLite 数据库副本，适合迁移或长期留档。</div>
          </div>
          <NButton size="small" @click="handleBackupDatabase">备份数据库</NButton>
        </div>
        <div class="data-action">
          <div>
            <div class="data-action-title">恢复数据库</div>
            <div class="data-action-desc">从已有 `.db` 备份恢复业务数据，不会改动当前桌面设置文件。</div>
          </div>
          <NButton size="small" type="warning" @click="handleRestoreDatabase">恢复数据库</NButton>
        </div>
      </div>
    </NCard>

    <!-- Key 管理 -->
    <NCard :bordered="false" class="settings-card" size="small" title="Key 管理">
      <template #header-extra>
        <NButton size="small" @click="showAddKey = !showAddKey">+ 添加 Key</NButton>
      </template>

      <div v-if="showAddKey" class="add-key-box">
        <div class="form-row" style="margin-bottom: 8px">
          <NInput v-model:value="newKeyName" placeholder="名称" style="flex: 1" />
          <NInputNumber v-model:value="newKeyMaxConcurrent" :min="1" :max="1000" placeholder="并发数" style="flex: 0 0 100px" />
          <NButton type="primary" size="small" @click="addKey">生成</NButton>
          <NButton size="small" @click="showAddKey = false">取消</NButton>
        </div>
        <div style="font-size:12px;color:var(--text-color-3,#94a3b8);margin-top:4px">
          Key 值由系统自动生成（<code>sk-gw-xxx</code> 格式），创建后仅展示一次
        </div>
      </div>

      <!-- 新 Key 展示 -->
      <NModal
        v-model:show="showNewKeyModal"
        preset="card"
        title="Key 已创建"
        style="max-width: 480px"
        :bordered="false"
        @update:show="(val: boolean) => { if (!val) newKeyCreated = null; }"
      >
        <div v-if="newKeyCreated" style="display:flex;flex-direction:column;gap:12px">
          <div style="font-size:13px">名称: <strong>{{ newKeyCreated.name }}</strong></div>
          <div style="font-size:13px;margin-bottom:4px">Key 值（请立即复制，关闭后不再显示）:</div>
          <div
            style="
              font-family:'JetBrains Mono','Consolas',monospace;
              font-size:14px;
              background:#1e293b;
              color:#e2e8f0;
              padding:12px 16px;
              border-radius:8px;
              word-break:break-all;
              cursor:pointer;
              user-select:all;
            "
            @click="copyKey(newKeyCreated.key)"
          >
            {{ newKeyCreated.key }}
          </div>
          <NButton size="small" @click="copyKey(newKeyCreated.key)">复制 Key</NButton>
          <NButton size="small" type="primary" @click="newKeyCreated = null">我已保存，关闭</NButton>
        </div>
      </NModal>

      <div class="keys-list">
        <div v-for="key in keys" :key="key.id" class="key-row">
          <div class="key-info">
            <span class="key-name">{{ key.name }}</span>
            <NTag size="small" style="font-family: 'JetBrains Mono', 'Consolas', monospace">
              {{ key.key_prefix }}****
            </NTag>
            <NTag :type="key.enabled ? 'success' : 'warning'" size="small">
              {{ key.enabled ? '启用' : '禁用' }}
            </NTag>
            <span class="key-concurrent" v-if="key.max_concurrent">并发: {{ key.max_concurrent }}</span>
          </div>
          <NSpace :size="4">
            <NButton size="tiny" quaternary @click="toggleKey(key)">
              {{ key.enabled ? '禁用' : '启用' }}
            </NButton>
            <NButton size="tiny" quaternary type="error" @click="deleteKey(key.id)">删除</NButton>
          </NSpace>
        </div>
        <NText v-if="keys.length === 0" depth="3" style="display: block; text-align: center; padding: 24px">
          暂无 Key，点击上方"+ 添加 Key"创建
        </NText>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.settings-page {
  width: 100%;
}

.settings-card {
  border-radius: 12px;
  margin-bottom: 16px;
}

.settings-alert {
  margin-bottom: 16px;
  border-radius: 12px;
}

.settings-help {
  display: block;
  margin-top: 4px;
  font-size: 12px;
}

.data-actions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.data-action {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 10px;
}

.data-action-title {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 4px;
}

.data-action-desc {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.add-key-box {
  background: var(--card-color-alt, #f8fafc);
  border-radius: 8px;
  padding: 16px;
  margin-bottom: 16px;
  border: 1px solid var(--border-color, #e2e8f0);
}

.keys-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.key-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  transition: background 0.2s;
}

.key-row:hover {
  background: var(--hover-color, #f8fafc);
}

.key-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.key-name {
  font-weight: 600;
  font-size: 13px;
  min-width: 60px;
}

.key-concurrent {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}
</style>
