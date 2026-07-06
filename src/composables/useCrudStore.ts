import { computed } from "vue";
import { useAsyncOperation } from "./useAsyncOperation";
import { useSwrCache } from "./useSwrCache";
import { notifyDataChanged, type DataChangeEvent } from "./useCrossStoreNotify";

type CrudApi<T> = {
  list: () => Promise<T[]>;
  create: (data: Partial<T>) => Promise<T>;
  update: (id: string, data: Partial<T>) => Promise<T>;
  delete: (id: string) => Promise<unknown>;
};

/** 通用 CRUD store — 消除跨文件复制 */
export function useCrudStore<T extends { id: string }>(
  id: DataChangeEvent,
  api: CrudApi<T>,
  ttl = 30_000,
) {
  const cache = useSwrCache<T[]>(ttl);
  const crudOp = useAsyncOperation();

  const items = computed(() => cache.data.value ?? []);
  const loading = computed(() => cache.loading.value);
  const error = computed(() => cache.error.value);

  async function fetchAll() {
    return cache.fetchIfNeeded(() => api.list());
  }

  async function create(data: Partial<T>) {
    const result = await crudOp.runOrThrow(() => api.create(data), "创建失败");
    if (result) {
      const list = cache.data.value ?? [];
      list.unshift(result);
      cache.data.value = list;
      notifyDataChanged(id);
    }
    return result;
  }

  async function update(uid: string, data: Partial<T>) {
    const result = await crudOp.runOrThrow(() => api.update(uid, data), "更新失败");
    if (result) {
      const list = cache.data.value;
      if (list) {
        const idx = list.findIndex((p) => p.id === uid);
        if (idx >= 0) list[idx] = result;
      }
      notifyDataChanged(id);
    }
    return result;
  }

  async function remove(uid: string) {
    await crudOp.runOrThrow(() => api.delete(uid), "删除失败");
    const list = cache.data.value;
    if (list) {
      cache.data.value = list.filter((p) => p.id !== uid);
      notifyDataChanged(id);
    }
  }

  /** 强制刷新（忽略缓存） */
  async function refresh() {
    return cache.refresh(() => api.list());
  }

  return {
    items,
    loading,
    error,
    crudLoading: crudOp.loading,
    crudError: crudOp.error,
    fetchAll,
    create,
    update,
    remove,
    refresh,
  };
}
