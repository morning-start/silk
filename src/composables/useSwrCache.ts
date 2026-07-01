import { ref, type Ref } from "vue";

/**
 * 简单的 SWR（stale-while-revalidate）缓存工具
 *
 * 避免每次视图切换都重新拉取数据。
 * - `data`：缓存的数据（可能为陈旧数据）
 * - `loading`：是否正在重新验证
 * - `error`：最近一次错误
 * - `fetchIfNeeded()`：如果数据为空或已过期，则重新拉取
 * - `refresh()`：强制刷新
 * - `clear()`：清除缓存
 */
export function useSwrCache<T>(ttlMs = 30_000) {
  const data = ref<T | null>(null) as Ref<T | null>;
  const loading = ref(false);
  const error = ref<string | null>(null);
  let lastFetchTime = 0;
  let fetchPromise: Promise<T | undefined> | null = null;

  async function fetchIfNeeded(fn: () => Promise<T>): Promise<T | undefined> {
    const now = Date.now();
    // 如果已有数据且未过期，直接返回
    if (data.value !== null && now - lastFetchTime < ttlMs) {
      return data.value;
    }
    // 避免并发重复请求
    if (fetchPromise) {
      return fetchPromise;
    }
    fetchPromise = doFetch(fn);
    try {
      return await fetchPromise;
    } finally {
      fetchPromise = null;
    }
  }

  async function refresh(fn: () => Promise<T>): Promise<T | undefined> {
    lastFetchTime = 0; // 强制过期
    return fetchIfNeeded(fn);
  }

  async function doFetch(fn: () => Promise<T>): Promise<T | undefined> {
    loading.value = true;
    error.value = null;
    try {
      const result = await fn();
      data.value = result;
      lastFetchTime = Date.now();
      return result;
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : "加载失败";
      // SWR: 即使出错也保留旧数据
      return data.value ?? undefined;
    } finally {
      loading.value = false;
    }
  }

  function clear() {
    data.value = null;
    lastFetchTime = 0;
    error.value = null;
  }

  return { data, loading, error, fetchIfNeeded, refresh, clear };
}
