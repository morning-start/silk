<template>
  <div class="quick-actions">
    <div class="quick-actions-header">
      <h3>快捷操作</h3>
      <button class="btn-settings" @click="$emit('open-settings')">
        ⚙️
      </button>
    </div>

    <div class="service-grid">
      <div
        v-for="service in services"
        :key="service.id"
        class="service-card"
        :class="{ 'active': service.active }"
        @click="toggleService(service)"
      >
        <div class="service-icon" :style="{ backgroundColor: service.color }">
          <span class="icon-text">{{ service.name.charAt(0) }}</span>
        </div>
        <div class="service-info">
          <div class="service-name">{{ service.name }}</div>
          <div class="service-status">
            <span class="status-dot" :class="{ 'active': service.active }"></span>
            <span class="status-text">{{ service.active ? '已启用' : '未启用' }}</span>
          </div>
        </div>
      </div>
    </div>

    <div class="action-buttons">
      <button class="btn-action btn-start" @click="startAllServices">
        ▶ 启动所有
      </button>
      <button class="btn-action btn-stop" @click="stopAllServices">
        ⏹ 停止所有
      </button>
      <button class="btn-action btn-test" @click="testConnections">
        🔗 测试连接
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface Service {
  id: string;
  name: string;
  color: string;
  active: boolean;
}

defineEmits<{
  (e: 'open-settings'): void;
}>();

const services = ref<Service[]>([
  { id: 'openai', name: 'OpenAI', color: '#10a37f', active: false },
  { id: 'claude', name: 'Claude', color: '#d97706', active: false },
  { id: 'gemini', name: 'Gemini', color: '#4285f4', active: false },
  { id: 'wenxin', name: '文心一言', color: '#2932e1', active: false },
  { id: 'tongyi', name: '通义千问', color: '#ff6a00', active: false },
  { id: 'deepseek', name: 'DeepSeek', color: '#0066ff', active: false },
]);

onMounted(async () => {
  await loadServiceStatus();
});

async function loadServiceStatus() {
  try {
    const providers = await invoke<Array<{ id: string; name: string; status: string }>>('list_providers');
    services.value.forEach(service => {
      const provider = providers.find(p =>
        p.name.toLowerCase().includes(service.id) ||
        p.name === service.name
      );
      service.active = provider?.status === 'enabled';
    });
  } catch (error) {
    console.error('加载服务状态失败:', error);
  }
}

async function toggleService(service: Service) {
  // 这里需要调用后端API来启用/禁用服务
  // 暂时只更新本地状态
  service.active = !service.active;
}

async function startAllServices() {
  try {
    await invoke('gateway_start');
    services.value.forEach(s => s.active = true);
  } catch (error) {
    console.error('启动失败:', error);
  }
}

async function stopAllServices() {
  try {
    await invoke('gateway_stop');
    services.value.forEach(s => s.active = false);
  } catch (error) {
    console.error('停止失败:', error);
  }
}

async function testConnections() {
  // 这里可以实现连接测试逻辑
  alert('连接测试功能开发中...');
}
</script>

<style scoped>
.quick-actions {
  background: white;
  border-radius: 12px;
  padding: 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.quick-actions-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.quick-actions-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1e293b;
}

.btn-settings {
  background: none;
  border: none;
  font-size: 18px;
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  transition: background 0.2s;
}

.btn-settings:hover {
  background: #f1f5f9;
}

.service-grid {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 16px;
}

.service-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
}

.service-card:hover {
  border-color: #cbd5e1;
  background: #f8fafc;
}

.service-card.active {
  border-color: #10b981;
  background: #f0fdf4;
}

.service-icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.icon-text {
  color: white;
  font-size: 16px;
  font-weight: 600;
}

.service-info {
  flex: 1;
  min-width: 0;
}

.service-name {
  font-weight: 600;
  color: #1e293b;
  font-size: 14px;
  margin-bottom: 2px;
}

.service-status {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #cbd5e1;
}

.status-dot.active {
  background: #10b981;
}

.status-text {
  font-size: 12px;
  color: #64748b;
}

.action-buttons {
  display: flex;
  gap: 8px;
}

.btn-action {
  flex: 1;
  padding: 8px 12px;
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-start {
  background: #10b981;
  color: white;
}

.btn-start:hover {
  background: #059669;
}

.btn-stop {
  background: #f1f5f9;
  color: #475569;
}

.btn-stop:hover {
  background: #e2e8f0;
}

.btn-test {
  background: #f0f9ff;
  color: #0891b2;
}

.btn-test:hover {
  background: #e0f2fe;
}
</style>
