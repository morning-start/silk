import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { api, type RequestLog } from "../api";

export const useLogsStore = defineStore("logs", () => {
  const logs = ref<RequestLog[]>([]);
  const total = ref(0);
  const limit = ref(50);
  const offset = ref(0);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const page = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.ceil(total.value / limit.value));

  async function fetchPage(pageNum: number) {
    loading.value = true;
    error.value = null;
    offset.value = (pageNum - 1) * limit.value;
    try {
      const result = await api.listLogs(limit.value, offset.value);
      logs.value = result.logs;
      total.value = result.total;
    } catch (e: any) {
      error.value = e.message || "获取日志失败";
    } finally {
      loading.value = false;
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
    loading.value = true;
    error.value = null;
    try {
      await api.cleanupLogs(beforeDays);
      await fetchAll();
    } catch (e: any) {
      error.value = e.message || "清理失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function clearAll() {
    loading.value = true;
    error.value = null;
    try {
      await api.clearAllLogs();
      logs.value = [];
      total.value = 0;
    } catch (e: any) {
      error.value = e.message || "清空失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { logs, total, limit, offset, page, totalPages, loading, error, fetchAll, fetchPage, nextPage, prevPage, cleanup, clearAll };
});
