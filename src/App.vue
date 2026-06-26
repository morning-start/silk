<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";

const router = useRouter();
const message = ref("Silk 丝路 加载中...");
const status = ref<any>(null);

onMounted(async () => {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    status.value = await invoke("gateway_status");
    message.value = "网关状态: " + (status.value?.running ? "运行中" : "已停止");
  } catch (e: any) {
    message.value = "错误: " + (e.message || "无法连接");
  }
});

function goTo(path: string) {
  router.push(path);
}
</script>

<template>
  <div style="padding: 20px; font-family: sans-serif">
    <h1>Silk 丝路</h1>
    <p>{{ message }}</p>
    <div style="margin: 20px 0">
      <button @click="goTo('/providers')">Provider 管理</button>
      <button @click="goTo('/groups')">分组</button>
      <button @click="goTo('/routing-rules')">路由规则</button>
      <button @click="goTo('/logs')">日志</button>
      <button @click="goTo('/settings')">设置</button>
    </div>
    <router-view />
  </div>
</template>
