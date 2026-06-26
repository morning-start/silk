import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type RoutingRule } from "../api";

export const useRoutingRulesStore = defineStore("routingRules", () => {
  const rules = ref<RoutingRule[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchAll() {
    loading.value = true;
    error.value = null;
    try {
      rules.value = await api.listRoutingRules();
    } catch (e: any) {
      error.value = e.message || "获取路由规则失败";
    } finally {
      loading.value = false;
    }
  }

  async function create(data: Partial<RoutingRule>) {
    loading.value = true;
    error.value = null;
    try {
      const created = await api.createRoutingRule(data);
      rules.value.unshift(created);
      return created;
    } catch (e: any) {
      error.value = e.message || "创建失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function update(id: string, data: Partial<RoutingRule>) {
    loading.value = true;
    error.value = null;
    try {
      const updated = await api.updateRoutingRule(id, data);
      const idx = rules.value.findIndex((r) => r.id === id);
      if (idx >= 0) rules.value[idx] = updated;
      return updated;
    } catch (e: any) {
      error.value = e.message || "更新失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function remove(id: string) {
    loading.value = true;
    error.value = null;
    try {
      await api.deleteRoutingRule(id);
      rules.value = rules.value.filter((r) => r.id !== id);
    } catch (e: any) {
      error.value = e.message || "删除失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { rules, loading, error, fetchAll, create, update, remove };
});
