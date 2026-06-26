import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type Provider } from "../api";

export const useProvidersStore = defineStore("providers", () => {
  const providers = ref<Provider[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchAll() {
    loading.value = true;
    error.value = null;
    try {
      providers.value = await api.listProviders();
    } catch (e: any) {
      error.value = e.message || "获取 Provider 失败";
    } finally {
      loading.value = false;
    }
  }

  async function create(data: Partial<Provider> & { api_key: string }) {
    loading.value = true;
    error.value = null;
    try {
      const created = await api.createProvider(data);
      providers.value.unshift(created);
      return created;
    } catch (e: any) {
      error.value = e.message || "创建失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function update(id: string, data: Partial<Provider>) {
    loading.value = true;
    error.value = null;
    try {
      const updated = await api.updateProvider(id, data);
      const idx = providers.value.findIndex((p) => p.id === id);
      if (idx >= 0) providers.value[idx] = updated;
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
      await api.deleteProvider(id);
      providers.value = providers.value.filter((p) => p.id !== id);
    } catch (e: any) {
      error.value = e.message || "删除失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { providers, loading, error, fetchAll, create, update, remove };
});
