import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { api, type RequestLog } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";
import { useSwrCache } from "../composables/useSwrCache";

const STORAGE_KEY = "silk:logs:offset";

export const useLogsStore = defineStore("logs", () => {
  // SWR 缓存：日志数据刷新不频繁，60s TTL 避免视图切换重复拉取
  const cache = useSwrCache<RequestLog[]>(60_000);
  const total = ref(0);
  const limit = ref(50);
  const offset = ref(loadSavedOffset());
  const fetchOp = useAsyncOperation();
  const actionOp = useAsyncOperation();

  const logs = computed(() => cache.data.value ?? []);
  const page = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.ceil(total.value / limit.value));

  async function fetchPage(pageNum: number) {
    const newOffset = (pageNum - 1) * limit.value;
    const result = await fetchOp.run(
      () => api.listLogs(limit.value, newOffset),
      "获取日志失败"
    );
    if (result) {
      cache.data.value = result.logs;
      total.value = result.total;
      offset.value = newOffset;
      saveOffset(newOffset);
    }
  }

  async function fetchAll() {
    // 使用 SWR 检查缓存，首页数据存在且未过期时跳过请求
    const existing = await cache.fetchIfNeeded(
      () => api.listLogs(limit.value, 0).then((r) => { total.value = r.total; return r.logs; })
    );
    if (existing && existing.length > 0) {
      cache.data.value = existing;
      offset.value = 0;
      saveOffset(0);
    } else {
      await fetchPage(1);
    }
  }

  async function goPage(delta: number) {
    const targetPage = page.value + delta;
    if (targetPage >= 1 && targetPage <= totalPages.value) {
      await fetchPage(targetPage);
    }
  }

  async function nextPage() {
    await goPage(1);
  }

  async function prevPage() {
    await goPage(-1);
  }

  async function cleanup(beforeDays: number) {
    await actionOp.runOrThrow(() => api.cleanupLogs(beforeDays), "清理失败");
    cache.clear();
    await fetchAll();
  }

  async function clearAll() {
    await actionOp.runOrThrow(() => api.clearAllLogs(), "清空失败");
    cache.clear();
    total.value = 0;
  }

  return {
    logs, total, limit, offset, page, totalPages,
    loading: fetchOp.loading || cache.loading.value, error: fetchOp.error || cache.error.value,
    actionLoading: actionOp.loading, actionError: actionOp.error,
    fetchAll, fetchPage, nextPage, prevPage, cleanup, clearAll,
  };
});

function loadSavedOffset(): number {
  try {
    const saved = sessionStorage.getItem(STORAGE_KEY);
    return saved ? Number(saved) || 0 : 0;
  } catch {
    return 0;
  }
}

function saveOffset(offset: number) {
  try {
    sessionStorage.setItem(STORAGE_KEY, String(offset));
  } catch { /* ignore */ }
}
