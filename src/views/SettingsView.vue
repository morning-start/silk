<script setup lang="ts">
import { computed, ref, onMounted, watch } from "vue";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import {
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSwitch,
  NButton,
  NText,
  NAlert,
  NCollapse,
  NCollapseItem,
  useMessage,
} from "naive-ui";
import { useGatewayStore } from "../stores/gateway";
import { useProvidersStore } from "../stores/providers";
import { storeToRefs } from "pinia";
import { configApi } from "../api/config";
import AppPageShell from "../components/AppPageShell.vue";

/** 日志级别选项（与后端 tracing 级别一致） */
const LOG_LEVEL_OPTIONS = [
  { label: "trace（最详细，仅排障时使用）", value: "trace" },
  { label: "debug", value: "debug" },
  { label: "info（推荐）", value: "info" },
  { label: "warn", value: "warn" },
  { label: "error（最简）", value: "error" },
];

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
const providersStore = useProvidersStore();
const { status, loading } = storeToRefs(gatewayStore);
const { providers } = storeToRefs(providersStore);
const message = useMessage();

const formRef = ref<any>(null);

/** 兜底渠道选项：默认项表示不指定（按模型映射 / 路径推断） */
const providerOptions = computed(() => [
  { label: "不指定（按模型映射与路径推断）", value: "" },
  ...providers.value.map((p) => ({ label: p.name, value: p.id })),
]);

/** 模块级日志覆盖用「模块 → 级别」的行式编辑，保存时再折叠成对象 */
const logModuleRows = ref<{ module: string; level: string }[]>([]);

function addLogModuleRow() {
  logModuleRows.value.push({ module: "", level: "debug" });
}

function removeLogModuleRow(index: number) {
  logModuleRows.value.splice(index, 1);
}

function logModulesFromRows(): Record<string, string> {
  const result: Record<string, string> = {};
  for (const row of logModuleRows.value) {
    const key = row.module.trim();
    if (key) result[key] = row.level;
  }
  return result;
}

const formValue = ref({
  bind_host: "127.0.0.1",
  bind_port: 1877,
  allow_remote: false,
  log_retention_days: 30,
  launch_at_startup: false,
  close_to_tray: true,
  minimize_to_tray: true,
  auto_start_gateway: true,
  default_provider_id: "",
  proxy_url: "",
  rate_limit_enabled: true,
  rate_limit_max_requests_per_minute: 1000,
  rate_limit_max_tokens_per_minute: 500000,
  trace_enabled: false,
  log_level: "info",
  file_level: "debug",
});

async function handleSave() {
  try {
    const portError = validatePort(formValue.value.bind_port);
    if (portError) {
      message.error(portError);
      return;
    }

    // 空字符串 → null：后端把「字段显式为 null」当作清除，
    // 与「字段缺失 = 不改动」区分开（见 UpdateGatewaySettings 的 NullableUpdate）
    const payload = {
      ...formValue.value,
      default_provider_id: formValue.value.default_provider_id || null,
      proxy_url: formValue.value.proxy_url.trim() || null,
      log_modules: logModulesFromRows(),
    };
    await gatewayStore.updateSettings(payload);
    if (status.value?.running) {
      message.success("设置已保存，网关已自动重启");
    } else {
      message.info("设置已保存。网关当前未运行，请点击顶栏「启动网关」按钮手动启动");
    }
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
    const result = await configApi.exportConfig({ file_path: filePath });
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
    const result = await configApi.backupDatabase({ file_path: filePath });
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

    const result = await configApi.restoreDatabase({ file_path: filePath });
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

    const result = await configApi.importConfig({ file_path: filePath });
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
        minimize_to_tray: s.minimize_to_tray,
        auto_start_gateway: s.auto_start_gateway,
        default_provider_id: s.default_provider_id || "",
        proxy_url: s.proxy_url || "",
        rate_limit_enabled: s.rate_limit_enabled,
        rate_limit_max_requests_per_minute: s.rate_limit_max_requests_per_minute,
        rate_limit_max_tokens_per_minute: s.rate_limit_max_tokens_per_minute,
        trace_enabled: s.trace_enabled,
        log_level: s.log_level,
        file_level: s.file_level,
      };
      logModuleRows.value = Object.entries(s.log_modules || {}).map(
        ([module, level]) => ({ module, level })
      );
    }
  },
  { immediate: true }
);

onMounted(async () => {
  gatewayStore.fetchStatus();
  // 兜底渠道下拉需要渠道列表；失败不阻塞设置页其余功能
  try {
    await providersStore.fetchAll();
  } catch {
    /* 忽略：无渠道时下拉只剩「不指定」 */
  }
});
</script>

