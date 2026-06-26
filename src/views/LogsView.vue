<script setup lang="ts">
import { ref, onMounted, h, computed } from "vue";
import {
  NButton,
  NDataTable,
  NTag,
  NText,
  NPagination,
  NSpace,
  NPopconfirm,
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import { useLogsStore } from "../stores/logs";
import { storeToRefs } from "pinia";
import type { RequestLog } from "../api";

const logsStore = useLogsStore();
const { logs, total, page, totalPages, loading } = storeToRefs(logsStore);
const message = useMessage();

const selectedLog = ref<RequestLog | null>(null);
const showDetail = ref(false);

const columns: DataTableColumns<RequestLog> = [
  { title: "时间", key: "timestamp", width: 160 },
  { title: "方法", key: "method", width: 70 },
  { title: "路径", key: "path", width: 200 },
  {
    title: "状态",
    key: "response_status",
    width: 80,
    render(row) {
      if (!row.response_status) return h(NText, { type: "default" }, { default: () => "-" });
      const type = row.response_status < 300 ? "success" : row.response_status < 400 ? "warning" : "error";
      return h(NText, { type }, { default: () => row.response_status });
    },
  },
  { title: "耗时(ms)", key: "duration_ms", width: 90 },
  { title: "模型", key: "model_used", width: 120 },
  {
    title: "流式",
    key: "stream_enabled",
    width: 60,
    render(row) {
      return h(NTag, { size: "small", type: row.stream_enabled ? "info" : "default" }, { default: () => row.stream_enabled ? "是" : "否" });
    },
  },
  {
    title: "操作",
    key: "actions",
    width: 80,
    render(row) {
      return h(NButton, { size: "small", onClick: () => handleViewDetail(row) }, { default: () => "详情" });
    },
  },
];

function handleViewDetail(row: RequestLog) {
  selectedLog.value = row;
  showDetail.value = true;
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

const paginationText = computed(() => {
  const start = (page.value - 1) * 50 + 1;
  const end = Math.min(page.value * 50, total.value);
  return `${start}-${end} / ${total.value}`;
});

onMounted(() => {
  logsStore.fetchAll();
});
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px">
      <n-text style="font-size: 18px; font-weight: 600">请求日志</n-text>
      <n-space>
        <n-button @click="handleCleanup">清理 7 天前</n-button>
        <n-popconfirm @positive-click="handleClearAll">
          <template #trigger>
            <n-button type="error">清空全部</n-button>
          </template>
          确定要清空所有日志吗？此操作不可恢复。
        </n-popconfirm>
      </n-space>
    </div>

    <n-data-table :columns="columns" :data="logs" :loading="loading" :bordered="false" striped />

    <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 16px">
      <n-text style="color: #999">{{ paginationText }}</n-text>
      <n-pagination
        v-model:page="page"
        :page-count="totalPages"
        :page-size="50"
        @update:page="logsStore.fetchPage"
      />
    </div>

    <!-- 日志详情 -->
    <n-modal v-model:show="showDetail" preset="dialog" title="日志详情" style="width: 700px">
      <div v-if="selectedLog">
        <div style="margin-bottom: 12px">
          <n-text strong>Request ID：</n-text>
          <n-text>{{ selectedLog.request_id }}</n-text>
        </div>
        <div style="margin-bottom: 12px">
          <n-text strong>时间：</n-text>
          <n-text>{{ selectedLog.timestamp }}</n-text>
        </div>
        <div style="margin-bottom: 12px">
          <n-text strong>请求：</n-text>
          <n-text>{{ selectedLog.method }} {{ selectedLog.path }}</n-text>
        </div>
        <div v-if="selectedLog.error_message" style="margin-bottom: 12px">
          <n-text strong type="error">错误：</n-text>
          <n-text type="error">{{ selectedLog.error_message }}</n-text>
        </div>
        <div style="margin-bottom: 12px">
          <n-text strong>请求大小：</n-text>
          <n-text>{{ selectedLog.request_size_bytes || 0 }} bytes</n-text>
        </div>
        <div style="margin-bottom: 12px">
          <n-text strong>响应大小：</n-text>
          <n-text>{{ selectedLog.response_size_bytes || 0 }} bytes</n-text>
        </div>
      </div>
      <template #action>
        <n-button @click="showDetail = false">关闭</n-button>
      </template>
    </n-modal>
  </div>
</template>
