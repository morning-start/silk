import { defineStore } from "pinia";
import { providersApi, type Provider } from "../api";
import { useCrudStore } from "../composables/useCrudStore";

export const useProvidersStore = defineStore("providers", () => {
  const {
    items,
    loading,
    error,
    crudLoading,
    crudError,
    fetchAll,
    create,
    update,
    remove,
    refresh,
  } = useCrudStore<Provider>("providers", {
    list: () => providersApi.list(),
    create: (data) =>
      providersApi.create(data as Partial<Provider> & { api_key: string }),
    update: (id, data) => providersApi.update(id, data),
    delete: (id) => providersApi.remove(id),
  });

  return {
    providers: items,
    loading,
    error,
    crudLoading,
    crudError,
    fetchAll,
    create,
    update,
    remove,
    refresh,
  };
});
