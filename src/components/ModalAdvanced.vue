<script setup lang="ts">
import { ref } from "vue";

/**
 * 弹窗内的「高级设置」折叠区。
 *
 * 设计意图：把低频/高风险字段收起来，但**不隐藏它的存在**——
 * count 表示当前有几项偏离默认值，非 0 时显示琥珀色徽标，
 * 用户不必展开就能知道"有没有被我改过"。
 *
 * 样式来自 style.css 的 `m-adv*` 全局类（浅色/深色共用一套令牌）。
 */
const props = withDefaults(
  defineProps<{
    title?: string;
    /** 已偏离默认值的项数；不传则不显示徽标 */
    count?: number;
    /** 右侧补充说明，如「代理 · 超时 · 重试」 */
    hint?: string;
    defaultOpen?: boolean;
  }>(),
  {
    title: "高级设置",
    count: undefined,
    hint: "",
    defaultOpen: false,
  },
);

const open = ref(props.defaultOpen);
</script>

<template>
  <div class="m-adv" :class="{ open }">
    <button type="button" class="m-adv-head" :aria-expanded="open" @click="open = !open">
      <span class="m-adv-chevron">▸</span>
      <span class="m-adv-title">{{ title }}</span>
      <span
        v-if="count !== undefined"
        class="m-adv-badge"
        :class="{ 'is-default': count === 0 }"
      >
        {{ count > 0 ? `已自定义 ${count} 项` : "使用默认值" }}
      </span>
      <span v-if="hint" class="m-adv-hint">{{ hint }}</span>
    </button>

    <Transition name="m-adv-expand">
      <div v-show="open" class="m-adv-body">
        <slot />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.m-adv-expand-enter-active,
.m-adv-expand-leave-active {
  transition: opacity var(--transition), transform var(--transition);
}

.m-adv-expand-enter-from,
.m-adv-expand-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

@media (prefers-reduced-motion: reduce) {
  .m-adv-expand-enter-active,
  .m-adv-expand-leave-active {
    transition: none;
  }
}
</style>
