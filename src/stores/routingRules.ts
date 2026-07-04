import { defineStore } from "pinia";
import { api, type RoutingRule } from "../api";
import { useCrudStore } from "../composables/useCrudStore";

export const useRoutingRulesStore = defineStore("routingRules", () => {
  const crud = useCrudStore<RoutingRule>("routingRules", {
    list: () => api.listRoutingRules(),
    create: (data) => api.createRoutingRule(data),
    update: (id, data) => api.updateRoutingRule(id, data),
    delete: (id) => api.deleteRoutingRule(id),
  });

  const { items, loading, error, fetchAll, create, update, remove, refresh } = crud;
  const rules = items;

  return {
    rules,
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