<template>
  <AppPageShell
    title="设置"
    desc="网关行为、数据管理与安全配置"
  >
    <template #actions>
      <NButton type="primary" size="small" @click="handleSave" :loading="loading">保存更改</NButton>
    </template>

    <!-- 网关基础 -->
    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">网关基础</h2>
      </header>
      <div class="s-card-body">
        <NAlert type="info" :bordered="false" style="margin-bottom: var(--sp-3)">
          Silk 的设置优先服务本地桌面使用。这里保留网关基础能力，并补充关闭窗口与自动启动网关等桌面行为。
        </NAlert>
        <NForm ref="formRef" :model="formValue" label-placement="left" label-width="100">
          <div class="sv-frow">
            <NFormItem label="监听地址" style="flex: 1">
              <NInput v-model:value="formValue.bind_host" placeholder="127.0.0.1" />
            </NFormItem>
            <NFormItem label="监听端口" style="flex: 1">
              <NInputNumber v-model:value="formValue.bind_port" :min="1024" :max="49151" style="width: 100%" placeholder="1877" />
            </NFormItem>
          </div>
          <div class="sv-frow">
            <NFormItem label="允许远程访问" style="flex: 1">
              <NSwitch v-model:value="formValue.allow_remote" />
            </NFormItem>
            <NFormItem label="日志保留天数" style="flex: 1">
              <NInputNumber v-model:value="formValue.log_retention_days" :min="1" :max="3650" style="width: 100%" />
            </NFormItem>
          </div>
          <div class="sv-frow">
            <NFormItem label="全局代理" style="flex: 1">
              <NInput v-model:value="formValue.proxy_url" placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:9000，留空表示直连" />
            </NFormItem>
          </div>
          <NText depth="3" class="settings-help">
            为所有未单独配置代理的渠道设置默认代理地址；在「渠道管理」中可为单个渠道覆盖此设置。
          </NText>
        </NForm>
      </div>
    </section>

    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">请求与限流</h2>
      </header>
      <div class="s-card-body">
        <NForm :model="formValue" label-placement="left" label-width="120">
          <div class="sv-frow">
            <NFormItem label="启用限流" style="flex: 0 0 200px">
              <NSwitch v-model:value="formValue.rate_limit_enabled" />
            </NFormItem>
            <NFormItem label="每分钟请求数" style="flex: 1">
              <NInputNumber
                v-model:value="formValue.rate_limit_max_requests_per_minute"
                :min="1"
                :max="100000"
                :disabled="!formValue.rate_limit_enabled"
                style="width: 100%"
              />
            </NFormItem>
            <NFormItem label="每分钟 Token" style="flex: 1">
              <NInputNumber
                v-model:value="formValue.rate_limit_max_tokens_per_minute"
                :min="1"
                :max="100000000"
                :disabled="!formValue.rate_limit_enabled"
                style="width: 100%"
              />
            </NFormItem>
          </div>
          <div class="sv-frow">
            <NFormItem label="兜底渠道" style="flex: 1">
              <NSelect
                v-model:value="formValue.default_provider_id"
                :options="providerOptions"
                filterable
              />
            </NFormItem>
          </div>
          <NText depth="3" class="settings-help">
            限流按网关 Key 统计每分钟的请求数与 Token 消耗，超出后直接返回 429；保存后立即生效（无需重启网关）。
            「兜底渠道」用于请求未命中「模型」页配置的映射时，默认转发到哪个渠道。
          </NText>
        </NForm>
      </div>
    </section>

    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">桌面行为</h2>
      </header>
      <div class="s-card-body">
        <NForm :model="formValue" label-placement="left" label-width="120">
          <div class="sv-frow">
            <NFormItem label="开机自启" style="flex: 1">
              <NSwitch v-model:value="formValue.launch_at_startup" />
            </NFormItem>
            <NFormItem label="关闭到后台" style="flex: 1">
              <NSwitch v-model:value="formValue.close_to_tray" />
            </NFormItem>
          </div>
          <div class="sv-frow">
            <NFormItem label="启动后自动开网关" style="flex: 1">
              <NSwitch v-model:value="formValue.auto_start_gateway" />
            </NFormItem>
            <NFormItem label="最小化到托盘" style="flex: 1">
              <NSwitch v-model:value="formValue.minimize_to_tray" />
            </NFormItem>
          </div>
          <NText depth="3" class="settings-help">
            开启"开机自启"后，Silk 会注册到系统启动项；开启"关闭到后台"后，关闭窗口会隐藏应用而不是直接退出；开启"最小化到托盘"后，最小化窗口会隐藏到托盘（点击托盘图标可恢复）；开启"启动后自动开网关"后，Silk 启动时会自动恢复本地网关。
          </NText>
        </NForm>
      </div>
    </section>

    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">日志</h2>
      </header>
      <div class="s-card-body">
        <NForm :model="formValue" label-placement="left" label-width="120">
          <div class="sv-frow">
            <NFormItem label="控制台级别" style="flex: 1">
              <NSelect v-model:value="formValue.log_level" :options="LOG_LEVEL_OPTIONS" />
            </NFormItem>
            <NFormItem label="文件级别" style="flex: 1">
              <NSelect v-model:value="formValue.file_level" :options="LOG_LEVEL_OPTIONS" />
            </NFormItem>
            <NFormItem label="协议转换追踪" style="flex: 0 0 220px">
              <NSwitch v-model:value="formValue.trace_enabled" />
            </NFormItem>
          </div>
          <NCollapse class="log-collapse">
            <NCollapseItem
              :title="
                logModuleRows.length
                  ? `模块级日志覆盖（已自定义 ${logModuleRows.length} 项）`
                  : '模块级日志覆盖（使用默认）'
              "
              name="log-modules"
            >
              <div v-for="(row, index) in logModuleRows" :key="index" class="log-module-row">
                <NInput
                  v-model:value="row.module"
                  placeholder="模块路径，如 silk_lib::gateway::middleware::dispatch_upstream"
                />
                <NSelect v-model:value="row.level" :options="LOG_LEVEL_OPTIONS" style="width: 160px" />
                <NButton quaternary circle type="error" @click="removeLogModuleRow(index)">×</NButton>
              </div>
              <NButton size="small" secondary @click="addLogModuleRow">+ 添加模块</NButton>
              <NText depth="3" class="settings-help">
                模块级覆盖优先于全局级别，用于临时放大某个模块的日志（如排查流式转换时分派模块的问题）。留空的模块行会被忽略。
              </NText>
            </NCollapseItem>
          </NCollapse>
          <NText depth="3" class="settings-help">
            日志写入数据目录下的 logs/silk.log，按天轮转并按"日志保留天数"清理。日志级别与追踪开关在应用重启后生效；「协议转换追踪」会把 prism 每次转换的输入输出额外写入日志，仅排障时开启。
          </NText>
        </NForm>
      </div>
    </section>

    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">配置与数据</h2>
      </header>
      <div class="s-card-body">
        <div class="s-grid-2">
          <div class="s-card">
            <div class="s-card-body da-card">
              <div class="da-content">
                <div class="da-title">导出配置</div>
                <div class="da-desc">导出当前网关设置、渠道、路由、模型映射与网关 Key；文件包含敏感密钥，请妥善保管。</div>
              </div>
              <NButton size="small" @click="handleExportConfig">导出</NButton>
            </div>
          </div>
          <div class="s-card">
            <div class="s-card-body da-card">
              <div class="da-content">
                <div class="da-title">导入配置</div>
                <div class="da-desc">从已有配置文件恢复 Silk 配置，不会清理历史日志。</div>
              </div>
              <NButton size="small" @click="handleImportConfig">导入</NButton>
            </div>
          </div>
          <div class="s-card">
            <div class="s-card-body da-card">
              <div class="da-content">
                <div class="da-title">备份数据库</div>
                <div class="da-desc">生成当前 SQLite 数据库副本，适合迁移或长期留档。</div>
              </div>
              <NButton size="small" @click="handleBackupDatabase">备份</NButton>
            </div>
          </div>
          <div class="s-card">
            <div class="s-card-body da-card">
              <div class="da-content">
                <div class="da-title">恢复数据库</div>
                <div class="da-desc">从已有 `.db` 备份恢复业务数据，不会改动当前桌面设置文件。</div>
              </div>
              <NButton size="small" type="warning" @click="handleRestoreDatabase">恢复</NButton>
            </div>
          </div>
        </div>
      </div>
    </section>
  </AppPageShell>
