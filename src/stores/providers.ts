import { defineStore } from "pinia";
import { api, type Provider } from "../api";
import { useCrudStore } from "../composables/useCrudStore";

export const useProvidersStore = defineStore("providers", () => {
  const crud = useCrudStore<Provider>(
    "providers",
    {
      list: () => api.listProviders(),
      create: (data) =>
        api.createProvider(data as Partial<Provider> & { api_key: string }),
      update: (id, data) => api.updateProvider(id, data),
      delete: (id) => api.deleteProvider(id),
    },
  );

  const { items, loading, error, fetchAll, create, update, remove, refresh } = crud;
  const providers = items;

  return {
    providers,
    loading,
    error,
    crudLoading: crud.crudLoading,
    crudError: crud.crudError,
    fetchAll,
    create,
    update,
    remove,
    refresh,
  };
});
