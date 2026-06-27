import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type GatewayStatus } from "../api";

export const useGatewayStore = defineStore("gateway", () => {
  const status = ref<GatewayStatus | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchStatus() {
    loading.value = true;
    error.value = null;
    try {
      status.value = await api.gatewayStatus();
    } catch (e: any) {
      error.value = e.message || "获取状态失败";
    } finally {
      loading.value = false;
    }
  }

  /** 带自动重试的初始化（最多尝试 3 次，间隔 1s） */
  async function initStatus(maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
      await fetchStatus();
      if (status.value !== null) return;
      await new Promise((r) => setTimeout(r, 1000));
    }
  }

  async function start() {
    loading.value = true;
    error.value = null;
    try {
      await api.gatewayStart();
      await fetchStatus();
    } catch (e: any) {
      error.value = e.message || "启动失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function stop() {
    loading.value = true;
    error.value = null;
    try {
      await api.gatewayStop();
      await fetchStatus();
    } catch (e: any) {
      error.value = e.message || "停止失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function restart() {
    loading.value = true;
    error.value = null;
    try {
      await api.gatewayRestart();
      await fetchStatus();
    } catch (e: any) {
      error.value = e.message || "重启失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function updateSettings(data: any) {
    loading.value = true;
    error.value = null;
    try {
      await api.updateGatewaySettings(data);
      await fetchStatus();
    } catch (e: any) {
      error.value = e.message || "更新设置失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { status, loading, error, fetchStatus, initStatus, start, stop, restart, updateSettings };
});
