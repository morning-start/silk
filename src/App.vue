<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import {
  NLayout,
  NLayoutSider,
  NMenu,
  NLayoutContent,
  NLayoutHeader,
  NButton,
  NText,
  NIcon,
  NBadge,
  useMessage,
} from "naive-ui";
import {
  ServerOutline,
  GitNetworkOutline,
  ListOutline,
  DocumentTextOutline,
  SettingsOutline,
  PlayOutline,
  StopOutline,
  RefreshOutline,
} from "@vicons/ionicons5";
import { useGatewayStore } from "./stores/gateway";
import { storeToRefs } from "pinia";

const router = useRouter();
const message = useMessage();
const gatewayStore = useGatewayStore();
const { status } = storeToRefs(gatewayStore);

const collapsed = ref(false);
const activeKey = ref("providers");

const menuOptions: any[] = [
  { label: "Provider 管理", key: "providers", icon: ServerOutline, route: "/providers" },
  { label: "负载均衡分组", key: "groups", icon: GitNetworkOutline, route: "/groups" },
  { label: "路由规则", key: "routing-rules", icon: ListOutline, route: "/routing-rules" },
  { label: "请求日志", key: "logs", icon: DocumentTextOutline, route: "/logs" },
  { label: "设置", key: "settings", icon: SettingsOutline, route: "/settings" },
];

function handleMenuSelect(key: string, item: any) {
  activeKey.value = key;
  router.push(item.route);
}

async function handleStart() {
  try {
    await gatewayStore.start();
    message.success("网关已启动");
  } catch (e: any) {
    message.error(e.message || "启动失败");
  }
}

async function handleStop() {
  try {
    await gatewayStore.stop();
    message.success("网关已停止");
  } catch (e: any) {
    message.error(e.message || "停止失败");
  }
}

async function handleRestart() {
  try {
    await gatewayStore.restart();
    message.success("网关已重启");
  } catch (e: any) {
    message.error(e.message || "重启失败");
  }
}

onMounted(() => {
  gatewayStore.fetchStatus();
});
</script>

<template>
  <n-layout has-sider style="height: 100vh">
    <n-layout-sider
      bordered
      collapse-mode="width"
      :collapsed="collapsed"
      :width="220"
      :collapsed-width="64"
      show-trigger
      @collapse="collapsed = true"
      @expand="collapsed = false"
    >
      <div style="padding: 16px; text-align: center">
        <n-text strong style="font-size: 16px">
          {{ collapsed ? "Silk" : "Silk 丝路" }}
        </n-text>
      </div>
      <n-menu
        :value="activeKey"
        :collapsed="collapsed"
        :collapsed-width="64"
        :options="menuOptions"
        @update:value="handleMenuSelect"
      />
    </n-layout-sider>

    <n-layout>
      <n-layout-header
        style="
          height: 56px;
          padding: 0 24px;
          display: flex;
          align-items: center;
          justify-content: space-between;
          border-bottom: 1px solid #eee;
          background: #fff;
        "
      >
        <div style="display: flex; align-items: center; gap: 12px">
          <n-badge :value="status?.running ? '运行中' : '已停止'" :type="status?.running ? 'success' : 'default'">
            <n-text style="font-size: 14px; font-weight: 500">
              {{ status?.address || "未启动" }}
            </n-text>
          </n-badge>
        </div>
        <div style="display: flex; gap: 8px">
          <n-button type="success" size="small" @click="handleStart">
            <template #icon><n-icon :component="PlayOutline" /></template>
            启动
          </n-button>
          <n-button type="warning" size="small" @click="handleStop">
            <template #icon><n-icon :component="StopOutline" /></template>
            停止
          </n-button>
          <n-button size="small" @click="handleRestart">
            <template #icon><n-icon :component="RefreshOutline" /></template>
            重启
          </n-button>
        </div>
      </n-layout-header>

      <n-layout-content style="padding: 24px; background: #f5f5f5">
        <router-view />
      </n-layout-content>
    </n-layout>
  </n-layout>
</template>
