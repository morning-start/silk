<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
import { useRouter } from "vue-router";
import { formatMs } from "../utils/format";
import {
  NIcon,
  NSpin,
  useMessage,
} from "naive-ui";
import {
  ArrowForwardOutline,
  CopyOutline,
  KeyOutline,
  RefreshOutline,
} from "@vicons/ionicons5";
import { statsApi } from "../api/stats";
import { configApi } from "../api/config";
import type { DashboardStats, RequestLog } from "../api";
import { useGatewayStore } from "../stores/gateway";
import { useDataChangeSignal } from "../composables/useCrossStoreNotify";
import { useConfirm } from "../utils/confirm";

const router = useRouter();
const gatewayStore = useGatewayStore();
const message = useMessage();
const { confirm } = useConfirm();

const loading = ref(false);
const error = ref<string | null>(null);
const stats = ref<DashboardStats | null>(null);
const recentLogs = ref<RequestLog[]>([]);
const logsLoading = ref(false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:1877");
const gatewayKey = ref("");

async function loadData() {
  loading.value = true;
  logsLoading.value = true;
  error.value = null;
  try {
    const [s, logs, key] = await Promise.all([
      statsApi.dashboard(),
      statsApi.recentRequests(10),
      configApi.getBuiltinGatewayKey(),
    ]);
    stats.value = s;
    recentLogs.value = logs;
    gatewayKey.value = key.plain_key;
  } catch (e: any) {
    error.value = e.message || "加载仪表盘数据失败";
  } finally {
    loading.value = false;
    logsLoading.value = false;
  }
}

function goToLogs() {
  router.push("/logs");
}

function copyGatewayAddress() {
  navigator.clipboard.writeText(`http://${bindAddress.value}`).then(() => {
    message.success("本地地址已复制");
  }).catch(() => {
    message.error("复制失败");
  });
}

function copyGatewayKey() {
  navigator.clipboard.writeText(gatewayKey.value).then(() => {
    message.success("API Key 已复制");
  }).catch(() => {
    message.error("复制失败");
  });
}

function resetGatewayKey() {
  confirm({
    title: "刷新本地 API Key",
    description: "重新生成网关的内置 Key，旧 Key 立即作废。",
    impacts: [
      "正在使用旧 Key 的客户端会立刻收到鉴权失败",
      "需要把新 Key 同步更新到各客户端配置中",
    ],
    positiveText: "刷新",
    onConfirm: async () => {
      const res = await configApi.resetBuiltinGatewayKey();
      gatewayKey.value = res.plain_key;
      message.success("API Key 已刷新，请更新客户端配置");
    },
    onError: (e: any) => message.error(e?.message || "刷新 API Key 失败"),
  });
}

onMounted(() => {
  loadData();
  gatewayStore.fetchStatus();
});

// 跨 Store 联动
const providersSignal = useDataChangeSignal("providers");

watch(
  [providersSignal],
  () => { loadData(); },
  { flush: "post" }
);
</script>

<template>
  <NSpin :show="loading" style="min-height: 300px">
    <div class="dashboard">
      <!-- Error Banner -->
      <div v-if="error" class="error-state">
        <div class="error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:var(--danger)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h3 class="error-title">仪表盘数据加载失败</h3>
        <p class="error-desc">{{ error }}</p>
        <button class="btn btn-primary" @click="loadData">重新加载</button>
      </div>

      <!-- Welcome / Status Card -->
      <div class="welcome-card card mb-16">
        <div class="card-body welcome-body">
          <div class="welcome-left">
            <span class="welcome-kicker">LOCAL RUNTIME / 01</span>
            <span class="welcome-label" :class="{ online: gatewayStore.status?.running }">
              <span class="status-dot-sm" :class="gatewayStore.status?.running ? 'online' : 'offline'"></span>
              {{ gatewayStore.status?.running ? '网关在线' : '网关离线' }}
            </span>
            <h2 class="welcome-heading" v-if="gatewayStore.status?.running">Silk 本地网关运行中</h2>
            <h2 class="welcome-heading" v-else>Silk 本地网关未启动</h2>
            <p class="welcome-desc" v-if="gatewayStore.status?.running">
              本地监听 <span class="text-mono">{{ bindAddress }}</span>，
              今日已处理 <span class="text-mono">{{ stats?.today_requests?.toLocaleString() || 0 }}</span> 次请求
            </p>
            <p class="welcome-desc" v-else>
              网关尚未启动，配置好渠道后即可启动网关转发请求
            </p>
            <div class="welcome-meta" v-if="gatewayStore.status?.running && stats">
              <span class="badge badge-success">可用渠道 {{ stats.active_providers }}</span>
              <span class="badge badge-accent">运行正常</span>
              <span class="badge badge-neutral">运行中</span>
            </div>
            <div class="row gap-md mt-20 welcome-actions">
              <button class="btn btn-primary" @click="copyGatewayAddress">
                <NIcon :size="14"><CopyOutline /></NIcon>
                复制本地 API 地址
              </button>
              <button class="btn btn-secondary" @click="copyGatewayKey">
                <NIcon :size="14"><KeyOutline /></NIcon>
                复制 API Key
              </button>
              <button class="btn btn-secondary" @click="resetGatewayKey">
                <NIcon :size="14"><RefreshOutline /></NIcon>
                刷新 API Key
              </button>
            </div>
          </div>
          <div class="welcome-right">
            <div class="console-panel">
              <div class="console-panel-head">
                <div>
                  <div class="console-panel-title">运行快照</div>
                  <div class="console-panel-sub">连通度与排障重点</div>
                </div>
              </div>
              <div class="console-grid">
                <div class="console-metric">
                  <div class="label">平均响应</div>
                  <div class="value">{{ stats?.today_avg_duration_ms ? Math.round(stats.today_avg_duration_ms) + 'ms' : '-' }}</div>
                </div>
                <div class="console-metric">
                  <div class="label">今日请求</div>
                  <div class="value">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
                </div>
                <div class="console-metric">
                  <div class="label">Token 消耗</div>
                  <div class="value">{{ stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) + 'K' : '-' }}</div>
                </div>
                <div class="console-metric">
                  <div class="label">错误请求</div>
                  <div class="value" :style="{ color: ((stats?.today_requests || 0) - (stats?.today_success || 0)) > 0 ? 'var(--danger)' : undefined }">{{ (stats?.today_requests || 0) - (stats?.today_success || 0) }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Stat Cards Row -->
      <div class="stat-grid">
        <div class="stat-card stat-card--accent">
          <div class="stat-label">今日请求数</div>
          <div class="stat-value accent">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
          <div class="stat-sub" v-if="stats">
            {{ stats.yesterday_requests ? ((stats.today_requests / stats.yesterday_requests - 1) * 100).toFixed(1) : 0 }}% 较昨日
          </div>
        </div>
        <div class="stat-card stat-card--success">
          <div class="stat-label">平均响应时间</div>
          <div class="stat-value success">{{ Math.round(stats?.today_avg_duration_ms || 0) }}<span class="stat-unit">ms</span></div>
          <div class="stat-sub">网关整体延迟</div>
        </div>
        <div class="stat-card stat-card--indigo">
          <div class="stat-label">今日 Token 消耗</div>
          <div class="stat-value accent">{{ (stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) : '0') + 'K' }}</div>
          <div class="stat-sub">总 Token 用量</div>
        </div>
        <div class="stat-card stat-card--neutral">
          <div class="stat-label">活跃渠道</div>
          <div class="stat-value">{{ stats?.active_providers || 0 }}</div>
          <div class="stat-sub">全部已配置</div>
        </div>
      </div>

      <!-- Main Row: Recent Requests -->
      <div class="dashboard-main-row">
        <div class="card">
          <div class="card-header">
            <h3>最新请求</h3>
            <button class="btn btn-ghost btn-sm" @click="goToLogs">
              全部日志
              <NIcon :size="14"><ArrowForwardOutline /></NIcon>
            </button>
          </div>
          <div class="card-body" style="padding:0">
            <div class="table-wrap">
              <table class="ds-table">
                <thead>
                  <tr>
                    <th>时间</th>
                    <th>方法</th>
                    <th>路径</th>
                    <th>状态</th>
                    <th class="text-right">耗时</th>
                    <th>渠道</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="log in recentLogs" :key="log.id">
                    <td class="text-mono">{{ (log.timestamp || '').length > 8 ? (log.timestamp || '').slice(11, 19) : (log.timestamp || '-') }}</td>
                    <td><span class="method" :class="'method-' + (log.method || 'GET')">{{ log.method || 'GET' }}</span></td>
                    <td class="text-mono text-sm" style="max-width:200px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap">{{ log.path || '-' }}</td>
                    <td><span class="badge" :class="(log.response_status || 0) < 300 ? 'badge-success' : (log.response_status || 0) < 400 ? 'badge-warning' : 'badge-danger'">{{ log.response_status || '-' }}</span></td>
                    <td class="num text-right">{{ formatMs(log.total_duration_ms) }}</td>
                    <td>{{ log.provider_name || log.provider_id || '-' }}</td>
                  </tr>
                  <tr v-if="recentLogs.length === 0 && !logsLoading">
                    <td colspan="6" style="text-align:center; padding:24px; color:var(--muted)">暂无日志记录</td>
                  </tr>
                  <tr v-if="logsLoading">
                    <td colspan="6" style="text-align:center; padding:24px; color:var(--muted)">加载中...</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
