import { defineStore } from "pinia";
import { computed } from "vue";
import { api, type Provider } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";
import { useSwrCache } from "../composables/useSwrCache";
import { notifyDataChanged } from "../composables/useCrossStoreNotify";

export const useProvidersStore = defineStore("providers", () => {
  // SWR 缓存：30 秒 TTL，视图切换时避免重复 fetch
  const cache = useSwrCache<Provider[]>(30_000);
  const crudOp = useAsyncOperation();

  const providers = computed(() => cache.data.value ?? []);
  const loading = computed(() => cache.loading.value);
  const error = computed(() => cache.error.value);

  async function fetchAll() {
    return cache.fetchIfNeeded(() => api.listProviders());
  }

  async function create(data: any) {
    const result = await crudOp.runOrThrow(() => api.createProvider(data), "创建失败");
    if (result) {
      // 直接插入本地缓存，避免重新拉取全量数据
      const list = cache.data.value ?? [];
      list.unshift(result);
      cache.data.value = list;
      notifyDataChanged("providers");
    }
    return result;
  }

  async function update(id: string, data: Partial<Provider>) {
    const result = await crudOp.runOrThrow(() => api.updateProvider(id, data), "更新失败");
    if (result) {
      const list = cache.data.value;
      if (list) {
        const idx = list.findIndex((p) => p.id === id);
        if (idx >= 0) list[idx] = result;
      }
      notifyDataChanged("providers");
    }
    return result;
  }

  async function remove(id: string) {
    await crudOp.runOrThrow(() => api.deleteProvider(id), "删除失败");
    const list = cache.data.value;
    if (list) {
      cache.data.value = list.filter((p) => p.id !== id);
      notifyDataChanged("providers");
    }
  }

  /** 强制刷新（忽略缓存） */
  async function refresh() {
    return cache.refresh(() => api.listProviders());
  }

  return {
    providers,
    loading, error,
    crudLoading: crudOp.loading, crudError: crudOp.error,
    fetchAll, create, update, remove, refresh,
  };
});
