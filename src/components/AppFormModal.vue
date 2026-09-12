<script setup lang="ts">
import { NButton, NModal } from "naive-ui";

defineProps<{
  show: boolean;
  title: string;
  width?: string;
  submitText?: string;
  submitDisabled?: boolean;
  /**
   * 常驻摘要（footer 左侧）：滚动到表单深处时，仍能看到当前配置的关键结论。
   * 例：["2 个密钥", "18 个模型", "openai"]  → 2 个密钥 · 18 个模型 · openai
   */
  summary?: string[];
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
  cancel: [];
  submit: [];
}>();

function close() {
  emit("update:show", false);
  emit("cancel");
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    :title="title"
    :style="{ width: `min(${width || '640px'}, calc(100vw - 32px))`, maxHeight: 'calc(100vh - 32px)' }"
    :bordered="false"
    :segmented="{ footer: true }"
    @update:show="(value) => emit('update:show', value)"
  >
    <div class="app-form-modal-body">
      <slot />
    </div>

    <template #footer>
      <div class="modal-footer">
        <div v-if="summary && summary.length" class="modal-summary">
          <template v-for="(item, index) in summary" :key="item + index">
            <span v-if="index > 0" class="modal-summary-dot" />
            <span>{{ item }}</span>
          </template>
        </div>
        <div class="modal-footer-main">
          <slot name="footer">
            <NButton @click="close">取消</NButton>
            <NButton type="primary" :disabled="submitDisabled" @click="emit('submit')">
              {{ submitText || "保存" }}
            </NButton>
          </slot>
        </div>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
/* 内容区限高滚动：表单内容较多时（如向导）弹窗不再无限长高 */
.app-form-modal-body {
  max-height: min(70vh, 640px);
  overflow-y: auto;
  padding: 2px 4px 4px 0;
  overscroll-behavior: contain;
}

.modal-footer {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 32px;
}

/* footer 左侧摘要占位，按钮组始终贴右 */
.modal-summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.modal-footer-main {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  flex: none;
}

@media (max-width: 560px) {
  .app-form-modal-body {
    max-height: calc(100vh - 148px);
  }

  .modal-footer-main :deep(.n-button) {
    flex: 1;
  }
}
</style>
