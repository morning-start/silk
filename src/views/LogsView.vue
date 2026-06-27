<script setup lang="ts">
import { ref, onMounted, h, computed } from "vue";
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
import type { RequestLog } from "../api";
import { api } from "../api";

const logsStore = useLogsStore();
const { logs, total, page, totalPages, loading, error } = storeToRefs(logsStore);
const message = useMessage();

const selectedLog = ref<RequestLog | null>(null);
const showDetail = ref(false);

const statusFilter = ref("all");

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
    title: "耗时",
    key: "duration_ms",
    width: 80,
    render(row) {
      if (row.duration_ms == null) return "-";
      return h("span", { class: "num" }, `${row.duration_ms}ms`);
    },
  },
  {
    title: "Token",
    key: "tokens_input",
    width: 70,
    render(row) {
      const total = (row.tokens_input || 0) + (row.tokens_output || 0);
      return h("span", { class: "num" }, total ? `${total}` : "-");
    },
  },
  { title: "模型", key: "model_used", width: 110 },
  { title: "服务商", key: "provider_id", width: 100 },
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
  return logs.value;
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
    const result = await api.exportLogsCsv({ limit: 10000 });
    message.success(`已导出 ${result.exported_count} 条日志到 ${result.file_path}`);
  } catch {
    message.error("导出失败");
  }
}

const paginationText = computed(() => {
  const start = (page.value - 1) * 50 + 1;
  const end = Math.min(page.value * 50, total.value);
  return `显示 ${start}-${end} / ${total.value.toLocaleString()} 条`;
});

onMounted(() => {
  logsStore.fetchAll();
});
</script>

<template>
  <div class="logs-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">请求日志</h2>
        <NTag size="small" type="info">共 {{ total.toLocaleString() }} 条</NTag>
      </div>
      <div class="toolbar-right">
        <NSpace :size="8">
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
        <div class="detail-row">
          <span class="detail-label">耗时：</span>
          <span class="detail-value">{{ selectedLog.duration_ms }}ms</span>
        </div>
        <div class="detail-row" v-if="selectedLog.provider_id">
          <span class="detail-label">服务商：</span>
          <span class="detail-value">{{ selectedLog.provider_id }}</span>
        </div>
        <div class="detail-row" v-if="selectedLog.model_used">
          <span class="detail-label">模型：</span>
          <span class="detail-value">{{ selectedLog.model_used }}</span>
        </div>
        <div class="detail-row" v-if="selectedLog.error_message">
          <span class="detail-label" style="color: #ef4444">错误：</span>
          <span class="detail-value" style="color: #ef4444">{{ selectedLog.error_message }}</span>
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
          <span class="detail-label">Token (输入/输出)：</span>
          <span class="detail-value">{{ selectedLog.tokens_input || 0 }} / {{ selectedLog.tokens_output || 0 }}</span>
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
  max-width: 1200px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar-right {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.table-card {
  border-radius: 12px;
}

.pagination-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 16px;
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
