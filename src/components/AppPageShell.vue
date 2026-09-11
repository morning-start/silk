<script setup lang="ts">
import { NButton, NEmpty, NSpin } from "naive-ui";

defineProps<{
  title: string;
  countLabel?: string;
  loading?: boolean;
  error?: string | null;
  empty?: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
  reloadText?: string;
}>();

const emit = defineEmits<{
  reload: [];
}>();
</script>

<template>
  <div class="app-page-shell">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">{{ title }}</h2>
        <slot name="count">
          <NTag v-if="countLabel" size="small" type="info">{{ countLabel }}</NTag>
        </slot>
      </div>
      <div class="toolbar-right">
        <slot name="actions" />
      </div>
    </div>

    <slot name="before" />

    <NSpin :show="loading" style="min-height: 220px">
      <template v-if="error && !loading">
        <div class="error-state">
          <div class="error-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          </div>
          <h3 class="error-title">数据加载失败</h3>
          <p class="error-desc">{{ error }}</p>
          <slot name="error-action">
            <NButton type="primary" @click="emit('reload')">{{ reloadText || "重新加载" }}</NButton>
          </slot>
        </div>
      </template>

      <template v-else-if="empty && !loading">
        <slot name="empty">
          <NEmpty :description="emptyDescription || emptyTitle || '暂无数据'" />
        </slot>
      </template>

      <slot v-else />
    </NSpin>

    <slot name="after" />
  </div>
</template>

<style scoped>
.app-page-shell {
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: 100%;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin: 0;
  letter-spacing: -0.02em;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.error-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 280px;
  gap: 12px;
  padding: 48px 32px;
  text-align: center;
  border-radius: var(--radius-lg, 10px);
  background: var(--card-bg, #ffffff);
  border: 1px solid var(--border, #e5e5e5);
}

.error-icon {
  color: var(--danger, #dc2626);
}

.error-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--fg, #0a0a0a);
  margin: 0;
}

.error-desc {
  font-size: 12.5px;
  color: var(--muted, #737373);
  margin: 0;
  max-width: 360px;
}
</style>
