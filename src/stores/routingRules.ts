import { defineStore } from "pinia";
import { computed } from "vue";
import { api, type RoutingRule } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";
import { useSwrCache } from "../composables/useSwrCache";
import { notifyDataChanged } from "../composables/useCrossStoreNotify";

export const useRoutingRulesStore = defineStore("routingRules", () => {
  // SWR 缓存：30 秒 TTL
  const cache = useSwrCache<RoutingRule[]>(30_000);
  const crudOp = useAsyncOperation();

  const rules = computed(() => cache.data.value ?? []);
  const loading = computed(() => cache.loading.value);
  const error = computed(() => cache.error.value);

  async function fetchAll() {
    return cache.fetchIfNeeded(() => api.listRoutingRules());
  }

  async function create(data: Partial<RoutingRule>) {
    const result = await crudOp.runOrThrow(() => api.createRoutingRule(data), "创建失败");
    if (result) {
      const list = cache.data.value ?? [];
      list.unshift(result);
      cache.data.value = list;
      notifyDataChanged("routingRules");
    }
    return result;
  }

  async function update(id: string, data: Partial<RoutingRule>) {
    const result = await crudOp.runOrThrow(() => api.updateRoutingRule(id, data), "更新失败");
    if (result) {
      const list = cache.data.value;
      if (list) {
        const idx = list.findIndex((r) => r.id === id);
        if (idx >= 0) list[idx] = result;
      }
      notifyDataChanged("routingRules");
    }
    return result;
  }

  async function remove(id: string) {
    await crudOp.runOrThrow(() => api.deleteRoutingRule(id), "删除失败");
    const list = cache.data.value;
    if (list) {
      cache.data.value = list.filter((r) => r.id !== id);
      notifyDataChanged("routingRules");
    }
  }

  /** 强制刷新（忽略缓存） */
  async function refresh() {
    return cache.refresh(() => api.listRoutingRules());
  }

  return { rules, loading, error, fetchAll, create, update, remove, refresh };
});
