<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
import { useRouter } from "vue-router";
import { formatMs } from "../utils/format";
import {
  NSpin,
  useMessage,
} from "naive-ui";
import { api, type DashboardStats, type RequestLog } from "../api";
import { useGatewayStore } from "../stores/gateway";
import { useDataChangeSignal } from "../composables/useCrossStoreNotify";

const router = useRouter();
const gatewayStore = useGatewayStore();
const message = useMessage();

const loading = ref(false);
const error = ref<string | null>(null);
const stats = ref<DashboardStats | null>(null);
const recentLogs = ref<RequestLog[]>([]);
const logsLoading = ref(false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:9876");

async function loadData() {
  loading.value = true;
  logsLoading.value = true;
  error.value = null;
  try {
    const [s, logs] = await Promise.all([
      api.dashboardStats(),
      api.recentRequests(10),
    ]);
    stats.value = s;
    recentLogs.value = logs;
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

function goToProviders() {
  router.push("/providers");
}

function copyGatewayAddress() {
  navigator.clipboard.writeText(`http://${bindAddress.value}`).then(() => {
    message.success("本地地址已复制");
  }).catch(() => {
    message.error("复制失败");
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
            <span class="welcome-label">
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
            <div class="row gap-md mt-20">
              <button class="btn btn-primary" @click="copyGatewayAddress">复制本地 API 地址</button>
              <button class="btn btn-secondary" @click="goToProviders">管理渠道</button>
              <button class="btn btn-secondary" @click="goToLogs">查看日志</button>
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
        <div class="stat-card">
          <div class="stat-label">今日请求数</div>
          <div class="stat-value accent">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
          <div class="stat-sub" v-if="stats">
            {{ stats.yesterday_requests ? ((stats.today_requests / stats.yesterday_requests - 1) * 100).toFixed(1) : 0 }}% 较昨日
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-label">平均响应时间</div>
          <div class="stat-value success">{{ Math.round(stats?.today_avg_duration_ms || 0) }}<span class="stat-unit">ms</span></div>
          <div class="stat-sub">网关整体延迟</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">今日 Token 消耗</div>
          <div class="stat-value accent">{{ (stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) : '0') + 'K' }}</div>
          <div class="stat-sub">总 Token 用量</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">活跃渠道</div>
          <div class="stat-value">{{ stats?.active_providers || 0 }}</div>
          <div class="stat-sub">全部已配置</div>
        </div>
      </div>

      <!-- Main Row: Recent Requests + Quick Actions -->
      <div class="dashboard-main-row">
        <div class="card">
          <div class="card-header">
            <h3>最新请求</h3>
            <button class="btn btn-ghost btn-sm" @click="goToLogs">全部日志 →</button>
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
        <div class="card">
          <div class="card-header"><h3>快捷操作</h3></div>
          <div class="card-body form-stack">
            <button class="btn btn-secondary w-full" style="justify-content:flex-start; gap:10px" @click="goToProviders">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:16px;height:16px;flex-shrink:0"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
              管理渠道
            </button>
            <button class="btn btn-secondary w-full" style="justify-content:flex-start; gap:10px" @click="copyGatewayAddress">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:16px;height:16px;flex-shrink:0"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>
              复制 API 地址
            </button>
            <button class="btn btn-secondary w-full" style="justify-content:flex-start; gap:10px" @click="goToLogs">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:16px;height:16px;flex-shrink:0"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M16 13H8M16 17H8M10 9H8"/></svg>
              查看日志
            </button>
            <hr class="rule">
            <div style="font-size:12px; color:var(--muted); line-height:1.6">
              <strong>提示：</strong>
              <span v-if="gatewayStore.status?.running">网关运行正常。</span>
              <span v-else>网关未启动，请先启动网关。</span>
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

/* ===== Welcome Card — design spec ===== */
.welcome-card {
  background: linear-gradient(135deg, #f8fafc, #f1f5f9);
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius-lg, 12px);
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

.welcome-label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--accent-soft, rgba(8, 145, 178, 0.08));
  border: 1px solid rgba(8, 145, 178, 0.15);
  font-size: 11px;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  color: var(--accent, #0891b2);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-weight: 500;
}

.welcome-heading {
  font-size: 20px;
  line-height: 1.4;
  font-weight: 600;
  color: var(--fg, #0f172a);
  margin: 16px 0 10px;
}

.welcome-desc {
  color: var(--fg-2, #334155);
  font-size: 13.5px;
  margin: 0;
}

.welcome-meta {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 18px;
}

/* ===== Console Panel — design spec ===== */
.console-panel {
  border-radius: var(--radius-lg, 12px);
  background: var(--sidebar-bg, #0f172a);
  border: 1px solid rgba(255, 255, 255, 0.08);
  padding: 18px;
}

.console-panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
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
  gap: 10px;
}

.console-metric {
  padding: 10px 12px;
  border-radius: var(--radius, 8px);
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

.console-metric .label {
  font-size: 9px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--sidebar-fg, #94a3b8);
  font-family: 'JetBrains Mono', ui-monospace, monospace;
}

.console-metric .value {
  margin-top: 4px;
  font-size: 16px;
  font-weight: 700;
  color: var(--sidebar-active, #f8fafc);
  font-family: 'JetBrains Mono', ui-monospace, monospace;
}

/* ===== Stat Grid — design spec ===== */
.stat-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 24px;
}

.stat-card {
  background: var(--surface, #ffffff);
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius-lg, 12px);
  padding: 20px;
  box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.05));
}

.stat-label {
  font-size: 11px;
  color: var(--muted, #64748b);
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 8px;
}

.stat-value {
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  color: var(--fg, #0f172a);
}

.stat-value.accent { color: var(--accent, #0891b2); }
.stat-value.success { color: var(--success, #10b981); }

.stat-unit {
  font-size: 14px;
  font-weight: 500;
  opacity: 0.6;
}

.stat-sub {
  font-size: 11px;
  color: var(--muted, #64748b);
  margin-top: 4px;
}

/* ===== Dashboard Main Row — design spec ===== */
.dashboard-main-row {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 16px;
}

/* ===== Card — design spec ===== */
.card {
  background: var(--surface, #ffffff);
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius-lg, 12px);
  box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.05));
  overflow: hidden;
}

.card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-soft, #e2e8f0);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-header h3 {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
}

.card-body {
  padding: 20px;
}

/* ===== Buttons — design spec ===== */
.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--radius, 8px);
  font-size: 13px;
  font-weight: 500;
  transition: all 150ms ease;
  white-space: nowrap;
  cursor: pointer;
  border: none;
  font-family: inherit;
}

.btn-primary {
  background: var(--accent, #0891b2);
  color: #ffffff;
}

.btn-primary:hover {
  background: var(--accent-hover, #0e7490);
}

.btn-secondary {
  background: var(--surface, #ffffff);
  color: var(--fg, #0f172a);
  border: 1px solid var(--border, #cbd5e1);
}

.btn-secondary:hover {
  border-color: var(--fg-2, #334155);
  background: var(--bg, #f8fafc);
}

.btn-ghost {
  color: var(--muted, #64748b);
  padding: 6px 10px;
  background: transparent;
  border: none;
}

.btn-ghost:hover {
  color: var(--fg, #0f172a);
  background: var(--surface-alt, #f1f5f9);
}

.btn-sm {
  padding: 5px 10px;
  font-size: 12px;
  border-radius: 6px;
}

.w-full { width: 100%; }

/* ===== Badges — design spec ===== */
.badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
}

.badge-success { background: var(--success-soft, rgba(16,185,129,0.1)); color: var(--success, #10b981); }
.badge-warning { background: var(--warn-soft, rgba(245,158,11,0.1)); color: var(--warn, #f59e0b); }
.badge-danger { background: var(--danger-soft, rgba(239,68,68,0.1)); color: var(--danger, #ef4444); }
.badge-neutral { background: var(--surface-alt, #f1f5f9); color: var(--muted, #64748b); border: 1px solid var(--border-soft, #e2e8f0); }
.badge-accent { background: var(--accent-soft, rgba(8,145,178,0.08)); color: var(--accent, #0891b2); }

/* ===== Table — design spec ===== */
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
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted, #64748b);
  border-bottom: 1px solid var(--border-soft, #e2e8f0);
  background: var(--bg, #f8fafc);
  font-weight: 500;
}

.ds-table td {
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-soft, #e2e8f0);
  font-size: 13px;
  color: var(--fg-2, #334155);
}

.ds-table tbody tr {
  transition: background 150ms ease;
}

.ds-table tbody tr:hover {
  background: var(--surface-alt, #f1f5f9);
}

/* ===== Method Badges — design spec ===== */
.method {
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 600;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
}

.method-GET { background: #dbeafe; color: #1d4ed8; }
.method-POST { background: #dcfce7; color: #15803d; }
.method-PUT { background: #fef3c7; color: #b45309; }
.method-DELETE { background: #fee2e2; color: #dc2626; }

/* ===== Status Dot Small — design spec ===== */
.status-dot-sm {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}

.status-dot-sm.online { background: var(--success, #10b981); }
.status-dot-sm.offline { background: var(--muted, #94a3b8); }

/* ===== Utilities ===== */
.text-mono {
  font-family: 'JetBrains Mono', ui-monospace, monospace !important;
}

.text-sm {
  font-size: 12px;
}

.text-right {
  text-align: right;
}

.num {
  font-family: 'JetBrains Mono', ui-monospace, monospace !important;
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
  border-top: 1px solid var(--border-soft, #e2e8f0);
  margin: 4px 0;
}
</style>
