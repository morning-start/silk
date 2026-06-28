import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { api, type RequestLog } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";

export const useLogsStore = defineStore("logs", () => {
  const logs = ref<RequestLog[]>([]);
  const total = ref(0);
  const limit = ref(50);
  const offset = ref(0);
  const fetchOp = useAsyncOperation();
  const actionOp = useAsyncOperation();

  const page = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.ceil(total.value / limit.value));

  async function fetchPage(pageNum: number) {
    const newOffset = (pageNum - 1) * limit.value;
    const result = await fetchOp.run(
      () => api.listLogs(limit.value, newOffset),
      "获取日志失败"
    );
    if (result) {
      logs.value = result.logs;
      total.value = result.total;
      offset.value = newOffset;
    }
  }

  async function fetchAll() {
    await fetchPage(1);
  }

  async function nextPage() {
    if (page.value < totalPages.value) {
      await fetchPage(page.value + 1);
    }
  }

  async function prevPage() {
    if (page.value > 1) {
      await fetchPage(page.value - 1);
    }
  }

  async function cleanup(beforeDays: number) {
    await actionOp.runOrThrow(() => api.cleanupLogs(beforeDays), "清理失败");
    await fetchAll();
  }

  async function clearAll() {
    await actionOp.runOrThrow(() => api.clearAllLogs(), "清空失败");
    logs.value = [];
    total.value = 0;
  }

  return {
    logs, total, limit, offset, page, totalPages,
    loading: fetchOp.loading, error: fetchOp.error,
    actionLoading: actionOp.loading, actionError: actionOp.error,
    fetchAll, fetchPage, nextPage, prevPage, cleanup, clearAll,
  };
});