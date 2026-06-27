<script setup lang="ts">
import { ref, onMounted, h } from "vue";
import { useRouter } from "vue-router";
import {
  NCard,
  NGrid,
  NGi,
  NDataTable,
  NButton,
  NTag,
  NText,
  NSpace,
  NSpin,
  NIcon,
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import {
  ArrowForwardOutline,
  ReloadOutline,
  StopOutline,
  PlayOutline,
  RocketOutline,
} from "@vicons/ionicons5";
import { api, type DashboardStats, type RequestLog } from "../api";
import { useGatewayStore } from "../stores/gateway";

const router = useRouter();
const gatewayStore = useGatewayStore();
const message = useMessage();

const loading = ref(false);
const error = ref<string | null>(null);
const stats = ref<DashboardStats | null>(null);
const recentLogs = ref<RequestLog[]>([]);
const logsLoading = ref(false);

const columns: DataTableColumns<RequestLog> = [
  {
    title: "时间",
    key: "timestamp",
    width: 130,
    render(row) {
      const t = row.timestamp || "";
      return h("span", { class: "text-mono" }, t.length > 8 ? t.slice(11, 19) : t);
    },
  },
  {
    title: "方法",
    key: "method",
    width: 70,
    render(row) {
      const method = row.method || "GET";
      const color = method === "POST" ? "success" : method === "GET" ? "info" : method === "PUT" ? "warning" : "error";
      return h(NTag, { size: "small", type: color as any }, { default: () => method });
    },
  },
  {
    title: "路径",
    key: "path",
    ellipsis: { tooltip: true },
    render(row) {
      return h("span", { class: "text-mono text-sm" }, row.path || "-");
    },
  },
  {
    title: "状态",
    key: "response_status",
    width: 70,
    render(row) {
      const status = row.response_status;
      if (!status) return h(NText, { depth: 3 }, { default: () => "-" });
      const type = status < 300 ? "success" : status < 400 ? "warning" : "error";
      return h(NTag, { size: "small", type: type as any }, { default: () => status });
    },
  },
  {
    title: "耗时",
    key: "duration_ms",
    width: 70,
    render(row) {
      const ms = row.duration_ms;
      if (ms == null) return "-";
      return h("span", { class: "num" }, `${ms}ms`);
    },
  },
  { title: "渠道", key: "provider_id", width: 90 },
];

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

async function startGateway() {
  try {
    await gatewayStore.start();
    message.success("网关已启动");
  } catch {
    message.error("启动失败");
  }
}

async function stopGateway() {
  try {
    await gatewayStore.stop();
    message.success("网关已停止");
  } catch {
    message.error("停止失败");
  }
}

async function restartGateway() {
  try {
    await gatewayStore.restart();
    message.success("网关已重启");
  } catch {
    message.error("重启失败");
  }
}

onMounted(() => {
  loadData();
  gatewayStore.fetchStatus();
});
</script>

<template>
  <NSpin :show="loading" style="min-height: 300px">
    <div class="dashboard">
      <!-- Error Banner -->
      <div v-if="error" class="error-state" style="margin-bottom: 16px">
        <div class="error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h3 class="error-title">仪表盘数据加载失败</h3>
        <p class="error-desc">{{ error }}</p>
        <NButton type="primary" @click="loadData">重新加载</NButton>
      </div>

      <!-- Welcome Card - Empty state -->
      <NCard class="welcome-card mb-16" :bordered="false">
        <div class="welcome-inner">
          <template v-if="gatewayStore.status?.running">
            <div class="welcome-text">
              <h2 class="welcome-title">欢迎回来 👋</h2>
              <p class="welcome-desc">
                网关运行正常，今日已处理
                <span class="welcome-num">{{ stats?.today_requests?.toLocaleString() || 0 }}</span>
                个请求
              </p>
            </div>
            <NSpace>
              <NButton quaternary @click="goToLogs">查看日志</NButton>
              <NButton type="primary" @click="goToProviders">管理渠道</NButton>
            </NSpace>
          </template>
          <template v-else>
            <div class="welcome-text">
              <h2 class="welcome-title">欢迎使用 Silk Gateway 🚀</h2>
              <p class="welcome-desc">
                网关尚未启动，点击右侧按钮开始使用 —— 配置好渠道后即可启动网关转发请求
              </p>
            </div>
            <NSpace>
              <NButton type="primary" @click="startGateway">
                <template #icon><NIcon><RocketOutline /></NIcon></template>
                启动网关
              </NButton>
              <NButton quaternary @click="goToProviders">配置渠道</NButton>
            </NSpace>
          </template>
        </div>
      </NCard>

      <!-- Stat Cards -->
      <NGrid :x-gap="16" :y-gap="16" :cols="4" class="mb-16">
        <NGi>
          <NCard class="stat-card" :bordered="false">
            <div class="stat-label">今日请求数</div>
            <div class="stat-value accent">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
            <div class="stat-sub" v-if="stats">
              {{ stats.yesterday_requests ? ((stats.today_requests / stats.yesterday_requests - 1) * 100).toFixed(1) : 0 }}% 较昨日
            </div>
          </NCard>
        </NGi>
        <NGi>
          <NCard class="stat-card" :bordered="false">
            <div class="stat-label">平均响应时间</div>
            <div class="stat-value success">{{ stats?.today_avg_duration_ms || 0 }}<span class="stat-unit">ms</span></div>
            <div class="stat-sub">网关整体延迟</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard class="stat-card" :bordered="false">
            <div class="stat-label">今日 Token 消耗</div>
            <div class="stat-value accent">{{ (stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) : '0') + 'K' }}</div>
            <div class="stat-sub">总 Token 用量</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard class="stat-card" :bordered="false">
            <div class="stat-label">活跃渠道</div>
            <div class="stat-value">{{ stats?.active_providers || 0 }}</div>
            <div class="stat-sub">全部已配置</div>
          </NCard>
        </NGi>
      </NGrid>

      <!-- Recent Requests + Gateway Control -->
      <NGrid :x-gap="16" :y-gap="16" :cols="3">
        <NGi :span="2">
          <NCard
            title="最近请求"
            size="small"
            :bordered="false"
            class="section-card"
          >
            <template #header-extra>
              <NButton text size="small" @click="goToLogs">
                查看全部
                <template #icon>
                  <NIcon size="14"><ArrowForwardOutline /></NIcon>
                </template>
              </NButton>
            </template>
            <NDataTable
              :columns="columns"
              :data="recentLogs"
              :loading="logsLoading"
              :bordered="false"
              :single-line="false"
              size="small"
              style="margin: -12px"
              :row-class-name="() => 'dashboard-row'"
            />
          </NCard>
        </NGi>
        <NGi>
          <NCard
            title="网关控制"
            size="small"
            :bordered="false"
            class="section-card"
          >
            <div class="gateway-control-body">
              <div class="gateway-control-info">
                <div class="control-label">运行状态</div>
                <div class="control-status">
                  <span class="status-indicator" :class="{ active: gatewayStore.status?.running }"></span>
                  <span :class="{ 'text-success': gatewayStore.status?.running }">
                    {{ gatewayStore.status?.running ? '运行中' : '已停止' }}
                  </span>
                </div>
                <div class="control-detail" v-if="gatewayStore.status?.running">
                  监听地址：<span class="text-mono">{{ gatewayStore.status?.address }}</span>
                </div>
              </div>
              <NSpace vertical style="width: 100%">
                <template v-if="gatewayStore.status?.running">
                  <NButton block secondary @click="restartGateway">
                    <template #icon><NIcon><ReloadOutline /></NIcon></template>
                    重启网关
                  </NButton>
                  <NButton block type="error" @click="stopGateway">
                    <template #icon><NIcon><StopOutline /></NIcon></template>
                    停止网关
                  </NButton>
                </template>
                <template v-else>
                  <NButton block type="primary" @click="startGateway">
                    <template #icon><NIcon><PlayOutline /></NIcon></template>
                    启动网关
                  </NButton>
                </template>
              </NSpace>
            </div>
          </NCard>
        </NGi>
      </NGrid>
    </div>
  </NSpin>
