<script setup lang="ts">
import { ref, watch } from "vue";
import { NButton, NInput } from "naive-ui";

interface McpRow {
  name: string;
  url: string;
}

const props = defineProps<{
  modelValue: Record<string, { url: string }>;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: Record<string, { url: string }>): void;
}>();

const rows = ref<McpRow[]>([]);

// 外部值（config_json.mcpServers）→ 行列表
watch(
  () => props.modelValue,
  (val) => {
    rows.value = Object.entries(val || {}).map(([name, cfg]) => ({
      name,
      url: typeof cfg?.url === "string" ? cfg.url : "",
    }));
  },
  { immediate: true }
);

// 行列表 → 外部值（仅保留非空名称）
function sync() {
  const out: Record<string, { url: string }> = {};
  for (const r of rows.value) {
    if (r.name.trim()) {
      out[r.name.trim()] = { url: r.url.trim() };
    }
  }
  emit("update:modelValue", out);
}

function addRow() {
  rows.value.push({ name: "", url: "" });
}

function removeRow(index: number) {
  rows.value.splice(index, 1);
  sync();
}
</script>

<template>
  <div class="mcp-editor">
    <div class="mcp-editor-header">
      <span class="mcp-editor-title">MCP 服务器</span>
      <NButton size="tiny" quaternary @click="addRow">+ 添加</NButton>
    </div>

    <div v-if="rows.length === 0" class="mcp-editor-empty">
      无 MCP 服务器，激活时会写入对应 harness 的 MCP 段
    </div>

    <div v-for="(row, index) in rows" :key="index" class="mcp-row">
      <NInput
        v-model:value="row.name"
        size="small"
        placeholder="名称（如 echo）"
        style="flex: 1"
        @update:value="sync"
      />
      <NInput
        v-model:value="row.url"
        size="small"
        placeholder="URL（如 http://127.0.0.1:3000）"
        style="flex: 2"
        @update:value="sync"
      />
      <NButton size="tiny" quaternary type="error" @click="removeRow(index)">删除</NButton>
    </div>
  </div>
</template>

<style scoped>
.mcp-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 8px;
  padding: 10px;
}

.mcp-editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.mcp-editor-title {
  font-size: 13px;
  font-weight: 600;
}

.mcp-editor-empty {
  font-size: 12px;
  color: var(--muted, #94a3b8);
}

.mcp-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
