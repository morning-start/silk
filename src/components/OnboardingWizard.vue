<template>
  <div class="onboarding-overlay" v-if="show">
    <div class="onboarding-modal">
      <div class="onboarding-header">
        <h2>欢迎使用 Silk</h2>
        <p>让我们快速设置您的AI服务</p>
      </div>

      <div class="onboarding-steps">
        <div
          v-for="(step, index) in steps"
          :key="index"
          class="step-indicator"
          :class="{ 'active': currentStep === index, 'completed': currentStep > index }"
        >
          <span class="step-number">{{ index + 1 }}</span>
          <span class="step-label">{{ step.title }}</span>
        </div>
      </div>

      <div class="onboarding-content">
        <!-- 步骤1：选择服务 -->
        <div v-if="currentStep === 0" class="step-content">
          <h3>选择您要使用的AI服务</h3>
          <p class="step-description">选择一个或多个AI服务，我们将在下一步配置API密钥。</p>

          <div class="service-grid">
            <div
              v-for="service in availableServices"
              :key="service.id"
              class="service-card"
              :class="{ 'selected': selectedServices.includes(service.id) }"
              @click="toggleService(service.id)"
            >
              <div class="service-icon" :style="{ backgroundColor: service.color }">
                <span class="icon-text">{{ service.name.charAt(0) }}</span>
              </div>
              <div class="service-info">
                <div class="service-name">{{ service.name }}</div>
                <div class="service-desc">{{ service.description }}</div>
              </div>
              <div class="service-check" v-if="selectedServices.includes(service.id)">✓</div>
            </div>
          </div>
        </div>

        <!-- 步骤2：配置API密钥 -->
        <div v-if="currentStep === 1" class="step-content">
          <h3>配置API密钥</h3>
          <p class="step-description">请输入您选择的AI服务的API密钥。</p>

          <div class="api-key-form">
            <div v-for="serviceId in selectedServices" :key="serviceId" class="key-input-group">
              <label>{{ getServiceName(serviceId) }}</label>
              <div class="input-wrapper">
                <input
                  v-model="apiKeys[serviceId]"
                  type="password"
                  :placeholder="`输入 ${getServiceName(serviceId)} API密钥`"
                />
                <span class="input-hint">
                  <a :href="getKeyUrl(serviceId)" target="_blank">获取密钥 →</a>
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- 步骤3：完成 -->
        <div v-if="currentStep === 2" class="step-content">
          <div class="completion-icon">✓</div>
          <h3>配置完成！</h3>
          <p class="step-description">您已成功配置以下AI服务：</p>
          <div class="configured-services">
            <span v-for="serviceId in selectedServices" :key="serviceId" class="service-tag">
              {{ getServiceName(serviceId) }}
            </span>
          </div>
          <p class="completion-hint">点击"开始使用"进入主界面。</p>
        </div>
      </div>

      <div class="onboarding-actions">
        <button v-if="currentStep > 0" class="btn-secondary" @click="prevStep">
          上一步
        </button>
        <div class="spacer"></div>
        <button
          v-if="currentStep < 2"
          class="btn-primary"
          :disabled="!canProceed"
          @click="nextStep"
        >
          下一步
        </button>
        <button
          v-else
          class="btn-primary"
          :disabled="saving"
          @click="completeSetup"
        >
          {{ saving ? '保存中...' : '开始使用' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface Service {
  id: string;
  name: string;
  description: string;
  color: string;
  keyUrl: string;
}

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  (e: 'complete'): void;
}>();

const currentStep = ref(0);
const selectedServices = ref<string[]>([]);
const apiKeys = ref<Record<string, string>>({});
const saving = ref(false);

const steps = [
  { title: '选择服务' },
  { title: '配置密钥' },
  { title: '完成' },
];

const availableServices: Service[] = [
  {
    id: 'openai',
    name: 'OpenAI',
    description: 'GPT-4、GPT-3.5等模型',
    color: '#10a37f',
    keyUrl: 'https://platform.openai.com/api-keys',
  },
  {
    id: 'claude',
    name: 'Claude',
    description: 'Claude 3 Opus、Sonnet等',
    color: '#d97706',
    keyUrl: 'https://console.anthropic.com/api-keys',
  },
  {
    id: 'gemini',
    name: 'Gemini',
    description: 'Google AI模型',
    color: '#4285f4',
    keyUrl: 'https://makersuite.google.com/app/apikey',
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    description: 'DeepSeek Chat、Coder',
    color: '#0066ff',
    keyUrl: 'https://platform.deepseek.com/api_keys',
  },
];

const canProceed = computed(() => {
  if (currentStep.value === 0) {
    return selectedServices.value.length > 0;
  }
  if (currentStep.value === 1) {
    return selectedServices.value.every(id => apiKeys.value[id]?.trim());
  }
  return true;
});

function toggleService(serviceId: string) {
  const index = selectedServices.value.indexOf(serviceId);
  if (index === -1) {
    selectedServices.value.push(serviceId);
    apiKeys.value[serviceId] = '';
  } else {
    selectedServices.value.splice(index, 1);
    delete apiKeys.value[serviceId];
  }
}

function getServiceName(serviceId: string): string {
  return availableServices.find(s => s.id === serviceId)?.name || serviceId;
}

function getKeyUrl(serviceId: string): string {
  return availableServices.find(s => s.id === serviceId)?.keyUrl || '#';
}

function prevStep() {
  if (currentStep.value > 0) {
    currentStep.value--;
  }
}

function nextStep() {
  if (currentStep.value < 2) {
    currentStep.value++;
  }
}

async function completeSetup() {
  saving.value = true;
  try {
    const result = await invoke<{ success: boolean; message: string }>('save_onboarding_config', {
      services: selectedServices.value,
      apiKeys: apiKeys.value,
    });

    if (result.success) {
      localStorage.setItem('onboarding_completed', 'true');
      emit('complete');
    }
  } catch (error) {
    console.error('保存配置失败:', error);
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.onboarding-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 10000;
  backdrop-filter: blur(4px);
}

.onboarding-modal {
  background: white;
  border-radius: 16px;
  width: 560px;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.onboarding-header {
  text-align: center;
  padding: 32px 32px 16px;
}

.onboarding-header h2 {
  margin: 0 0 8px;
  font-size: 24px;
  font-weight: 600;
  color: #1e293b;
}

.onboarding-header p {
  margin: 0;
  color: #64748b;
  font-size: 14px;
}

.onboarding-steps {
  display: flex;
  justify-content: center;
  gap: 32px;
  padding: 16px 32px;
  border-bottom: 1px solid #e2e8f0;
}

.step-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  opacity: 0.4;
}

.step-indicator.active {
  opacity: 1;
  color: #0891b2;
}

.step-indicator.completed {
  opacity: 1;
  color: #10b981;
}

.step-number {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: #e2e8f0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
}

.step-indicator.active .step-number {
  background: #0891b2;
  color: white;
}

.step-indicator.completed .step-number {
  background: #10b981;
  color: white;
}

.step-label {
  font-size: 13px;
  font-weight: 500;
}

.onboarding-content {
  padding: 24px 32px;
}

.step-content h3 {
  margin: 0 0 8px;
  font-size: 18px;
  font-weight: 600;
  color: #1e293b;
}

.step-description {
  margin: 0 0 20px;
  color: #64748b;
  font-size: 14px;
}

.service-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.service-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  border: 2px solid #e2e8f0;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.service-card:hover {
  border-color: #cbd5e1;
  background: #f8fafc;
}

.service-card.selected {
  border-color: #0891b2;
  background: #f0fdfa;
}

.service-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.icon-text {
  color: white;
  font-size: 18px;
  font-weight: 600;
}

.service-info {
  flex: 1;
}

.service-name {
  font-weight: 600;
  color: #1e293b;
  margin-bottom: 2px;
}

.service-desc {
  font-size: 12px;
  color: #64748b;
}

.service-check {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: #0891b2;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
}

.api-key-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.key-input-group label {
  display: block;
  font-weight: 600;
  color: #1e293b;
  margin-bottom: 6px;
  font-size: 14px;
}

.input-wrapper {
  position: relative;
}

.input-wrapper input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  font-size: 14px;
  outline: none;
  transition: border-color 0.2s;
  box-sizing: border-box;
}

.input-wrapper input:focus {
  border-color: #0891b2;
}

.input-hint {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
}

.input-hint a {
  color: #0891b2;
  text-decoration: none;
}

.input-hint a:hover {
  text-decoration: underline;
}

.completion-icon {
  width: 64px;
  height: 64px;
  border-radius: 50%;
  background: #10b981;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  margin: 0 auto 16px;
}

.configured-services {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: center;
  margin: 16px 0;
}

.service-tag {
  padding: 6px 12px;
  background: #f0fdfa;
  color: #0891b2;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
}

.completion-hint {
  color: #64748b;
  font-size: 14px;
  text-align: center;
  margin: 16px 0 0;
}

.onboarding-actions {
  display: flex;
  align-items: center;
  padding: 16px 32px 24px;
}

.spacer {
  flex: 1;
}

.btn-secondary {
  padding: 10px 20px;
  background: #f1f5f9;
  color: #475569;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.btn-secondary:hover {
  background: #e2e8f0;
}

.btn-primary {
  padding: 10px 24px;
  background: #0891b2;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.btn-primary:hover:not(:disabled) {
  background: #0e7490;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