</template>

<style scoped>
.dashboard {
  max-width: 1200px;
}

.mb-16 {
  margin-bottom: 16px;
}

.welcome-card {
  background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%) !important;
  border-radius: 12px;
}

.welcome-inner {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.welcome-title {
  font-size: 20px;
  font-weight: 700;
  color: #ffffff;
  margin: 0 0 4px;
}

.welcome-desc {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.85);
  margin: 0;
}

.welcome-num {
  font-weight: 600;
  color: #ffffff;
}

.stat-card {
  border-radius: 12px;
}

.stat-label {
  font-size: 13px;
  color: var(--text-color-3, #64748b);
  margin-bottom: 4px;
}

.stat-value {
  font-size: 28px;
  font-weight: 700;
  line-height: 1.2;
  margin-bottom: 4px;
}

.stat-value.accent {
  color: #6366f1;
}

.stat-value.success {
  color: #22c55e;
}

.stat-unit {
  font-size: 14px;
  font-weight: 500;
  opacity: 0.6;
}

.stat-sub {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.section-card {
  border-radius: 12px;
}

.text-mono {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-size: 12px;
}

.text-sm {
  font-size: 12px;
}

.num {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-size: 12px;
}

.gateway-control-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 8px 0;
}

.gateway-control-info {
  text-align: center;
  width: 100%;
}

.control-label {
  font-size: 13px;
  color: var(--text-color-3, #64748b);
  margin-bottom: 8px;
}

.control-status {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 8px;
}

.status-indicator {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #94a3b8;
  transition: all 0.3s;
}

.status-indicator.active {
  background: #22c55e;
  box-shadow: 0 0 0 4px rgba(34, 197, 94, 0.2);
}

.text-success {
  color: #22c55e;
}

.control-detail {
  font-size: 12px;
  color: var(--text-color-3, #64748b);
}

:deep(.dashboard-row td) {
  padding: 8px 12px !important;
}
</style>
