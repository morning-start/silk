<script setup lang="ts">
import { ref, onMounted, h, computed } from "vue";
import { formatMs } from "../utils/format";
import { save } from "@tauri-apps/plugin-dialog";
import {
  NButton,
  NDataTable,
  NModal,
  NTag,
  NText,
  NSpace,
  NSelect,
  NPagination,
  NCard,
  NPopconfirm,
  NGrid,
  NGi,
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import { useLogsStore } from "../stores/logs";
import { storeToRefs } from "pinia";
import type { HourlyStats, ProviderStats, RequestLog } from "../api";
import { api } from "../api";

const logsStore = useLogsStore();
const { logs, total, page, totalPages, loading, error } = storeToRefs(logsStore);
const message = useMessage();

const selectedLog = ref<RequestLog | null>(null);
const showDetail = ref(false);

const statusFilter = ref("all");
const metricRange = ref(24);
const providerStats = ref<ProviderStats[]>([]);
const hourlyData = ref<HourlyStats[]>([]);

const columns: DataTableColumns<RequestLog> = [
  {
    title: "时间",
    key: "timestamp",
    width: 150,
    render(row) {
      return h("span", { class: "text-mono text-sm" }, row.timestamp || "-");
    },
  },
  {
    title: "方法",
    key: "method",
    width: 70,
    render(row) {
      const m = row.method || "GET";
      const color = m === "POST" ? "success" : m === "GET" ? "info" : m === "PUT" ? "warning" : "error";
      return h(NTag, { size: "small", type: color as any }, { default: () => m });
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
      if (!row.response_status) return h(NText, { depth: 3 }, { default: () => "-" });
      const type = row.response_status < 300 ? "success" : row.response_status < 400 ? "warning" : "error";
      return h(NTag, { size: "small", type: type as any }, { default: () => row.response_status });
    },
  },
  {
    title: "响应",
    key: "resp_ms",
    width: 65,
    render(row) {
      return h("span", { class: "num" }, formatMs(row.resp_ms));
    },
  },
  {
    title: "耗时",
    key: "total_duration_ms",
    width: 65,
    render(row) {
      return h("span", { class: "num" }, formatMs(row.total_duration_ms));
    },
  },
  {
    title: "Token",
    key: "tokens_sent",
    width: 100,
    render(row) {
      const inp = row.tokens_input || 0;
      const out = row.tokens_output || 0;
      const sent = row.tokens_sent || 0;
      if (inp + out === 0 && sent === 0) return "-";
      // 显示节省：tokens_input - tokens_sent（正数=优化节省）
      const saved = inp > sent ? inp - sent : null;
      const parts = [`↑${((inp + out) / 1000).toFixed(1)}k`];
      if (saved !== null && saved > 0) {
        parts.push(`-${saved}`);
      }
      return h("span", { class: "num" }, parts.join(" "));
    },
  },
  {
    title: "费用",
    key: "cost",
    width: 80,
    render(row) {
      if (row.cost == null) return h("span", { class: "num" }, "-");
      return h("span", { class: "num" }, `$${row.cost.toFixed(6)}`);
    },
  },
  { title: "模型", key: "model_id", width: 110,
    render(row) {
      if (row.model_name) return `${row.model_name} (${row.model_id})`;
      return row.model_id || "-";
    },
  },
  { title: "渠道", key: "provider_name", width: 120,
    render(row) {
      const label = row.provider_name || row.provider_id || "-";
      return h("span", { class: "text-sm" }, label);
    },
  },
  {
    title: "重试",
    key: "retry_count",
    width: 60,
    render(row) {
      if (row.retry_count === 0) return h("span", { class: "num" }, "-");
      return h(NTag, { size: "small", type: "warning" }, { default: () => row.retry_count });
    },
  },
  {
    title: "流式",
    key: "stream_enabled",
    width: 60,
    render(row) {
      return h(NTag, { size: "small", type: row.stream_enabled ? "info" : "default" }, {
        default: () => row.stream_enabled ? "是" : "否",
      });
    },
  },
  {
    title: "操作",
    key: "actions",
    width: 70,
    render(row) {
      return h(NButton, { size: "small", quaternary: true, onClick: () => handleViewDetail(row) }, { default: () => "详情" });
    },
  },
];

const filteredLogs = computed(() => {
  if (statusFilter.value === "all") return logs.value;
  const prefix = statusFilter.value.charAt(0);
  return logs.value.filter((log) => {
    const status = log.response_status;
    if (status == null) return false;
    const statusStr = String(status);
    return statusStr.startsWith(prefix);
  });
});

function handleViewDetail(row: RequestLog) {
  selectedLog.value = row;
  showDetail.value = true;
}

function copyDetail() {
  if (!selectedLog.value) return;
  const text = JSON.stringify(selectedLog.value, null, 2);
  navigator.clipboard.writeText(text).then(() => {
    message.success("已复制");
  }).catch(() => {
    message.error("复制失败");
  });
}

async function handleCleanup() {
  try {
    await logsStore.cleanup(7);
    message.success("已清理 7 天前的日志");
  } catch {
    message.error("清理失败");
  }
}

async function handleClearAll() {
  try {
    await logsStore.clearAll();
    message.success("已清空所有日志");
  } catch {
    message.error("清空失败");
  }
}

async function handleExportCsv() {
  try {
    const filePath = await save({
      title: "导出日志 CSV",
      defaultPath: `silk_logs_${new Date().toISOString().slice(0, 10)}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!filePath) return;
    const result = await api.exportLogsCsv({ limit: 10000, file_path: filePath });
    message.success(`已导出 ${result.exported_count} 条日志到 ${result.file_path}`);
  } catch {
    message.error("导出失败");
  }
}

async function loadStats(hours = metricRange.value) {
  metricRange.value = hours;
  try {
    const [hourly, providers] = await Promise.all([
      api.hourlyStats(hours),
      api.statsByProvider(5),
    ]);
    hourlyData.value = hourly;
    providerStats.value = providers;
  } catch {
    // keep logs usable even if stats fail
  }
}

const totalRequestsInRange = computed(() =>
  hourlyData.value.reduce((sum, item) => sum + item.request_count, 0)
);

const averageDurationInRange = computed(() => {
  if (hourlyData.value.length === 0) return 0;
  return Math.round(
    hourlyData.value.reduce((sum, item) => sum + item.avg_duration_ms, 0) / hourlyData.value.length
  );
});

const totalTokensInRange = computed(() =>
  hourlyData.value.reduce((sum, item) => sum + item.total_tokens, 0)
);

const estimatedCostInRange = computed(() => totalTokensInRange.value / 1_000_000 * 3);

const activeProvidersInRange = computed(() =>
  providerStats.value.filter((item) => item.request_count > 0).length
);

const paginationText = computed(() => {
  const start = (page.value - 1) * 50 + 1;
  const end = Math.min(page.value * 50, total.value);
  return `显示 ${start}-${end} / ${total.value.toLocaleString()} 条`;
});

onMounted(() => {
  logsStore.fetchAll();
  loadStats();
});
</script>

<template>
  <div class="logs-page">
    <NGrid :x-gap="16" :y-gap="16" :cols="5" class="mb-16">
      <NGi>
        <NCard :bordered="false" class="metric-card">
          <div class="stat-label">请求数 ({{ metricRange }}h)</div>
          <div class="stat-value accent">{{ totalRequestsInRange.toLocaleString() }}</div>
          <div class="stat-sub">本地网关总请求</div>
        </NCard>
      </NGi>
      <NGi>
        <NCard :bordered="false" class="metric-card">
          <div class="stat-label">平均响应</div>
          <div class="stat-value success">{{ averageDurationInRange }}<span class="stat-unit">ms</span></div>
          <div class="stat-sub">轻量监控</div>
        </NCard>
      </NGi>
      <NGi>
        <NCard :bordered="false" class="metric-card">
          <div class="stat-label">Token 消耗</div>
          <div class="stat-value accent">{{ (totalTokensInRange / 1000).toFixed(1) }}<span class="stat-unit">K</span></div>
          <div class="stat-sub">最近 {{ metricRange }} 小时</div>
        </NCard>
      </NGi>
      <NGi>
        <NCard :bordered="false" class="metric-card">
          <div class="stat-label">估算费用</div>
          <div class="stat-value">${{ estimatedCostInRange.toFixed(2) }}</div>
          <div class="stat-sub">按 $3/1M tokens</div>
        </NCard>
      </NGi>
      <NGi>
        <NCard :bordered="false" class="metric-card">
          <div class="stat-label">活跃渠道</div>
          <div class="stat-value">{{ activeProvidersInRange }}</div>
          <div class="stat-sub">最近 {{ metricRange }} 小时</div>
        </NCard>
      </NGi>
    </NGrid>

    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">请求日志</h2>
        <NTag size="small" type="info">共 {{ total.toLocaleString() }} 条</NTag>
      </div>
      <div class="toolbar-right">
        <NSpace :size="8">
          <NSelect
            v-model:value="metricRange"
            :options="[
              { label: '最近 1 小时', value: 1 },
              { label: '最近 24 小时', value: 24 },
              { label: '最近 7 天', value: 168 },
            ]"
            style="width: 130px"
            size="small"
            @update:value="loadStats"
          />
          <NSelect
            v-model:value="statusFilter"
            :options="[
              { label: '全部状态', value: 'all' },
              { label: '2xx 成功', value: '2xx' },
              { label: '4xx 客户端错误', value: '4xx' },
              { label: '5xx 服务端错误', value: '5xx' },
            ]"
            style="width: 140px"
            size="small"
          />
          <NButton secondary size="small" @click="() => { logsStore.fetchAll(); loadStats(); }">刷新</NButton>
          <NButton secondary size="small" @click="handleExportCsv">导出 CSV</NButton>
          <NButton secondary size="small" @click="handleCleanup">清理 7 天前</NButton>
          <NPopconfirm @positive-click="handleClearAll">
            <template #trigger>
              <NButton type="error" size="small">清空全部</NButton>
            </template>
            确定要清空所有日志吗？此操作不可恢复。
          </NPopconfirm>
        </NSpace>
      </div>
    </div>

    <NCard :bordered="false" class="table-card" size="small">
      <!-- Error State -->
      <template v-if="error">
        <div class="error-state">
          <div class="error-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <h3 class="error-title">数据加载失败</h3>
          <p class="error-desc">{{ error }}</p>
          <NButton type="primary" @click="logsStore.fetchAll()">重新加载</NButton>
        </div>
      </template>
      <!-- Empty State -->
      <template v-else-if="!loading && filteredLogs.length === 0">
        <div class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#94a3b8"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>
          </div>
          <h3 class="empty-title">暂无日志记录</h3>
          <p class="empty-desc">启动网关并发送请求后，日志将实时显示在这里</p>
        </div>
      </template>
      <NDataTable
        v-else
        :columns="columns"
        :data="filteredLogs"
        :loading="loading"
        :bordered="false"
        :single-line="false"
        striped
        size="small"
      />
    </NCard>

    <NCard
      v-if="providerStats.length > 0"
      title="渠道请求占比"
      :bordered="false"
      class="table-card provider-card"
      size="small"
    >
      <div class="provider-summary-list">
        <div v-for="item in providerStats" :key="item.provider_name || 'unknown'" class="provider-summary-item">
          <div class="provider-summary-head">
            <span class="provider-summary-name">{{ item.provider_name || "未知渠道" }}</span>
            <span class="provider-summary-count">{{ item.request_count.toLocaleString() }} 次</span>
          </div>
          <div class="provider-summary-meta">
            <span>平均 {{ item.avg_duration_ms }}ms</span>
            <span>{{ (item.total_tokens / 1000).toFixed(1) }}K tokens</span>
          </div>
        </div>
      </div>
    </NCard>

    <div class="pagination-bar">
      <NText depth="3" style="font-size: 13px">{{ paginationText }}</NText>
      <NPagination
        v-model:page="page"
        :page-count="totalPages"
        :page-size="50"
        @update:page="logsStore.fetchPage"
      />
    </div>

    <!-- Log Detail Modal -->
    <NModal
      v-model:show="showDetail"
      preset="card"
      title="日志详情"
      style="max-width: 700px"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <div v-if="selectedLog" class="log-detail">
        <div class="detail-row">
          <span class="detail-label">Request ID：</span>
          <span class="detail-value text-mono">{{ selectedLog.request_id }}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">时间：</span>
          <span class="detail-value">{{ selectedLog.timestamp }}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">请求：</span>
          <NTag size="small" type="info">{{ selectedLog.method }}</NTag>
          <span class="detail-value text-mono" style="margin-left: 8px">{{ selectedLog.path }}</span>
        </div>
        <div class="detail-row" v-if="selectedLog.response_status">
          <span class="detail-label">状态码：</span>
          <NTag size="small" :type="selectedLog.response_status < 300 ? 'success' : 'error'">
            {{ selectedLog.response_status }}
          </NTag>
        </div>
        <div class="detail-row" v-if="selectedLog.resp_ms != null">
          <span class="detail-label">响应：</span>
          <span class="detail-value">{{ selectedLog.resp_ms }}ms</span>
        </div>
        <div class="detail-row" v-if="selectedLog.total_duration_ms != null">
          <span class="detail-label">耗时：</span>
          <span class="detail-value">{{ selectedLog.total_duration_ms }}ms</span>
        </div>
        <div class="detail-row" v-if="selectedLog.inbound_protocol || selectedLog.outbound_protocol">
          <span class="detail-label">协议(入→出)：</span>
          <NTag size="small">{{ selectedLog.inbound_protocol || '-' }}</NTag>
          <span style="margin: 0 4px">→</span>
          <NTag size="small">{{ selectedLog.outbound_protocol || '-' }}</NTag>
        </div>
        <div class="detail-row" v-if="selectedLog.route_id">
          <span class="detail-label">路由规则：</span>
          <span class="detail-value text-mono">{{ selectedLog.route_id }}</span>
        </div>
        <div class="detail-row" v-if="selectedLog.provider_id">
          <span class="detail-label">渠道：</span>
          <span class="detail-value">{{ selectedLog.provider_name || selectedLog.provider_id }}</span>
          <span v-if="selectedLog.provider_name && selectedLog.provider_name !== selectedLog.provider_id" class="detail-value text-mono" style="color: #94a3b8; font-size: 12px">({{ selectedLog.provider_id }})</span>
        </div>
        <div class="detail-row" v-if="selectedLog.model_id || selectedLog.model_name">
          <span class="detail-label">模型：</span>
          <span class="detail-value">{{ selectedLog.model_name || selectedLog.model_id }}</span>
          <span v-if="selectedLog.model_name && selectedLog.model_id && selectedLog.model_name !== selectedLog.model_id" class="detail-value text-mono" style="color: #94a3b8; font-size: 12px">({{ selectedLog.model_id }})</span>
        </div>
        <div class="detail-row" v-if="selectedLog.auth_key_name">
          <span class="detail-label">认证 Key：</span>
          <NTag size="small" type="success">{{ selectedLog.auth_key_name }}</NTag>
        </div>
        <div class="detail-row" v-if="selectedLog.channel_key_name">
          <span class="detail-label">渠道 Key：</span>
          <NTag size="small" type="info">{{ selectedLog.channel_key_name }}</NTag>
        </div>
        <div class="detail-row">
          <span class="detail-label">请求大小：</span>
          <span class="detail-value">{{ (selectedLog.request_size_bytes || 0).toLocaleString() }} bytes</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">响应大小：</span>
          <span class="detail-value">{{ (selectedLog.response_size_bytes || 0).toLocaleString() }} bytes</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Token (输入/发送/输出)：</span>
          <span class="detail-value">
            {{ selectedLog.tokens_input || 0 }} / {{ selectedLog.tokens_sent || 0 }} / {{ selectedLog.tokens_output || 0 }}
            <span v-if="selectedLog.tokens_input != null && selectedLog.tokens_sent != null && selectedLog.tokens_input > selectedLog.tokens_sent" style="color: var(--success, #10b981); margin-left: 8px">
              优化 -{{ selectedLog.tokens_input - selectedLog.tokens_sent }}
            </span>
          </span>
        </div>
        <div class="detail-row" v-if="selectedLog.cost != null">
          <span class="detail-label">费用：</span>
          <span class="detail-value num">${{ selectedLog.cost.toFixed(8) }}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">流式：</span>
          <NTag size="small" :type="selectedLog.stream_enabled ? 'info' : 'default'">{{ selectedLog.stream_enabled ? '是' : '否' }}</NTag>
        </div>
        <div class="detail-row">
          <span class="detail-label">缓存命中：</span>
          <NTag size="small" :type="selectedLog.cache_hit ? 'success' : 'default'">{{ selectedLog.cache_hit ? '是' : '否' }}</NTag>
        </div>
        <div class="detail-row" v-if="selectedLog.retry_count > 0">
          <span class="detail-label">重试次数：</span>
          <NTag size="small" type="warning">{{ selectedLog.retry_count }}</NTag>
        </div>
        <div class="detail-row" v-if="selectedLog.error_message">
          <span class="detail-label" style="color: #ef4444">错误：</span>
          <span class="detail-value" style="color: #ef4444">{{ selectedLog.error_message }}</span>
        </div>
        <div class="detail-row" v-if="selectedLog.error_code">
          <span class="detail-label">错误码：</span>
          <NTag size="small" type="error">{{ selectedLog.error_code }}</NTag>
        </div>
      </div>
      <template #footer>
        <div style="display: flex; justify-content: flex-end; gap: 8px">
          <NButton size="small" @click="copyDetail">复制 JSON</NButton>
          <NButton size="small" @click="showDetail = false">关闭</NButton>
        </div>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.logs-page {
  width: 100%;
}

.metric-card {
  border-radius: 12px;
}

.stat-label {
  font-size: 13px;
  color: var(--text-color-3, #64748b);
  margin-bottom: 4px;
}

.stat-value {
  font-size: 24px;
  font-weight: 700;
  line-height: 1.2;
  margin-bottom: 4px;
}

.stat-value.accent {
  color: var(--accent, #0891b2);
}

.stat-value.success {
  color: var(--success, #10b981);
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

.pagination-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 16px;
}

.provider-card {
  margin-top: 16px;
}

.provider-summary-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.provider-summary-item {
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 10px;
  padding: 12px 14px;
  background: var(--card-color, #ffffff);
}

.provider-summary-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 6px;
}

.provider-summary-name {
  font-size: 13px;
  font-weight: 600;
}

.provider-summary-count,
.provider-summary-meta {
  font-size: 12px;
  color: var(--text-color-3, #64748b);
}

.provider-summary-meta {
  display: flex;
  gap: 12px;
}

.log-detail {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.detail-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.detail-label {
  font-size: 13px;
  font-weight: 500;
  min-width: 110px;
  color: var(--text-color-2, #475569);
}

.detail-value {
  font-size: 13px;
}
</style>
