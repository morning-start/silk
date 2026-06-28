<script setup lang="ts">
import { computed, h, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import {
  NLayout,
  NLayoutSider,
  NLayoutHeader,
  NLayoutContent,
  NMenu,
  NButton,
  NSpace,
  NIcon,
  useMessage,
} from "naive-ui";
import {
  GridOutline,
  PeopleOutline,
  CubeOutline,
  LayersOutline,
  GitBranchOutline,
  DocumentTextOutline,
  SettingsOutline,
  CodeSlashOutline,
  StatsChartOutline,
  AnalyticsOutline,
  MoonOutline,
  SunnyOutline,
  PowerOutline,
  ReloadOutline,
  StopOutline,
} from "@vicons/ionicons5";
import { useGatewayStore } from "./stores/gateway";

const props = defineProps<{ isDark: boolean }>();
const emit = defineEmits<{ "toggle-theme": [] }>();

const router = useRouter();
const route = useRoute();
const gatewayStore = useGatewayStore();
const message = useMessage();

const menuOptions = [
  { label: "仪表盘", key: "/dashboard", icon: () => h(NIcon, null, { default: () => h(GridOutline) }) },
  { type: "divider" as const, key: "d1" },
  { label: "渠道管理", key: "services", type: "group" as const, children: [
    { label: "渠道管理", key: "/providers", icon: () => h(NIcon, null, { default: () => h(PeopleOutline) }) },
    { label: "模型池", key: "/model-square", icon: () => h(NIcon, null, { default: () => h(CubeOutline) }) },
    { label: "分组管理", key: "/groups", icon: () => h(NIcon, null, { default: () => h(LayersOutline) }) },
    { label: "路由规则", key: "/routing-rules", icon: () => h(NIcon, null, { default: () => h(GitBranchOutline) }) },
  ]},
  { type: "divider" as const, key: "d2" },
  { label: "监控与日志", key: "monitoring", type: "group" as const, children: [
    { label: "请求日志", key: "/logs", icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) }) },
    { label: "实时监控", key: "/monitoring", icon: () => h(NIcon, null, { default: () => h(StatsChartOutline) }) },
    { label: "用量分析", key: "/analytics", icon: () => h(NIcon, null, { default: () => h(AnalyticsOutline) }) },
  ]},
  { type: "divider" as const, key: "d3" },
  { label: "系统", key: "system", type: "group" as const, children: [
    { label: "系统设置", key: "/settings", icon: () => h(NIcon, null, { default: () => h(SettingsOutline) }) },
    { label: "API 调试", key: "/debugger", icon: () => h(NIcon, null, { default: () => h(CodeSlashOutline) }) },
  ]},
];

const activeKey = computed(() => route.path);

function handleMenuUpdate(key: string) {
  router.push(key);
}

function toggleTheme() {
  emit("toggle-theme");
}

