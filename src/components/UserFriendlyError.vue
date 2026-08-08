<template>
  <div class="error-alert" v-if="error" :class="errorClass">
    <div class="error-icon">
      <span v-if="error.error_type === 'Authentication'">🔑</span>
      <span v-else-if="error.error_type === 'RateLimit'">⏱️</span>
      <span v-else-if="error.error_type === 'Timeout'">⏰</span>
      <span v-else-if="error.error_type === 'Network'">🌐</span>
      <span v-else-if="error.error_type === 'ServiceUnavailable'">🔧</span>
      <span v-else>⚠️</span>
    </div>

    <div class="error-content">
      <div class="error-title">{{ error.title }}</div>
      <div class="error-message">{{ error.message }}</div>
      <div class="error-suggestion" v-if="error.suggestion && showSuggestion">
        💡 {{ error.suggestion }}
      </div>
    </div>

    <div class="error-actions">
      <button
        v-if="error.suggestion && !showSuggestion"
        class="btn-link"
        @click="showSuggestion = true"
      >
        查看建议
      </button>
      <button v-if="retryable" class="btn-retry" @click="$emit('retry')">
        重试
      </button>
      <button class="btn-close" @click="$emit('close')">
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';

interface UserFriendlyError {
  title: string;
  message: string;
  suggestion?: string;
  error_type: 'Authentication' | 'RateLimit' | 'ServiceUnavailable' | 'BadRequest' | 'Network' | 'Timeout' | 'Unknown';
}

const props = withDefaults(defineProps<{
  error: UserFriendlyError | null;
  retryable?: boolean;
}>(), {
  retryable: true,
});

defineEmits<{
  (e: 'close'): void;
  (e: 'retry'): void;
}>();

const showSuggestion = ref(false);

const errorClass = computed(() => {
  if (!props.error) return '';
  return `error-${props.error.error_type.toLowerCase()}`;
});
</script>

<style scoped>
.error-alert {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 10px;
  background: #fef2f2;
  border: 1px solid #fecaca;
  animation: slideIn 0.2s ease-out;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.error-authentication {
  background: #fffbeb;
  border-color: #fde68a;
}

.error-ratelimit {
  background: #fef3c7;
  border-color: #fde68a;
}

.error-timeout {
  background: #fef9c3;
  border-color: #fde047;
}

.error-network {
  background: #f0f9ff;
  border-color: #bae6fd;
}

.error-serviceunavailable {
  background: #f5f3ff;
  border-color: #ddd6fe;
}

.error-icon {
  font-size: 20px;
  line-height: 1;
  flex-shrink: 0;
}

.error-content {
  flex: 1;
  min-width: 0;
}

.error-title {
  font-weight: 600;
  color: #1e293b;
  font-size: 14px;
  margin-bottom: 4px;
}

.error-message {
  color: #475569;
  font-size: 13px;
  line-height: 1.5;
}

.error-suggestion {
  margin-top: 8px;
  padding: 8px 10px;
  background: rgba(255, 255, 255, 0.6);
  border-radius: 6px;
  font-size: 12px;
  color: #64748b;
}

.error-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.btn-link {
  background: none;
  border: none;
  color: #0891b2;
  font-size: 12px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
}

.btn-link:hover {
  background: rgba(8, 145, 178, 0.1);
}

.btn-retry {
  background: #0891b2;
  color: white;
  border: none;
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.2s;
}

.btn-retry:hover {
  background: #0e7490;
}

.btn-close {
  background: none;
  border: none;
  color: #94a3b8;
  font-size: 16px;
  cursor: pointer;
  padding: 4px;
  line-height: 1;
  border-radius: 4px;
}

.btn-close:hover {
  background: rgba(0, 0, 0, 0.05);
  color: #475569;
}
</style>
