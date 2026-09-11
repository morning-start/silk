<template>
  <div class="splash-screen" v-if="visible">
    <div class="splash-content">
      <div class="logo-container">
        <div class="logo-icon">
          <svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <defs>
              <linearGradient id="logoGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" style="stop-color:#0891b2;stop-opacity:1" />
                <stop offset="100%" style="stop-color:#06b6d4;stop-opacity:1" />
              </linearGradient>
            </defs>
            <circle cx="50" cy="50" r="45" fill="url(#logoGradient)" />
            <text x="50" y="65" font-family="Arial, sans-serif" font-size="40" font-weight="bold" fill="white" text-anchor="middle">S</text>
          </svg>
        </div>
        <h1 class="app-name">Silk</h1>
        <p class="app-tagline">您的个人AI总机</p>
      </div>

      <div class="loading-container">
        <div class="loading-steps">
          <div
            v-for="(step, index) in loadingSteps"
            :key="index"
            class="loading-step"
            :class="{ 'completed': step.completed, 'current': step.current }"
          >
            <span class="step-icon">
              <span v-if="step.completed">✓</span>
              <span v-else-if="step.current" class="spinner">◌</span>
              <span v-else>○</span>
            </span>
            <span class="step-text">{{ step.text }}</span>
          </div>
        </div>
      </div>

      <div class="version-info">
        <p>版本 {{ version }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { getVersion } from '@tauri-apps/api/app';

interface LoadingStep {
  text: string;
  completed: boolean;
  current: boolean;
}

defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  (e: 'complete'): void;
}>();

const loadingSteps = ref<LoadingStep[]>([
  { text: '初始化应用', completed: false, current: true },
  { text: '加载配置', completed: false, current: false },
  { text: '准备就绪', completed: false, current: false },
]);

const version = ref('');

onMounted(async () => {
  version.value = await getVersion();
  await simulateLoading();
});

async function simulateLoading() {
  // 步骤1：初始化应用
  await delay(400);
  loadingSteps.value[0].completed = true;
  loadingSteps.value[0].current = false;
  loadingSteps.value[1].current = true;

  // 步骤2：加载配置
  await delay(400);
  loadingSteps.value[1].completed = true;
  loadingSteps.value[1].current = false;
  loadingSteps.value[2].current = true;

  // 步骤3：准备就绪
  await delay(300);
  loadingSteps.value[2].completed = true;
  loadingSteps.value[2].current = false;

  // 延迟后关闭启动画面
  await delay(300);
  emit('complete');
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
</script>

<style scoped>
.splash-screen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background:
    radial-gradient(640px 380px at 80% -10%, rgba(99, 102, 241, 0.22), transparent 60%),
    radial-gradient(560px 340px at -10% 110%, rgba(6, 182, 212, 0.18), transparent 60%),
    linear-gradient(135deg, #0a1220 0%, #0f172a 60%, #101826 100%);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
  animation: fadeIn 0.3s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.splash-content {
  text-align: center;
  color: white;
}

.logo-container {
  margin-bottom: 48px;
}

.logo-icon {
  width: 88px;
  height: 88px;
  margin: 0 auto 24px;
  filter: drop-shadow(0 8px 24px rgba(6, 182, 212, 0.35));
  animation: logoFloat 2.6s ease-in-out infinite;
}

@keyframes logoFloat {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-5px); }
}

.logo-icon svg {
  width: 100%;
  height: 100%;
}

.app-name {
  font-size: 36px;
  font-weight: 600;
  margin: 0 0 8px 0;
  letter-spacing: 2px;
  background: linear-gradient(135deg, #22d3ee, #818cf8);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.app-tagline {
  font-size: 14px;
  color: #94a3b8;
  margin: 0;
}

.loading-container {
  margin-bottom: 48px;
}

.loading-steps {
  display: flex;
  flex-direction: column;
  gap: 12px;
  text-align: left;
  max-width: 200px;
  margin: 0 auto;
}

.loading-step {
  display: flex;
  align-items: center;
  gap: 10px;
  opacity: 0.4;
  transition: opacity 0.3s;
  font-size: 13px;
}

.loading-step.completed {
  opacity: 1;
  color: #34d399;
}

.loading-step.current {
  opacity: 1;
  color: #22d3ee;
}

.step-icon {
  width: 16px;
  text-align: center;
}

.spinner {
  display: inline-block;
  animation: spin 1s linear infinite;
  text-shadow: 0 0 8px rgba(34, 211, 238, 0.8);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.version-info {
  color: #64748b;
  font-size: 12px;
  font-family: var(--font-mono, 'JetBrains Mono', ui-monospace, monospace);
}
</style>