const isRunning = computed(() => gatewayStore.status?.running ?? false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:2013");

async function startGateway() {
  try {
    await gatewayStore.start();
    message.success("网关已启动");
  } catch {
    message.error("启动失败");
  }
}

async function stopGateway() {
  try {
    await gatewayStore.stop();
    message.success("网关已停止");
  } catch {
    message.error("停止失败");
  }
}

async function restartGateway() {
  try {
    await gatewayStore.restart();
    message.success("网关已重启");
  } catch {
    message.error("重启失败");
  }
}

onMounted(() => {
  gatewayStore.initStatus();
});
</script>

<template>
  <NLayout class="app-layout" has-sider>
    <!-- Sidebar -->
    <NLayoutSider
      bordered
      :width="220"
      :native-scrollbar="false"
      class="app-sidebar"
    >
      <div class="sidebar-brand">
        <div class="sidebar-logo">
          <span class="logo-dot"></span>
          <span class="logo-text">Silk</span>
          <span class="logo-sub">Gateway</span>
        </div>
        <div class="sidebar-version">v0.1.0 · 多模型中转网关</div>
      </div>

      <div class="sidebar-menu-wrap">
        <NMenu
          :value="activeKey"
          :options="menuOptions"
          @update:value="handleMenuUpdate"
          :indent="18"
          class="sidebar-menu"
        />
      </div>

      <div class="sidebar-footer">
        <div class="sidebar-footer-info">
          <span class="sidebar-footer-label">gateway.silk.io</span>
          <span class="sidebar-footer-addr">{{ bindAddress }}</span>
        </div>
      </div>
    </NLayoutSider>

    <!-- Main Content -->
    <NLayout>
      <!-- Topbar -->
      <NLayoutHeader bordered class="app-topbar">
        <div class="topbar-inner">
          <div class="topbar-title">
            <span class="topbar-title-text">{{ route.meta?.title || 'Silk Gateway' }}</span>
          </div>
          <div class="topbar-actions">
            <NSpace :size="12" align="center">
              <template v-if="gatewayStore.loading && !gatewayStore.status">
                <div class="gateway-status-indicator">
                  <span class="status-dot loading"></span>
                  <span class="status-text">检测中...</span>
                </div>
              </template>
              <template v-else>
                <div class="gateway-status-indicator">
                  <span class="status-dot" :class="{ running: isRunning }"></span>
                  <span class="status-text" :class="{ running: isRunning }">
                    {{ isRunning ? '运行中' : '已停止' }}
                  </span>
                  <span class="status-addr" v-if="isRunning">· {{ bindAddress }}</span>
                </div>

                <template v-if="isRunning">
                  <NButton quaternary size="small" @click="restartGateway" title="重启网关">
                    <template #icon>
                      <NIcon><ReloadOutline /></NIcon>
                    </template>
                  </NButton>
                  <NButton quaternary size="small" type="error" @click="stopGateway" title="停止网关">
                    <template #icon>
                      <NIcon><StopOutline /></NIcon>
                    </template>
                  </NButton>
                </template>
                <template v-else>
                  <NButton quaternary size="small" type="success" @click="startGateway" title="启动网关">
                    <template #icon>
                      <NIcon><PowerOutline /></NIcon>
                    </template>
                  </NButton>
                </template>
              </template>

              <NButton quaternary size="small" @click="toggleTheme" :title="isDark ? '切换浅色' : '切换深色'">
                <template #icon>
                  <NIcon>
                    <SunnyOutline v-if="props.isDark" />
                    <MoonOutline v-else />
                  </NIcon>
                </template>
              </NButton>
            </NSpace>
          </div>
        </div>
      </NLayoutHeader>

      <!-- Content -->
      <NLayoutContent content-style="padding: 24px;" class="app-content">
        <router-view />
      </NLayoutContent>
    </NLayout>
  </NLayout>
</template>

<style scoped>
.app-layout {
  height: 100vh;
}

.app-sidebar {
  background: var(--sidebar-bg, #1e293b) !important;
  display: flex !important;
  flex-direction: column !important;
  height: 100vh !important;
}

.sidebar-brand {
  padding: 20px 16px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.sidebar-logo {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.logo-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: linear-gradient(135deg, #6366f1, #8b5cf6);
  flex-shrink: 0;
}

.logo-text {
  font-size: 20px;
  font-weight: 700;
  color: #f1f5f9;
  letter-spacing: -0.02em;
}

.logo-sub {
  font-size: 14px;
  font-weight: 500;
  color: #94a3b8;
  margin-left: 2px;
}

.sidebar-version {
  font-size: 11px;
  color: #64748b;
  margin-left: 18px;
}

.sidebar-menu-wrap {
  flex: 1;
  overflow-y: auto;
  padding-bottom: 60px;
}

.sidebar-menu {
  margin-top: 4px;
}

/* 侧边栏深色背景 - 菜单字体颜色 */
.sidebar-menu :deep(.n-menu-item-content) {
  color: #cbd5e1;
}

.sidebar-menu :deep(.n-menu-item-content--selected) {
  color: #ffffff;
  background: rgba(99, 102, 241, 0.15) !important;
}

.sidebar-menu :deep(.n-menu-item-content--selected) .n-menu-item-content__icon {
  color: #818cf8;
}

.sidebar-menu :deep(.n-menu-item-content:hover) {
  color: #f1f5f9;
  background: rgba(255, 255, 255, 0.06) !important;
}

.sidebar-menu :deep(.n-menu-item-content__icon) {
  color: #64748b;
}

.sidebar-menu :deep(.n-menu-item-content--selected .n-menu-item-content__arrow) {
  color: #818cf8;
}

.sidebar-menu :deep(.n-menu-item-content__arrow) {
  color: #475569;
}

/* 分组标题 */
.sidebar-menu :deep(.n-menu-item-group-header) {
  color: #475569 !important;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 8px 16px 4px;
}

/* 分割线 */
.sidebar-menu :deep(.n-menu-divider) {
  background-color: rgba(255, 255, 255, 0.08) !important;
}

.sidebar-footer {
  padding: 12px 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
  background: var(--sidebar-bg, #1e293b);
}

.sidebar-footer-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sidebar-footer-label {
  font-size: 11px;
  font-weight: 600;
  color: #475569;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.sidebar-footer-addr {
  font-size: 12px;
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  color: #64748b;
}

/* Topbar */
.app-topbar {
  background: var(--topbar-bg, #ffffff) !important;
  border-bottom: 1px solid var(--topbar-border, #e2e8f0);
}

.topbar-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  height: 56px;
}

.topbar-title-text {
  font-size: 16px;
  font-weight: 600;
  color: var(--topbar-title, #1e293b);
}

.topbar-actions {
  display: flex;
  align-items: center;
}

.gateway-status-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  border-radius: 20px;
  background: var(--status-bg, #f1f5f9);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #94a3b8;
  transition: all 0.3s;
}

.status-dot.loading {
  background: #fbbf24;
  animation: pulse-dot 1.2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.75); }
}

.status-dot.running {
  background: #22c55e;
  box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.2);
}

.status-text {
  font-size: 13px;
  font-weight: 500;
  color: #94a3b8;
}

.status-text.running {
  color: #22c55e;
}

.status-addr {
  font-size: 12px;
  color: #64748b;
  font-family: 'JetBrains Mono', 'Consolas', monospace;
}

/* Content area */
.app-content {
  background: var(--content-bg, #f8fafc);
  min-height: calc(100vh - 56px);
  overflow-y: auto;
}
</style>