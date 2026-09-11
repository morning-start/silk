<template>
  <div class="quick-actions">
    <div class="quick-actions-header">
      <h3>快捷操作</h3>
      <button class="btn-icon" @click="$emit('open-settings')" title="设置">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="width:16px;height:16px">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
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
        <div class="service-icon" :style="{ background: service.gradient }">
          <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="width:18px;height:18px">
            <!-- 根据服务 ID 显示不同图标 -->
            <circle v-if="service.id === 'openai'" cx="12" cy="12" r="10"/>
            <path v-if="service.id === 'openai'" d="M12 8v4l3 3"/>
            <path v-if="service.id === 'claude'" d="M12 2L2 7l10 5 10-5-10-5z"/>
            <path v-if="service.id === 'gemini'" d="M12 2L2 7l10 5 10-5-10-5z"/>
            <rect v-if="service.id === 'wenxin'" x="3" y="3" width="18" height="18" rx="2"/>
            <rect v-if="service.id === 'tongyi'" x="3" y="3" width="18" height="18" rx="2"/>
            <polygon v-if="service.id === 'deepseek'" points="12,2 22,8.5 22,15.5 12,22 2,15.5 2,8.5"/>
          </svg>
        </div>
        <div class="service-info">
          <div class="service-name">{{ service.name }}</div>
          <div class="service-status">
            <span class="status-dot" :class="{ 'active': service.active }"></span>
            <span class="status-text">{{ service.active ? '已启用' : '未启用' }}</span>
          </div>
        </div>
        <div class="service-toggle">
          <div class="toggle-track" :class="{ 'active': service.active }">
            <div class="toggle-thumb"></div>
          </div>
        </div>
      </div>
    </div>

    <div class="action-buttons">
      <button class="btn-action btn-start" @click="startAllServices">
        <svg viewBox="0 0 24 24" fill="currentColor" style="width:14px;height:14px"><polygon points="5,3 19,12 5,21"/></svg>
        启动所有
      </button>
      <button class="btn-action btn-stop" @click="stopAllServices">
        <svg viewBox="0 0 24 24" fill="currentColor" style="width:14px;height:14px"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>
        停止所有
      </button>
      <button class="btn-action btn-test" @click="testConnections">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="width:14px;height:14px"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
        测试连接
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
  gradient: string;
  active: boolean;
}

defineEmits<{
  (e: 'open-settings'): void;
}>();

const services = ref<Service[]>([
  { id: 'openai', name: 'OpenAI', gradient: 'linear-gradient(135deg, #10b981, #059669)', active: false },
  { id: 'claude', name: 'Claude', gradient: 'linear-gradient(135deg, #f59e0b, #d97706)', active: false },
  { id: 'gemini', name: 'Gemini', gradient: 'linear-gradient(135deg, #3b82f6, #2563eb)', active: false },
  { id: 'wenxin', name: '文心一言', gradient: 'linear-gradient(135deg, #6366f1, #4f46e5)', active: false },
  { id: 'tongyi', name: '通义千问', gradient: 'linear-gradient(135deg, #f97316, #ea580c)', active: false },
  { id: 'deepseek', name: 'DeepSeek', gradient: 'linear-gradient(135deg, #06b6d4, #0891b2)', active: false },
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
  alert('连接测试功能开发中...');
}
</script>

<style scoped>
.quick-actions {
  background: var(--glass-bg, rgba(255, 255, 255, 0.72));
  backdrop-filter: blur(18px) saturate(1.4);
  -webkit-backdrop-filter: blur(18px) saturate(1.4);
  border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.6));
  border-radius: var(--radius-xl, 20px);
  padding: 20px;
  box-shadow: var(--shadow-sm);
  position: relative;
  overflow: hidden;
}

/* 顶部渐变光带 */
.quick-actions::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--gradient, linear-gradient(135deg, #06b6d4 0%, #6366f1 100%));
  opacity: 0.85;
}

.quick-actions-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.quick-actions-header h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--fg, #0f172a);
  letter-spacing: -0.01em;
}