</div>
    </div>
  </NSpin>
</template>

<style scoped>
.dashboard {
  width: 100%;
}

/* ===== Welcome Card — 极简白底卡 ===== */
.welcome-card {
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-lg, 10px);
  box-shadow: var(--shadow-card, 0 1px 0 0 rgba(0,0,0,0.02));
  position: relative;
  overflow: hidden;
}

.welcome-body {
  display: grid;
  grid-template-columns: minmax(0, 1.8fr) minmax(320px, 1fr);
  gap: 24px;
  padding: 24px 28px;
}

.welcome-left {
  flex: 1;
  min-width: 0;
}

.welcome-kicker {
  display: block;
  margin-bottom: 10px;
  color: var(--accent);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.welcome-label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 3px 8px;
  border-radius: 999px;
  background: var(--surface-alt, #f5f5f5);
  border: 1px solid var(--border, #e5e5e5);
  font-size: 11px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  color: var(--muted, #737373);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-weight: 600;
}

.welcome-label.online {
  color: var(--success);
  background: var(--success-soft);
  border-color: color-mix(in srgb, var(--success) 22%, var(--surface));
}

.welcome-heading {
  font-size: 20px;
  line-height: 1.4;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin: 14px 0 8px;
  letter-spacing: -0.02em;
}

.welcome-desc {
  color: var(--fg-2, #171717);
  font-size: 13.5px;
  margin: 0;
}

.welcome-meta {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 16px;
}

/* ===== Console Panel — 深色面板（极简） ===== */
.console-panel {
  border-radius: var(--radius-lg, 10px);
  background: var(--sidebar-bg-solid, #0f141c);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: var(--shadow-card, 0 1px 0 0 rgba(0,0,0,0.02));
  padding: 18px 20px;
  position: relative;
  overflow: hidden;
}

.console-panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.console-panel-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--sidebar-active, #f8fafc);
}

.console-panel-sub {
  font-size: 11px;
  color: var(--sidebar-fg, #94a3b8);
  margin-top: 2px;
}

.console-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.console-metric {
  padding: 10px 12px;
  border-radius: var(--radius, 8px);
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  transition: background var(--transition), border-color var(--transition);
}

.console-metric:hover {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.08);
}

.console-metric .label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--sidebar-fg, #94a3b8);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  font-weight: 600;
}

.console-metric .value {
  margin-top: 4px;
  font-size: 16px;
  font-weight: 700;
  color: var(--sidebar-active, #f8fafc);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}

/* ===== Stat Grid — Vercel 风格 4 列 ===== */
.stat-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 20px;
}

.stat-card {
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-lg, 10px);
  padding: 18px 20px;
  box-shadow: var(--shadow-card, 0 1px 0 0 rgba(0,0,0,0.02));
  transition: border-color var(--transition);
  position: relative;
  overflow: hidden;
}

.stat-card::before {
  content: "";
  position: absolute;
  inset: 0 auto auto 0;
  width: 100%;
  height: 2px;
  background: var(--border);
}

.stat-card--accent::before { background: var(--accent); }
.stat-card--success::before { background: var(--success); }
.stat-card--indigo::before { background: var(--brand-2); }
.stat-card--neutral::before { background: var(--muted); }

.stat-card:hover {
  border-color: var(--muted, #a3a3a3);
}

.stat-label {
  font-size: 11px;
  color: var(--muted, #737373);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-weight: 600;
  margin-bottom: 6px;
}

.stat-value {
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  color: var(--fg, #0a0a0a);
}

.stat-value.accent { color: var(--fg, #0a0a0a); }
.stat-value.success { color: var(--success, #16a34a); }

.stat-unit {
  font-size: 14px;
  font-weight: 500;
  opacity: 0.6;
}

.stat-sub {
  font-size: 11px;
  color: var(--muted, #737373);
  margin-top: 4px;
}

/* ===== Dashboard Main Row ===== */
.dashboard-main-row {
  width: 100%;
}

.welcome-actions {
  flex-wrap: wrap;
}

/* ===== Card — Vercel 风格 ===== */
.card {
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-lg, 10px);
  box-shadow: var(--shadow-card, 0 1px 0 0 rgba(0,0,0,0.02));
  overflow: hidden;
}

.card-header {
  padding: 14px 20px;
  border-bottom: 1px solid var(--border, #e5e5e5);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-header h3 {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  color: var(--fg, #0a0a0a);
}

.card-body {
  padding: 16px 20px;
}

/* ===== Buttons — Vercel 风格 ===== */
.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--radius, 8px);
  font-size: 13px;
  font-weight: 600;
  transition: background-color var(--transition), border-color var(--transition), color var(--transition);
  white-space: nowrap;
  cursor: pointer;
  border: 1px solid var(--border, #e5e5e5);
  font-family: inherit;
  background: var(--surface, #ffffff);
  color: var(--fg-2, #171717);
}

.btn :deep(.n-icon) {
  flex: 0 0 auto;
}

.btn-primary {
  background: var(--fg, #0a0a0a);
  color: var(--surface, #ffffff);
  border-color: var(--fg, #0a0a0a);
  box-shadow: var(--shadow-sm, 0 1px 0 0 rgba(0,0,0,0.02));
}

.btn-primary:hover {
  background: var(--fg-2, #171717);
  border-color: var(--fg-2, #171717);
}

.btn-secondary {
  background: var(--surface, #ffffff);
  color: var(--fg-2, #171717);
  border: 1px solid var(--border, #e5e5e5);
}

.btn-secondary:hover {
  background: var(--surface-alt, #f5f5f5);
  border-color: var(--muted, #737373);
}

.btn-ghost {
  color: var(--muted, #737373);
  padding: 6px 10px;
  background: transparent;
  border-color: transparent;
}

.btn-ghost:hover {
  color: var(--fg, #0a0a0a);
  background: var(--surface-alt, #f5f5f5);
}

.btn-sm {
  padding: 5px 10px;
  font-size: 12px;
  border-radius: var(--radius-sm, 6px);
}

.w-full { width: 100%; }

/* ===== Badges — pill 风格 ===== */
.badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  border: 1px solid transparent;
}

.badge-success { background: color-mix(in srgb, var(--success) 12%, var(--surface)); color: var(--success); border-color: color-mix(in srgb, var(--success) 20%, var(--surface)); }
.badge-warning { background: color-mix(in srgb, var(--warn) 12%, var(--surface)); color: var(--warn); border-color: color-mix(in srgb, var(--warn) 20%, var(--surface)); }
.badge-danger { background: color-mix(in srgb, var(--danger) 12%, var(--surface)); color: var(--danger); border-color: color-mix(in srgb, var(--danger) 20%, var(--surface)); }
.badge-neutral { background: var(--surface-alt, #f5f5f5); color: var(--muted, #737373); border-color: var(--border, #e5e5e5); }
.badge-accent { background: color-mix(in srgb, var(--fg) 8%, var(--surface)); color: var(--fg); border-color: color-mix(in srgb, var(--fg) 16%, var(--surface)); }

/* ===== Table — Vercel 风格 ===== */
.table-wrap {
  overflow-x: auto;
}

.ds-table {
  width: 100%;
  border-collapse: collapse;
}

.ds-table th {
  padding: 10px 14px;
  text-align: left;
  font-size: 11px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted, #737373);
  border-bottom: 1px solid var(--border, #e5e5e5);
  background: var(--surface-alt, #f5f5f5);
  font-weight: 600;
}

.ds-table td {
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-soft, #ededed);
  font-size: 13px;
  color: var(--fg-2, #171717);
}

.ds-table tbody tr {
  transition: background var(--transition);
}

.ds-table tbody tr:hover {
  background: color-mix(in srgb, var(--accent) 4%, transparent);
}

.ds-table tbody tr:last-child td {
  border-bottom: none;
}

/* ===== Method Badges — pill 风格 ===== */
.method {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 600;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  border: 1px solid transparent;
}

.method-GET { background: color-mix(in srgb, var(--fg) 8%, var(--surface)); color: var(--fg); border-color: color-mix(in srgb, var(--fg) 14%, var(--surface)); }
.method-POST { background: color-mix(in srgb, var(--success) 12%, var(--surface)); color: var(--success); border-color: color-mix(in srgb, var(--success) 20%, var(--surface)); }
.method-PUT { background: color-mix(in srgb, var(--warn) 12%, var(--surface)); color: var(--warn); border-color: color-mix(in srgb, var(--warn) 20%, var(--surface)); }
.method-DELETE { background: color-mix(in srgb, var(--danger) 12%, var(--surface)); color: var(--danger); border-color: color-mix(in srgb, var(--danger) 20%, var(--surface)); }

/* ===== Status Dot Small ===== */
.status-dot-sm {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
  flex-shrink: 0;
}

.status-dot-sm.online {
  background: var(--success, #16a34a);
}

.status-dot-sm.offline {
  background: var(--muted, #737373);
}

/* ===== Utilities ===== */
.text-mono {
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace) !important;
}

.text-sm {
  font-size: 12px;
}

.text-right {
  text-align: right;
}

.num {
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace) !important;
  font-variant-numeric: tabular-nums;
}

.mb-16 { margin-bottom: 16px; }
.mt-20 { margin-top: 20px; }

.row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.gap-md { gap: 12px; }

.form-stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.rule {
  border: 0;
  border-top: 1px solid var(--border-soft, #ededed);
  margin: 4px 0;
}

@media (max-width: 980px) {
  .welcome-body {
    grid-template-columns: 1fr;
  }

  .console-panel {
    max-width: none;
  }

  .stat-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 560px) {
  .welcome-body {
    padding: 20px;
  }

  .welcome-heading {
    font-size: 18px;
  }

  .welcome-actions .btn {
    flex: 1 1 100%;
    justify-content: center;
  }

  .stat-grid {
    gap: 8px;
  }

  .stat-card {
    padding: 14px;
  }

  .stat-value {
    font-size: 20px;
  }
}
</style>
