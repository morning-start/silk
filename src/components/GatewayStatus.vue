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
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  background:
    radial-gradient(280px 140px at 0% 0%, rgba(6, 182, 212, 0.08), transparent 60%),
    radial-gradient(240px 160px at 100% 100%, rgba(99, 102, 241, 0.06), transparent 60%),
    var(--glass-bg, rgba(255, 255, 255, 0.72));
  backdrop-filter: blur(var(--glass-blur, 18px)) saturate(1.4);
  -webkit-backdrop-filter: blur(var(--glass-blur, 18px)) saturate(1.4);
  border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.6));
  border-radius: var(--radius-lg, 14px);
  box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.05));
  position: relative;
  overflow: hidden;
  transition: all 0.25s ease;
}

.gateway-status::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(90deg, rgba(6, 182, 212, 0.4), rgba(99, 102, 241, 0.4), transparent 60%);
  opacity: 0.6;
}

.gateway-status.running {
  background:
    radial-gradient(320px 180px at 0% 0%, rgba(16, 185, 129, 0.08), transparent 60%),
    radial-gradient(280px 140px at 100% 100%, rgba(6, 182, 212, 0.06), transparent 60%),
    var(--glass-bg, rgba(255, 255, 255, 0.72));
  border-color: rgba(16, 185, 129, 0.25);
}

.gateway-status.running::before {
  background: linear-gradient(90deg, #10b981, #06b6d4, #6366f1);
  opacity: 1;
  box-shadow: 0 0 12px rgba(16, 185, 129, 0.5);
}

/* 运行时的呼吸光晕 */
.gateway-status.running .status-glow {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: radial-gradient(ellipse at 20% 50%, rgba(16, 185, 129, 0.06), transparent 60%);
  animation: breathe 4s ease-in-out infinite;
}

@keyframes breathe {
  0%, 100% { opacity: 0.5; }
  50% { opacity: 1; }
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-dot {
  position: relative;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--muted, #94a3b8);
  transition: all 0.3s ease;
  flex-shrink: 0;
}

.status-dot::after {
  content: "";
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  background: transparent;
  transition: all 0.3s ease;
}

.status-dot.active {
  background: #10b981;
  box-shadow: 0 0 10px rgba(16, 185, 129, 0.6), 0 0 20px rgba(16, 185, 129, 0.3);
}

.status-dot.active::after {
  animation: ping 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

@keyframes ping {
  0% { transform: scale(1); opacity: 0.6; }
  80%, 100% { transform: scale(1.8); opacity: 0; }
}

.status-text {
  font-size: 13px;
  font-weight: 500;
  color: var(--fg-2, #334155);
}

.gateway-status.running .status-text {
  color: #10b981;
  font-weight: 600;
}

.status-address {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--muted, #64748b);
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
  background: var(--surface-alt, #f1f5f9);
  padding: 3px 10px;
  border-radius: 6px;
  border: 1px solid var(--border-soft, #e2e8f0);
  margin-left: auto;
  transition: all 0.2s ease;
}

.gateway-status.running .status-address {
  background: rgba(16, 185, 129, 0.06);
  border-color: rgba(16, 185, 129, 0.2);
  color: #059669;
}

.btn-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  font-size: 12px;
  font-weight: 500;
  font-family: inherit;
  border: none;
  border-radius: var(--radius, 8px);
  cursor: pointer;
  transition: all 150ms ease;
  position: relative;
  overflow: hidden;
}

/* 停止按钮 — 默认玻璃效果 */
.btn-toggle:not([disabled]) {
  background: var(--surface, #ffffff);
  color: var(--fg-2, #334155);
  border: 1px solid var(--border, #cbd5e1);
}

.btn-toggle:not([disabled]):hover {
  background: var(--surface-alt, #f1f5f9);
  border-color: var(--muted, #64748b);
}

/* 运行中的停止按钮 — 红色微妙提示 */
.gateway-status.running .btn-toggle {
  background: rgba(239, 68, 68, 0.06);
  color: var(--danger, #ef4444);
  border: 1px solid rgba(239, 68, 68, 0.2);
}

.gateway-status.running .btn-toggle:hover {
  background: rgba(239, 68, 68, 0.12);
  border-color: rgba(239, 68, 68, 0.4);
  box-shadow: 0 0 12px rgba(239, 68, 68, 0.15);
}

/* 未运行时启动按钮 — 主题渐变 */
.gateway-status:not(.running) .btn-toggle:not([disabled]) {
  background: var(--gradient, linear-gradient(135deg, #06b6d4, #6366f1));
  color: #ffffff;
  border: none;
  box-shadow: 0 2px 10px -2px rgba(6, 182, 212, 0.4);
}

.gateway-status:not(.running) .btn-toggle:not([disabled]):hover {
  background: var(--gradient-hover, linear-gradient(135deg, #0ea5e9, #4f46e5));
  box-shadow: 0 4px 16px -2px rgba(6, 182, 212, 0.5);
  transform: translateY(-1px);
}

.btn-toggle[disabled] {
  opacity: 0.7;
  cursor: not-allowed;
}

/* 加载旋转点 */
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
  background:
    radial-gradient(280px 160px at 0% 0%, rgba(34, 211, 238, 0.06), transparent 60%),
    radial-gradient(240px 140px at 100% 100%, rgba(129, 140, 248, 0.05), transparent 60%),
    var(--glass-bg, rgba(16, 24, 38, 0.66));
  border-color: var(--glass-border, rgba(148, 163, 184, 0.14));
}

body.dark .gateway-status.running {
  background:
    radial-gradient(300px 180px at 0% 0%, rgba(16, 185, 129, 0.08), transparent 60%),
    radial-gradient(260px 140px at 100% 100%, rgba(34, 211, 238, 0.06), transparent 60%),
    var(--glass-bg, rgba(16, 24, 38, 0.66));
  border-color: rgba(16, 185, 129, 0.25);
}

body.dark .status-address {
  background: rgba(255, 255, 255, 0.04);
  border-color: var(--border, #26334a);
  color: var(--muted, #8b9bb4);
}

body.dark .gateway-status.running .status-address {
  background: rgba(16, 185, 129, 0.08);
  border-color: rgba(16, 185, 129, 0.2);
  color: #34d399;
}
</style>
