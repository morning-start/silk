import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type ProviderGroup, type GroupWithMembers } from "../api";

export const useGroupsStore = defineStore("groups", () => {
  const groups = ref<ProviderGroup[]>([]);
  const currentGroup = ref<GroupWithMembers | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchAll() {
    loading.value = true;
    error.value = null;
    try {
      groups.value = await api.listGroups();
    } catch (e: any) {
      error.value = e.message || "获取分组失败";
    } finally {
      loading.value = false;
    }
  }

  async function fetchById(id: string) {
    loading.value = true;
    error.value = null;
    try {
      currentGroup.value = await api.getGroup(id);
    } catch (e: any) {
      error.value = e.message || "获取分组失败";
    } finally {
      loading.value = false;
    }
  }

  async function create(data: { name: string; model_name: string; strategy?: string }) {
    loading.value = true;
    error.value = null;
    try {
      const created = await api.createGroup(data);
      groups.value.unshift(created);
      return created;
    } catch (e: any) {
      error.value = e.message || "创建失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function update(id: string, data: Partial<ProviderGroup>) {
    loading.value = true;
    error.value = null;
    try {
      const updated = await api.updateGroup(id, data);
      const idx = groups.value.findIndex((g) => g.id === id);
      if (idx >= 0) groups.value[idx] = updated;
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
      await api.deleteGroup(id);
      groups.value = groups.value.filter((g) => g.id !== id);
    } catch (e: any) {
      error.value = e.message || "删除失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function addMember(groupId: string, providerId: string, weight?: number) {
    loading.value = true;
    error.value = null;
    try {
      await api.addGroupMember(groupId, { provider_id: providerId, weight });
      await fetchById(groupId);
    } catch (e: any) {
      error.value = e.message || "添加成员失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function removeMember(id: string, groupId: string) {
    loading.value = true;
    error.value = null;
    try {
      await api.removeGroupMember(id);
      await fetchById(groupId);
    } catch (e: any) {
      error.value = e.message || "移除成员失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { groups, currentGroup, loading, error, fetchAll, fetchById, create, update, remove, addMember, removeMember };
});
