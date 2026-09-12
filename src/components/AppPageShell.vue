<script setup lang="ts">
import { NButton, NEmpty, NSpin } from "naive-ui";

/**
 * 全站唯一的页面骨架：页头（标题 / 说明 / 操作）+ 加载态 + 空态 + 错误态。
 *
 * 页面不要再自己写 page-header / page-title / empty-state，
 * 结构统一由这里产出，样式统一走 style.css 的 p-* / s-* 类。
 */
defineProps<{
  title: string;
  /** 标题下方的一句话说明，用于交代这个页面在管什么 */
  desc?: string;
  loading?: boolean;
  error?: string | null;
  empty?: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
  reloadText?: string;
  /** 空态是否用朴素提示（true）还是 NEmpty 图形（false，默认） */
  emptyPlain?: boolean;
}>();

const emit = defineEmits<{
  reload: [];
}>();
</script>

<template>
  <div class="p-page">
    <header class="p-head">
      <div class="p-head-main">
        <div class="p-head-row">
          <h1 class="p-head-title">{{ title }}</h1>
          <slot name="count" />
        </div>
        <p v-if="desc" class="p-head-desc">{{ desc }}</p>
        <slot name="head-extra" />
      </div>

      <div v-if="$slots.actions" class="p-head-actions">
        <slot name="actions" />
      </div>
    </header>

    <slot name="before" />

    <NSpin :show="loading" style="min-height: 220px">
      <template v-if="error && !loading">
        <div class="s-state">
          <span class="s-state-icon is-error">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              style="width: 40px; height: 40px"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
          </span>
          <h3 class="s-state-title">数据加载失败</h3>
          <p class="s-state-desc">{{ error }}</p>
          <slot name="error-action">
            <NButton type="primary" @click="emit('reload')">{{ reloadText || "重新加载" }}</NButton>
          </slot>
        </div>
      </template>

      <template v-else-if="empty && !loading">
        <slot name="empty">
          <div v-if="emptyPlain" class="s-state">
            <h3 class="s-state-title">{{ emptyTitle || "暂无数据" }}</h3>
            <p v-if="emptyDescription" class="s-state-desc">{{ emptyDescription }}</p>
          </div>
          <NEmpty v-else :description="emptyDescription || emptyTitle || '暂无数据'" style="padding: 32px 0" />
        </slot>
      </template>

      <slot v-else />
    </NSpin>

    <slot name="after" />
  </div>
</template>
