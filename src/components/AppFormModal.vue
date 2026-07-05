<script setup lang="ts">
import { NButton, NModal } from "naive-ui";

defineProps<{
  show: boolean;
  title: string;
  width?: string;
  submitText?: string;
  submitDisabled?: boolean;
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
    :style="{ maxWidth: width || '640px' }"
    :bordered="false"
    :segmented="{ footer: true }"
    @update:show="(value) => emit('update:show', value)"
  >
    <slot />

    <template #footer>
      <div class="modal-footer">
        <slot name="footer">
          <NButton @click="close">取消</NButton>
          <NButton type="primary" :disabled="submitDisabled" @click="emit('submit')">
            {{ submitText || "保存" }}
          </NButton>
        </slot>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
