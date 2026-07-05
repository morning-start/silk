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
          <span v-if="countLabel">{{ countLabel }}</span>
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
