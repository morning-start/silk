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

interface LoadingStep {
  text: string;
  completed: boolean;
  current: boolean;
}

withDefaults(defineProps<{
  visible: boolean;
  version?: string;
}>(), {
  version: '1.0.0',
});

const emit = defineEmits<{
  (e: 'complete'): void;
}>();

const loadingSteps = ref<LoadingStep[]>([
  { text: '初始化应用', completed: false, current: true },
  { text: '加载配置', completed: false, current: false },
  { text: '准备就绪', completed: false, current: false },
]);

onMounted(async () => {
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
  background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
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
  width: 80px;
  height: 80px;
  margin: 0 auto 20px;
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
  color: #10b981;
}

.loading-step.current {
  opacity: 1;
  color: #06b6d4;
}

.step-icon {
  width: 16px;
  text-align: center;
}

.spinner {
  display: inline-block;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.version-info {
  color: #475569;
  font-size: 12px;
}
</style>