</template>
<style scoped>
/* 页面骨架、页头、卡片、网格全部走 style.css，
   这里只保留设置页特有的表单行、帮助文字与折叠项造型。 */

/* 表单横向行：两栏并排，间距走令牌 */
.sv-frow {
  display: flex;
  gap: var(--sp-3);
  margin-bottom: var(--sp-3);
}

.sv-frow:last-child {
  margin-bottom: 0;
}

.settings-help {
  display: block;
  margin-top: 4px;
  font-size: var(--fs-sm);
  color: var(--muted);
}

/* 数据操作卡片：标题/说明在左，操作在右 */
.da-card {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
}

.da-content {
  flex: 1;
  min-width: 0;
}

.da-title {
  font-size: var(--fs-base);
  font-weight: 600;
  margin-bottom: 4px;
  color: var(--fg);
}

.da-desc {
  font-size: var(--fs-sm);
  color: var(--muted);
  line-height: 1.4;
}

/* 高级项折叠：与弹窗内的 ModalAdvanced 同一思路——收起但不隐藏状态 */
.log-collapse {
  margin: 4px 0 0;
  border-top: 1px solid var(--border-soft);
}

.log-module-row {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin-bottom: var(--sp-2);
}

.log-module-row :deep(.n-input) {
  flex: 1;
  min-width: 0;
}
</style>
