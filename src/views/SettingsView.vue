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
  NCard,
  NAlert,
  useMessage,
} from "naive-ui";
import { useGatewayStore } from "../stores/gateway";
import { storeToRefs } from "pinia";
import { api } from "../api";

/**
 * 端口冲突校验
 *
 * 规则：
 *  - 1024–49151（用户端口区间，无需管理员权限）
 *  - 避开 Hyper-V / Docker 常见保留端口
 *  - 避开主流数据库、中间件、Web 服务器默认端口
 *  - 避开开发者高频选型端口（3000、5000、8000、8080、9090 等）
 */
const CONFLICT_PORTS = new Set([
  // Hyper-V / Docker / 系统保留
  135, 136, 137, 138, 139, 445, 548, 3389, 5353, 5985, 5986,
  // 数据库
  1433, 1434, 1521, 3306, 5432, 6379, 9042, 27017,
  // 中间件 / 消息队列
  5672, 8161, 9200, 5601, 15672,
  // Web 服务器 / 代理
  8080, 8443, 9443,
  // 开发者高频
  3000, 4000, 5000, 5173, 8000, 8090, 9000, 9090,
  // 其他常见服务
  3478, 1714, 1715, 1716, 1717, 1718, 1719, 1720, 1721, 1722, 1723, 1724, 1764,
]);

function validatePort(port: number): string | null {
  if (port < 1024 || port > 49151) {
    return "端口必须在 1024–49151 的用户端口区间内（无需管理员权限）";
  }
  if (CONFLICT_PORTS.has(port)) {
    return `端口 ${port} 与常见服务端口冲突，请选择其他端口`;
  }
  return null;
}

const gatewayStore = useGatewayStore();
const { status, loading } = storeToRefs(gatewayStore);
const message = useMessage();

const formRef = ref<any>(null);
const formValue = ref({
  bind_host: "127.0.0.1",
  bind_port: 1877,
  allow_remote: false,
  log_retention_days: 30,
  launch_at_startup: false,
  close_to_tray: true,
  auto_start_gateway: true,
  default_provider_id: "",
});

async function handleSave() {
  try {
    const portError = validatePort(formValue.value.bind_port);
    if (portError) {
      message.error(portError);
      return;
    }

    // 空字符串转 null，避免覆盖已有值
    const payload = {
      ...formValue.value,
      default_provider_id: formValue.value.default_provider_id || null,
    };
    await gatewayStore.updateSettings(payload);
    message.success("设置已保存");
  } catch {
    message.error("保存失败");
  }
}

async function handleExportConfig() {
  try {
    const accepted = await confirm(
      "导出的配置文件包含可迁移的明文渠道 Key 与网关 Key，请妥善保管。是否继续？",
      { title: "导出 Silk 配置", kind: "warning", okLabel: "继续", cancelLabel: "取消" }
    );
    if (!accepted) return;

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
      };
    }
  },
  { immediate: true }
);

onMounted(() => {
  gatewayStore.fetchStatus();
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
            <NInputNumber v-model:value="formValue.bind_port" :min="1024" :max="49151" style="width: 100%" placeholder="1877" />
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
            <div class="data-action-desc">导出当前网关设置、渠道、路由、模型映射与网关 Key；文件包含敏感密钥，请妥善保管。</div>
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
</style>