.btn-icon {
  width: 30px;
  height: 30px;
  display: inline-grid;
  place-items: center;
  border-radius: var(--radius, 8px);
  border: 1px solid var(--border-soft, #e2e8f0);
  background: var(--surface, #ffffff);
  color: var(--muted, #64748b);
  cursor: pointer;
  transition: all 150ms ease;
}

.btn-icon:hover {
  color: var(--accent, #0891b2);
  border-color: var(--accent, #0891b2);
  background: var(--accent-soft, rgba(8, 145, 178, 0.06));
}

.service-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.service-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius-lg, 14px);
  cursor: pointer;
  transition: all 150ms ease;
  background: var(--surface, #ffffff);
}

.service-card:hover {
  border-color: rgba(6, 182, 212, 0.3);
  background: var(--surface-alt, #f8fafc);
  box-shadow: 0 2px 8px rgba(6, 182, 212, 0.08);
}

.service-card.active {
  border-color: rgba(16, 185, 129, 0.3);
  background: var(--success-soft, rgba(16, 185, 129, 0.05));
}

.service-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
}

.service-info {
  flex: 1;
  min-width: 0;
}

.service-name {
  font-weight: 600;
  color: var(--fg, #0f172a);
  font-size: 13px;
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
  background: var(--border, #cbd5e1);
  transition: all 200ms ease;
}

.status-dot.active {
  background: #10b981;
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.5);
  animation: pulse-dot 2s infinite;
}

@keyframes pulse-dot {
  0%, 100% { box-shadow: 0 0 6px rgba(16, 185, 129, 0.5); }
  50% { box-shadow: 0 0 12px rgba(16, 185, 129, 0.8); }
}

.status-text {
  font-size: 11px;
  color: var(--muted, #64748b);
}

/* 自定义开关 */
.service-toggle {
  flex-shrink: 0;
}

.toggle-track {
  width: 32px;
  height: 18px;
  border-radius: 9px;
  background: var(--border, #cbd5e1);
  position: relative;
  transition: background 150ms ease;
}

.toggle-track.active {
  background: #10b981;
}

.toggle-thumb {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: white;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 150ms ease;
  box-shadow: 0 1px 3px rgba(0,0,0,0.2);
}

.toggle-track.active .toggle-thumb {
  transform: translateX(14px);
}

.action-buttons {
  display: flex;
  gap: 8px;
}

.btn-action {
  flex: 1;
  padding: 9px 12px;
  border: none;
  border-radius: var(--radius, 8px);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 150ms ease;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  font-family: inherit;
}

.btn-start {
  background: linear-gradient(135deg, #10b981, #059669);
  color: white;
  box-shadow: 0 2px 8px rgba(16, 185, 129, 0.3);
}

.btn-start:hover {
  background: linear-gradient(135deg, #059669, #047857);
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.4);
  transform: translateY(-1px);
}

.btn-stop {
  background: var(--surface, #ffffff);
  color: var(--fg-2, #334155);
  border: 1px solid var(--border, #cbd5e1);
}

.btn-stop:hover {
  background: var(--surface-alt, #f1f5f9);
  border-color: var(--muted, #64748b);
}

.btn-test {
  background: var(--accent-soft, rgba(8, 145, 178, 0.06));
  color: var(--accent, #0891b2);
  border: 1px solid rgba(8, 145, 178, 0.2);
}

.btn-test:hover {
  background: rgba(8, 145, 178, 0.12);
  border-color: rgba(8, 145, 178, 0.4);
}

/* 暗色适配 */
body.dark .service-card {
  background: var(--glass-bg, rgba(16, 24, 38, 0.66));
  border-color: var(--border, #26334a);
}

body.dark .service-card:hover {
  border-color: rgba(34, 211, 238, 0.3);
  background: var(--glass-bg-strong, rgba(16, 24, 38, 0.82));
}

body.dark .service-card.active {
  border-color: rgba(16, 185, 129, 0.3);
  background: var(--success-soft, rgba(16, 185, 129, 0.08));
}

body.dark .btn-stop {
  background: var(--surface, #101826);
  border-color: var(--border, #26334a);
  color: var(--fg-2, #c6d2e4);
}

body.dark .btn-stop:hover {
  background: var(--surface-alt, #151f31);
  border-color: var(--muted, #8b9bb4);
}

body.dark .btn-test {
  background: rgba(34, 211, 238, 0.08);
  border-color: rgba(34, 211, 238, 0.2);
  color: var(--accent, #22d3ee);
}

body.dark .btn-test:hover {
  background: rgba(34, 211, 238, 0.14);
  border-color: rgba(34, 211, 238, 0.4);
}
</style>
