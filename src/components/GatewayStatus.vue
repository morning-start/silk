<template>
  <div class="gateway-status" :class="{ 'running': status?.running }">
    <div class="status-glow"></div>
    <div class="status-indicator">
      <span class="status-dot" :class="{ 'active': status?.running }">
        <span class="status-dot-ring"></span>
      </span>
      <span class="status-text">{{ statusText }}</span>
    </div>
    <div class="status-address" v-if="status?.running">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:12px;height:12px;flex-shrink:0">
        <path d="M5 12.55a11 11 0 0 1 14.08 0"/>
        <path d="M1.42 9a16 16 0 0 1 21.16 0"/>
        <path d="M8.53 16.11a6 6 0 0 1 6.95 0"/>
        <line x1="12" y1="20" x2="12.01" y2="20"/>
      </svg>
      {{ status.address }}
    </div>
    <button class="btn-toggle" @click="toggleGateway" :disabled="loading">
      <template v-if="loading">
        <span class="btn-loading"></span>
        处理中…
      </template>
      <template v-else-if="status?.running">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:12px;height:12px">
          <rect x="6" y="6" width="12" height="12" rx="1.5"/>
        </svg>
        停止
      </template>
      <template v-else>
        <svg viewBox="0 0 24 24" fill="currentColor" style="width:12px;height:12px">
          <polygon points="5,3 19,12 5,21"/>
        </svg>
        启动
      </template>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { gatewayNotifications } from '../utils/notification';

interface GatewayStatus {
  running: boolean;
  address: string;
  settings: {
    bind_host: string;
    bind_port: number;
  };
}

const status = ref<GatewayStatus | null>(null);
const loading = ref(false);

const statusText = computed(() => {
  if (!status.value) return '未知';
  return status.value.running ? '运行中' : '已停止';
});

onMounted(async () => {
  await fetchStatus();
});

async function fetchStatus() {
  try {
    status.value = await invoke<GatewayStatus>('gateway_status');
  } catch (error) {
    console.error('获取网关状态失败:', error);
  }
}

async function toggleGateway() {
  loading.value = true;
  try {
    if (status.value?.running) {
      await invoke('gateway_stop');
      gatewayNotifications.stopped();
    } else {
      const result = await invoke<{ success: boolean; address: string }>('gateway_start');
      gatewayNotifications.started(result.address);
    }
    await fetchStatus();
  } catch (error) {
    const errorMsg = String(error);
    if (status.value?.running) {
      gatewayNotifications.stopFailed(errorMsg);
    } else {
      gatewayNotifications.startFailed(errorMsg);
    }
    console.error('操作失败:', error);
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped>
.gateway-status {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius-lg, 10px);
  box-shadow: var(--shadow-card, 0 1px 0 0 rgba(0,0,0,0.02));
}

.gateway-status.running {
  border-color: var(--border, #e5e5e5);
  background: var(--card-bg, #ffffff);
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  position: relative;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--muted, #737373);
  flex-shrink: 0;
}

.status-dot.active {
  background: var(--success, #16a34a);
}

.status-dot.active::after {
  content: "";
  position: absolute;
  inset: -3px;
  border-radius: 50%;
  border: 2px solid var(--success, #16a34a);
  opacity: 0.3;
  animation: pulse-ring 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

@keyframes pulse-ring {
  0% { transform: scale(1); opacity: 0.4; }
  80%, 100% { transform: scale(1.6); opacity: 0; }
}

.status-text {
  font-size: 13px;
  font-weight: 500;
  color: var(--fg-2, #171717);
}

.gateway-status.running .status-text {
  color: var(--success, #16a34a);
  font-weight: 600;
}

.status-address {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--muted, #737373);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  background: var(--surface-alt, #f5f5f5);
  padding: 3px 8px;
  border-radius: var(--radius-sm, 6px);
  border: 1px solid var(--border-soft, #ededed);
  margin-left: auto;
}

.gateway-status.running .status-address {
  background: var(--success-soft, rgba(22,163,74,0.10));
  border-color: var(--success-soft, rgba(22,163,74,0.12));
  color: var(--success, #16a34a);
}

.btn-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 600;
  font-family: inherit;
  border: 1px solid var(--border, #e5e5e5);
  border-radius: var(--radius, 8px);
  cursor: pointer;
  transition: background-color var(--transition), border-color var(--transition), color var(--transition);
  background: var(--surface, #ffffff);
  color: var(--fg-2, #171717);
}

.btn-toggle:hover {
  background: var(--surface-alt, #f5f5f5);
  border-color: var(--muted, #737373);
}

/* 运行中的停止按钮 — 红黑风格 */
.gateway-status.running .btn-toggle {
  background: var(--surface, #ffffff);
  color: var(--danger, #dc2626);
  border-color: var(--border, #e5e5e5);
}

.gateway-status.running .btn-toggle:hover {
  background: var(--danger-soft, rgba(220,38,38,0.10));
  border-color: var(--danger, #dc2626);
}

/* 未运行时启动按钮 — 实心黑 */
.gateway-status:not(.running) .btn-toggle:not([disabled]) {
  background: var(--fg, #0a0a0a);
  color: var(--surface, #ffffff);
  border-color: var(--fg, #0a0a0a);
}

.gateway-status:not(.running) .btn-toggle:not([disabled]):hover {
  background: var(--fg-2, #171717);
  border-color: var(--fg-2, #171717);
}

.btn-toggle[disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-loading {
  width: 10px;
  height: 10px;
  border: 1.5px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 暗色适配 */
body.dark .gateway-status {
  background: var(--card-bg, #171717);
  border-color: var(--border, #282828);
}

body.dark .status-address {
  background: var(--surface-alt, #262626);
  border-color: var(--border, #282828);
  color: var(--muted, #a1a1a1);
}

body.dark .gateway-status.running .status-address {
  background: var(--success-soft, rgba(98,209,120,0.14));
  border-color: var(--success-soft, rgba(98,209,120,0.18));
  color: var(--success, #62d178);
}

body.dark .btn-toggle {
  background: var(--surface, #171717);
  color: var(--fg-2, #e5e5e5);
  border-color: var(--border, #282828);
}

body.dark .btn-toggle:hover {
  background: var(--surface-alt, #262626);
  border-color: var(--muted, #a1a1a1);
}

body.dark .gateway-status.running .btn-toggle {
  background: var(--surface, #171717);
  color: var(--danger, #ff6166);
  border-color: var(--border, #282828);
}

body.dark .gateway-status.running .btn-toggle:hover {
  background: var(--danger-soft, rgba(255,97,102,0.14));
  border-color: var(--danger, #ff6166);
}

body.dark .gateway-status:not(.running) .btn-toggle:not([disabled]) {
  background: var(--fg, #fafafa);
  color: var(--surface, #171717);
  border-color: var(--fg, #fafafa);
}

body.dark .gateway-status:not(.running) .btn-toggle:not([disabled]):hover {
  background: var(--fg-2, #e5e5e5);
  border-color: var(--fg-2, #e5e5e5);
}
</style>
