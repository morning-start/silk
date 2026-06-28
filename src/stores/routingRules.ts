import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type RoutingRule } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";

export const useRoutingRulesStore = defineStore("routingRules", () => {
  const rules = ref<RoutingRule[]>([]);
  const fetchOp = useAsyncOperation();
  const crudOp = useAsyncOperation();

  async function fetchAll() {
    const result = await fetchOp.run(() => api.listRoutingRules(), "获取路由规则失败");
    if (result) rules.value = result;
  }

  async function create(data: Partial<RoutingRule>) {
    const result = await crudOp.runOrThrow(() => api.createRoutingRule(data), "创建失败");
    if (result) {
      rules.value.unshift(result);
    }
    return result;
  }

  async function update(id: string, data: Partial<RoutingRule>) {
    const result = await crudOp.runOrThrow(() => api.updateRoutingRule(id, data), "更新失败");
    if (result) {
      const idx = rules.value.findIndex((r) => r.id === id);
      if (idx >= 0) rules.value[idx] = result;
    }
    return result;
  }

  async function remove(id: string) {
    await crudOp.runOrThrow(() => api.deleteRoutingRule(id), "删除失败");
    rules.value = rules.value.filter((r) => r.id !== id);
  }

  return { rules, loading: fetchOp.loading, error: fetchOp.error, fetchAll, create, update, remove };
});