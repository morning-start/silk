<template>
  <div class="gateway-status" :class="{ 'running': status?.running }">
    <div class="status-indicator">
      <span class="status-dot" :class="{ 'active': status?.running }"></span>
      <span class="status-text">{{ statusText }}</span>
    </div>
    <div class="status-address" v-if="status?.running">
      {{ status.address }}
    </div>
    <button class="btn-toggle" @click="toggleGateway">
      {{ status?.running ? '停止' : '启动' }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

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
    } else {
      await invoke('gateway_start');
    }
    await fetchStatus();
  } catch (error) {
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
  padding: 8px 12px;
  background: #f8fafc;
  border-radius: 8px;
  border: 1px solid #e2e8f0;
}

.gateway-status.running {
  background: #f0fdf4;
  border-color: #bbf7d0;
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #94a3b8;
}

.status-dot.active {
  background: #10b981;
  box-shadow: 0 0 8px rgba(16, 185, 129, 0.4);
}

.status-text {
  font-size: 13px;
  font-weight: 500;
  color: #475569;
}

.status-address {
  font-size: 12px;
  color: #64748b;
  font-family: monospace;
  background: white;
  padding: 2px 8px;
  border-radius: 4px;
}

.btn-toggle {
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 500;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  background: #0891b2;
  color: white;
}

.btn-toggle:hover {
  background: #0e7490;
}

.gateway-status.running .btn-toggle {
  background: #f1f5f9;
  color: #475569;
}

.gateway-status.running .btn-toggle:hover {
  background: #e2e8f0;
}
</style>
