import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api, type ProviderGroup, type GroupWithMembers } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";
import { useSwrCache } from "../composables/useSwrCache";
import { notifyDataChanged } from "../composables/useCrossStoreNotify";

export const useGroupsStore = defineStore("groups", () => {
  // SWR 缓存：30 秒 TTL
  const cache = useSwrCache<ProviderGroup[]>(30_000);
  const crudOp = useAsyncOperation();

  const groups = computed(() => cache.data.value ?? []);
  const currentGroup = ref<GroupWithMembers | null>(null);
  const loading = computed(() => cache.loading.value);
  const error = computed(() => cache.error.value);

  async function fetchAll() {
    return cache.fetchIfNeeded(() => api.listGroups());
  }

  async function fetchById(id: string) {
    const result = await crudOp.run(() => api.getGroup(id), "获取分组失败");
    if (result) currentGroup.value = result;
  }

  async function create(data: { name: string; model_name: string; strategy?: string }) {
    const result = await crudOp.runOrThrow(() => api.createGroup(data), "创建失败");
    if (result) {
      const list = cache.data.value ?? [];
      list.unshift(result);
      cache.data.value = list;
      notifyDataChanged("groups");
    }
    return result;
  }

  async function update(id: string, data: Partial<ProviderGroup>) {
    const result = await crudOp.runOrThrow(() => api.updateGroup(id, data), "更新失败");
    if (result) {
      const list = cache.data.value;
      if (list) {
        const idx = list.findIndex((g) => g.id === id);
        if (idx >= 0) list[idx] = result;
      }
      notifyDataChanged("groups");
    }
    return result;
  }

  async function remove(id: string) {
    await crudOp.runOrThrow(() => api.deleteGroup(id), "删除失败");
    const list = cache.data.value;
    if (list) {
      cache.data.value = list.filter((g) => g.id !== id);
      notifyDataChanged("groups");
    }
  }

  async function addMember(groupId: string, providerId: string, weight?: number) {
    await crudOp.runOrThrow(
      () => api.addGroupMember(groupId, { provider_id: providerId, weight }),
      "添加成员失败"
    );
    await fetchById(groupId);
  }

  async function removeMember(id: string, groupId: string) {
    await crudOp.runOrThrow(() => api.removeGroupMember(id), "移除成员失败");
    await fetchById(groupId);
  }

  /** 强制刷新（忽略缓存） */
  async function refresh() {
    return cache.refresh(() => api.listGroups());
  }

  return {
    groups,
    currentGroup,
    loading,
    error,
    crudLoading: crudOp.loading,
    crudError: crudOp.error,
    fetchAll,
    fetchById,
    create,
    update,
    remove,
    addMember,
    removeMember,
    refresh,
  };
});
