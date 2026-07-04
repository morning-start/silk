import { defineStore } from "pinia";
import { ref } from "vue";
import { api, type GatewaySettings, type GatewayStatus } from "../api";
import { useAsyncOperation } from "../composables/useAsyncOperation";

/** 延迟辅助函数 — 使用 Promise.withResolvers 保持控制流线性 */
function delay(ms: number): Promise<void> {
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(resolve, ms);
  return promise;
}

export const useGatewayStore = defineStore("gateway", () => {
  const status = ref<GatewayStatus | null>(null);
  const op = useAsyncOperation();

  async function fetchStatus() {
    status.value = await api.gatewayStatus();
  }

  /** 带自动重试的初始化（最多尝试 3 次，间隔 1s） */
  async function initStatus(maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
      try {
        await fetchStatus();
      } catch {
        // ignore, will retry
      }
      if (status.value !== null) return;
      await delay(1000);
    }
  }

  async function start() {
    await op.runOrThrow(async () => {
      await api.gatewayStart();
      await fetchStatus();
    }, "启动失败");
  }

  async function stop() {
    await op.runOrThrow(async () => {
      await api.gatewayStop();
      await fetchStatus();
    }, "停止失败");
  }

  async function restart() {
    await op.runOrThrow(async () => {
      await api.gatewayRestart();
      await fetchStatus();
    }, "重启失败");
  }

  async function updateSettings(data: Partial<GatewaySettings>) {
    await op.runOrThrow(async () => {
      await api.updateGatewaySettings(data);
      await fetchStatus();
    }, "更新设置失败");
  }

  return {
    status,
    loading: op.loading,
    error: op.error,
    fetchStatus,
    initStatus,
    start,
    stop,
    restart,
    updateSettings,
  };
});
