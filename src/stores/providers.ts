import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type Provider } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";

export const useProvidersStore = defineStore("providers", () => {
  const providers = ref<Provider[]>([]);
  const fetchOp = useAsyncOperation();
  const crudOp = useAsyncOperation();

  async function fetchAll() {
    const result = await fetchOp.run(() => api.listProviders(), "获取渠道失败");
    if (result) providers.value = result;
  }

  async function create(data: any) {
    const result = await crudOp.runOrThrow(() => api.createProvider(data), "创建失败");
    if (result) {
      providers.value.unshift(result);
    }
    return result;
  }

  async function update(id: string, data: Partial<Provider>) {
    const result = await crudOp.runOrThrow(() => api.updateProvider(id, data), "更新失败");
    if (result) {
      const idx = providers.value.findIndex((p) => p.id === id);
      if (idx >= 0) providers.value[idx] = result;
    }
    return result;
  }

  async function remove(id: string) {
    await crudOp.runOrThrow(() => api.deleteProvider(id), "删除失败");
    providers.value = providers.value.filter((p) => p.id !== id);
  }

  return {
    providers,
    loading: fetchOp.loading, error: fetchOp.error,
    crudLoading: crudOp.loading, crudError: crudOp.error,
    fetchAll, create, update, remove,
  };
});