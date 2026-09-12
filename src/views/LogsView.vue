<script setup lang="ts">
import { ref, onMounted, h, computed } from "vue";
import { formatMs } from "../utils/format";
import { save } from "@tauri-apps/plugin-dialog";
import { copyWithFeedback } from "../utils/clipboard";
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
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import { useLogsStore } from "../stores/logs";
import { storeToRefs } from "pinia";
import type { HourlyStats, ProviderStats, RequestLog } from "../api";
import { logsApi } from "../api/logs";
import { statsApi } from "../api/stats";
import ModalAdvanced from "../components/ModalAdvanced.vue";

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

async function copyDetail() {
  if (!selectedLog.value) return;
  const text = JSON.stringify(selectedLog.value, null, 2);
  const ok = await copyWithFeedback(text, "日志详情");
  if (ok) {
    message.success("已复制");
  } else {
    message.error("复制失败");
  }
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
    const result = await logsApi.exportCsv({ limit: 10000, file_path: filePath });
    message.success(`已导出 ${result.exported_count} 条日志到 ${result.file_path}`);
  } catch {
    message.error("导出失败");
  }
}

async function loadStats(hours = metricRange.value) {
  metricRange.value = hours;
  try {
    const [hourly, providers] = await Promise.all([
      statsApi.hourly(hours),
      statsApi.byProvider(5),
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
    <!-- 顶部标题 + 快捷指标 -->
    <div class="page-header">
      <div class="page-header-left">
        <h2 class="page-title">请求日志</h2>
        <NTag size="small" type="info">共 {{ total.toLocaleString() }} 条</NTag>
      </div>
      <div class="page-header-stats">
        <div class="mini-stat">
          <span class="mini-stat-label">请求</span>
          <span class="mini-stat-val accent">{{ totalRequestsInRange.toLocaleString() }}</span>
        </div>
        <div class="mini-stat-divider"></div>
        <div class="mini-stat">
          <span class="mini-stat-label">平均</span>
          <span class="mini-stat-val success">{{ averageDurationInRange }}<span class="mini-unit">ms</span></span>
        </div>
        <div class="mini-stat-divider"></div>
        <div class="mini-stat">
          <span class="mini-stat-label">Tokens</span>
          <span class="mini-stat-val accent">{{ (totalTokensInRange / 1000).toFixed(1) }}K</span>
        </div>
        <div class="mini-stat-divider"></div>
        <div class="mini-stat">
          <span class="mini-stat-label">活跃渠道</span>
          <span class="mini-stat-val">{{ activeProvidersInRange }}</span>
        </div>
      </div>
    </div>

    <!-- 工具栏：筛选 + 操作 -->
    <div class="toolbar">
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
      </NSpace>
      <NSpace :size="8">
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

    <!-- 主表格（含分页） -->
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
      <template v-else>
        <NDataTable
          :columns="columns"
          :data="filteredLogs"
          :loading="loading"
          :bordered="false"
          :single-line="false"
          :scroll-x="1000"
          striped
          size="small"
        />
        <!-- 分页整合在表格卡片内 -->
        <div class="table-pagination">
          <NText depth="3" style="font-size: 12px">{{ paginationText }}</NText>
          <NPagination
            v-model:page="page"
            :page-count="totalPages"
            :page-size="50"
            :show-size-picker="false"
            @update:page="logsStore.fetchPage"
          />
        </div>
      </template>
    </NCard>

    <!-- 日志详情弹窗 -->
    <!-- 日志详情：顶部先给结论（状态 / 耗时 / 模型 / 渠道），明细走双栏，低频字段折叠 -->
    <NModal
      v-model:show="showDetail"
      preset="card"
      title="日志详情"
      style="width: min(720px, calc(100vw - 32px))"
      :bordered="false"
      :segmented="{ footer: true }"
    >
      <div v-if="selectedLog" class="log-detail">
        <!-- 概览条：一眼确认"成功没 / 多快 / 走的谁" -->
        <div class="log-summary">
          <div class="ls-cell">
            <span class="ls-label">状态码</span>
            <span
              class="ls-value"
              :class="selectedLog.response_status && selectedLog.response_status >= 400 ? 'is-error' : 'is-ok'"
            >
              {{ selectedLog.response_status ?? "—" }}
            </span>
          </div>
          <div class="ls-cell">
            <span class="ls-label">总耗时</span>
            <span class="ls-value">{{ formatMs(selectedLog.total_duration_ms) }}</span>
          </div>
          <div class="ls-cell">
            <span class="ls-label">模型</span>
            <span class="ls-value is-mono">{{ selectedLog.model_name || selectedLog.model_id || "—" }}</span>
          </div>
          <div class="ls-cell">
            <span class="ls-label">渠道</span>
            <span class="ls-value">{{ selectedLog.provider_name || selectedLog.provider_id || "—" }}</span>
          </div>
        </div>

        <section class="m-section">
          <div class="m-sec-head">
            <span class="m-sec-title">请求</span>
          </div>
          <div class="detail-grid">
            <div class="dg-item">
              <span class="detail-label">Request ID</span>
              <span class="detail-value text-mono">{{ selectedLog.request_id }}</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">时间</span>
              <span class="detail-value">{{ selectedLog.timestamp }}</span>
            </div>
            <div class="dg-item dg-full">
              <span class="detail-label">请求</span>
              <span class="dg-value-row">
                <NTag size="small" type="info">{{ selectedLog.method }}</NTag>
                <span class="detail-value text-mono">{{ selectedLog.path }}</span>
              </span>
            </div>
          </div>
        </section>

        <section class="m-section">
          <div class="m-sec-head">
            <span class="m-sec-title">执行结果</span>
          </div>
          <div class="detail-grid">
            <div class="dg-item">
              <span class="detail-label">状态码</span>
              <NTag
                v-if="selectedLog.response_status"
                size="small"
                :type="selectedLog.response_status < 300 ? 'success' : 'error'"
              >
                {{ selectedLog.response_status }}
              </NTag>
              <span v-else class="detail-value">—</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">首次响应</span>
              <span class="detail-value">{{ formatMs(selectedLog.resp_ms) }}</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">总耗时</span>
              <span class="detail-value">{{ formatMs(selectedLog.total_duration_ms) }}</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">重试</span>
              <NTag v-if="selectedLog.retry_count > 0" size="small" type="warning">{{ selectedLog.retry_count }}</NTag>
              <span v-else class="detail-value">未重试</span>
            </div>
          </div>
        </section>
        <section class="m-section">
          <div class="m-sec-head">
            <span class="m-sec-title">路由信息</span>
          </div>
          <div class="detail-grid">
            <div class="dg-item" v-if="selectedLog.inbound_protocol || selectedLog.outbound_protocol">
              <span class="detail-label">协议转换</span>
              <span class="dg-value-row">
                <NTag size="small">{{ selectedLog.inbound_protocol || "-" }}</NTag>
                <span class="dg-arrow">→</span>
                <NTag size="small">{{ selectedLog.outbound_protocol || "-" }}</NTag>
              </span>
            </div>
            <div class="dg-item" v-if="selectedLog.provider_id">
              <span class="detail-label">渠道</span>
              <span class="detail-value">
                {{ selectedLog.provider_name || selectedLog.provider_id }}
                <span v-if="selectedLog.provider_name && selectedLog.provider_name !== selectedLog.provider_id" class="dg-sub">
                  {{ selectedLog.provider_id }}
                </span>
              </span>
            </div>
            <div class="dg-item" v-if="selectedLog.model_id || selectedLog.model_name">
              <span class="detail-label">模型</span>
              <span class="detail-value">
                {{ selectedLog.model_name || selectedLog.model_id }}
                <span v-if="selectedLog.model_name && selectedLog.model_id && selectedLog.model_name !== selectedLog.model_id" class="dg-sub">
                  {{ selectedLog.model_id }}
                </span>
              </span>
            </div>
          </div>
        </section>

        <section v-if="selectedLog.error_message || selectedLog.error_code" class="m-section">
          <div class="m-sec-head">
            <span class="m-sec-title">错误信息</span>
          </div>
          <div class="error-box">
            <div v-if="selectedLog.error_code" class="error-line">
              <span class="detail-label">错误码</span>
              <NTag size="small" type="error">{{ selectedLog.error_code }}</NTag>
            </div>
            <div v-if="selectedLog.error_message" class="error-line">
              <span class="detail-label">错误</span>
              <span class="detail-value">{{ selectedLog.error_message }}</span>
            </div>
          </div>
        </section>

        <!-- 低频排障字段收进折叠区，主流程不被数据量/缓存类信息淹没 -->
        <ModalAdvanced title="更多信息" hint="数据量 · 缓存 · 认证 Key">
          <div class="detail-grid">
            <div class="dg-item">
              <span class="detail-label">请求大小</span>
              <span class="detail-value">{{ (selectedLog.request_size_bytes || 0).toLocaleString() }} bytes</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">响应大小</span>
              <span class="detail-value">{{ (selectedLog.response_size_bytes || 0).toLocaleString() }} bytes</span>
            </div>
            <div class="dg-item">
              <span class="detail-label">Tokens</span>
              <span class="detail-value">
                输入 {{ selectedLog.tokens_input || 0 }} / 发送 {{ selectedLog.tokens_sent || 0 }} / 输出 {{ selectedLog.tokens_output || 0 }}
                <span
                  v-if="selectedLog.tokens_input != null && selectedLog.tokens_sent != null && selectedLog.tokens_input > selectedLog.tokens_sent"
                  class="dg-saved"
                >
                  优化 -{{ selectedLog.tokens_input - selectedLog.tokens_sent }}
                </span>
              </span>
            </div>
            <div class="dg-item">
              <span class="detail-label">流式</span>
              <NTag size="small" :type="selectedLog.stream_enabled ? 'info' : 'default'">
                {{ selectedLog.stream_enabled ? "是" : "否" }}
              </NTag>
            </div>
            <div class="dg-item">
              <span class="detail-label">缓存命中</span>
              <NTag size="small" :type="selectedLog.cache_hit ? 'success' : 'default'">
                {{ selectedLog.cache_hit ? "是" : "否" }}
              </NTag>
            </div>
            <div class="dg-item" v-if="selectedLog.auth_key_name">
              <span class="detail-label">认证 Key</span>
              <NTag size="small" type="success">{{ selectedLog.auth_key_name }}</NTag>
            </div>
            <div class="dg-item" v-if="selectedLog.channel_key_name">
              <span class="detail-label">渠道 Key</span>
              <NTag size="small" type="info">{{ selectedLog.channel_key_name }}</NTag>
            </div>
          </div>
        </ModalAdvanced>
      </div>
      <template #footer>
        <div class="detail-footer">
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
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 顶部标题 + mini-stat 行 */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
}

.page-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.page-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin: 0;
  letter-spacing: -0.01em;
}

.page-header-stats {
  display: flex;
  align-items: center;
  gap: 0;
  background: var(--card-bg, #fff);
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-sm, 6px);
  padding: 6px 12px;
  overflow: hidden;
  flex-shrink: 1;
}

.mini-stat {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 0 10px;
  min-width: 0;
}

.mini-stat-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--muted, #737373);
  font-weight: 600;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  white-space: nowrap;
}

.mini-stat-val {
  font-size: 16px;
  font-weight: 700;
  color: var(--fg, #0a0a0a);
  line-height: 1.2;
  letter-spacing: -0.02em;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  white-space: nowrap;
}

.mini-stat-val.accent {
  color: var(--accent, #2563eb);
}

.mini-stat-val.success {
  color: var(--success, #16a34a);
}

.mini-unit {
  font-size: 11px;
  font-weight: 500;
  opacity: 0.65;
  margin-left: 1px;
}

.mini-stat-divider {
  width: 1px;
  background: var(--border, #e5e5e5);
  margin: 4px 0;
  flex-shrink: 0;
}

/* 工具栏 */
.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

/* 表格卡片 */
.table-card {
  border-radius: var(--radius-sm, 6px);
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.table-pagination {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  border-top: 1px solid var(--border-soft, #ededed);
  flex-shrink: 0;
}

/* 空态 / 错误 */
.error-state, .empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 48px 16px;
  text-align: center;
}

.error-icon, .empty-icon {
  opacity: 0.6;
}

.error-title, .empty-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin: 0;
}

.error-desc, .empty-desc {
  font-size: 13px;
  color: var(--muted, #737373);
  margin: 0;
  max-width: 320px;
}

/* 详情弹窗 */
.log-detail {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

/* 概览条：4 格等宽，滚动之前先看到结论 */
.log-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 1px;
  background: var(--border-soft);
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.ls-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  padding: 10px 12px;
  background: var(--surface-alt);
}

.ls-label {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--muted);
}

.ls-value {
  font-size: 15px;
  font-weight: 600;
  color: var(--fg);
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ls-value.is-mono {
  font-family: var(--font-mono);
  font-size: 13px;
}

.ls-value.is-ok {
  color: var(--success);
}

.ls-value.is-error {
  color: var(--danger);
}

/* 明细：双栏 key-value，代替原来的单列长列表 */
.detail-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px 20px;
}

.dg-item {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
  padding: 3px 0;
}

.dg-full {
  grid-column: 1 / -1;
}

.dg-value-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex-wrap: wrap;
}

.dg-arrow {
  font-size: 12px;
  color: var(--muted);
}

.detail-label {
  flex: none;
  min-width: 72px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--muted);
}

.detail-value {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: var(--fg-2, #171717);
  word-break: break-all;
}

/* 名称之外的原始 ID，弱化但可查 */
.dg-sub {
  margin-left: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--muted);
}

.dg-saved {
  margin-left: 8px;
  color: var(--success);
}

/* 错误区：整块高亮，排障时第一眼命中 */
.error-box {
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--danger) 28%, var(--border));
  border-radius: var(--radius-sm);
  background: var(--danger-soft);
}

.error-line {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.error-line + .error-line {
  margin-top: 6px;
}

.error-line .detail-label,
.error-line .detail-value {
  color: var(--danger);
}

.detail-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

@media (max-width: 640px) {
  .log-summary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .detail-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .dg-full {
    grid-column: 1;
  }
}
</style>
