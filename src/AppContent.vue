<script setup lang="ts">
import { computed, onMounted, ref, onErrorCaptured } from "vue";
import { useRouter, useRoute } from "vue-router";
import {
  NLayout,
  NLayoutSider,
  NLayoutHeader,
  NLayoutContent,
  NIcon,
  useMessage,
} from "naive-ui";
import {
  PowerOutline,
  ReloadOutline,
  StopOutline,
} from "@vicons/ionicons5";
import { useGatewayStore } from "./stores/gateway";

const { isDark } = defineProps<{ isDark: boolean }>();
const emit = defineEmits<{ "toggle-theme": [] }>();

const router = useRouter();
const route = useRoute();
const gatewayStore = useGatewayStore();
const message = useMessage();

function handleNav(path: string) {
  router.push(path);
}

function toggleTheme() {
  emit("toggle-theme");
}

const isRunning = computed(() => gatewayStore.status?.running ?? false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:1877");

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

// 全局错误边界：捕获子组件渲染错误，显示降级 UI
const errorInfo = ref<{ message: string; stack?: string } | null>(null);
onErrorCaptured((err, _instance, info) => {
  errorInfo.value = {
    message: err instanceof Error ? err.message : String(err),
    stack: err instanceof Error ? err.stack : undefined,
  };
  console.error(`[ErrorBoundary] ${info}:`, err);
  return false; // 阻止错误继续向上传播
});
</script>

<template>
  <NLayout class="app-layout" has-sider>
    <!-- Sidebar -->
    <NLayoutSider
      :width="240"
      :native-scrollbar="false"
      class="app-sidebar"
    >
      <div class="sidebar-brand">
        <h1><span class="logo-dot"></span> Silk Gateway</h1>
        <p>本地 AI 网关控制台</p>
      </div>

      <div class="sidebar-menu-wrap">
        <nav class="sidebar-nav">
          <button :class="{ active: route.path === '/dashboard' }" @click="handleNav('/dashboard')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>仪表盘
          </button>
          <div class="sidebar-section">核心工作流</div>
          <button :class="{ active: route.path.startsWith('/providers') }" @click="handleNav('/providers')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>渠道
          </button>
          <button :class="{ active: route.path === '/model-square' }" @click="handleNav('/model-square')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>模型
          </button>
          <button :class="{ active: route.path === '/agent-profiles' }" @click="handleNav('/agent-profiles')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>预设
          </button>
          <button :class="{ active: route.path === '/logs' }" @click="handleNav('/logs')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M16 13H8M16 17H8M10 9H8"/></svg>日志
          </button>
          <div class="sidebar-section">系统</div>
          <button :class="{ active: route.path === '/settings' }" @click="handleNav('/settings')">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>设置
          </button>
        </nav>
      </div>

      <div class="sidebar-footer">gateway.silk.io · v1.0.0</div>
    </NLayoutSider>

    <!-- Main Content -->
    <NLayout class="main-area">
      <!-- Topbar -->
      <NLayoutHeader bordered class="app-topbar">
        <div class="topbar-inner">
          <div class="topbar-title">
            <span class="topbar-title-text">{{ route.meta?.title || '仪表盘' }}</span>
          </div>
          <div class="topbar-actions">
            <span class="status-dot" :class="{ running: isRunning }"></span>
            <span class="status-text">
              gateway://{{ bindAddress }}
              <template v-if="isRunning"> · 正常接收流量</template>
              <template v-else> · 已停止</template>
            </span>
            <div class="listen-pill">
              <span class="status-dot-sm" :class="{ online: isRunning }"></span>
              <span class="listen-addr">{{ bindAddress }}</span>
            </div>
            <template v-if="isRunning">
              <button class="topbar-btn" @click="restartGateway" title="重启网关">
                <NIcon size="16"><ReloadOutline /></NIcon>
              </button>
              <button class="topbar-btn topbar-btn-danger" @click="stopGateway" title="停用">
                <NIcon size="16"><StopOutline /></NIcon>
              </button>
            </template>
            <template v-else>
              <button class="topbar-btn" @click="startGateway" title="启动网关">
                <NIcon size="16"><PowerOutline /></NIcon>
              </button>
            </template>
            <button class="theme-toggle" @click="toggleTheme" :title="isDark ? '切换浅色' : '切换深色'">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" style="width:16px;height:16px">
                <template v-if="isDark">
                  <circle cx="12" cy="12" r="5"/>
                  <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
                </template>
                <template v-else>
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
                </template>
              </svg>
            </button>
          </div>
        </div>
      </NLayoutHeader>

      <!-- Content -->
      <NLayoutContent content-style="padding: 28px;" class="app-content">
        <template v-if="errorInfo">
          <div class="error-boundary">
            <div class="error-boundary-icon">⚠️</div>
            <h3 class="error-boundary-title">页面渲染出错</h3>
            <p class="error-boundary-message">{{ errorInfo.message }}</p>
            <NButton type="primary" @click="errorInfo = null">重试</NButton>
          </div>
        </template>
        <template v-else>
          <router-view />
        </template>
      </NLayoutContent>

      <!-- Main Footer -->
      <footer class="main-footer">Silk Gateway v1.0.0 · 纯本地私有化多模型中转网关 · 零云端上传数据</footer>
    </NLayout>
  </NLayout>
</template>

<style scoped>
.app-layout {
  height: 100vh;
}

/* 主区域 */
.main-area {
  height: 100%;
}

/* ===== Sidebar ===== */
.app-sidebar {
  background: var(--sidebar-bg, #0f172a) !important;
  display: flex !important;
  flex-direction: column !important;
  height: 100vh !important;
  border-right: 1px solid var(--border-soft, #e2e8f0) !important;
}

/* Brand — align with design spec */
.sidebar-brand {
  padding: 24px 20px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.sidebar-brand h1 {
  font-size: 16px;
  font-weight: 600;
  color: var(--sidebar-active, #f8fafc);
  letter-spacing: -0.02em;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
}

.sidebar-brand h1 .logo-dot {
  width: 10px;
  height: 10px;
  background: var(--accent, #0891b2);
  border-radius: 3px;
  flex-shrink: 0;
}

.sidebar-brand p {
  font-size: 11px;
  color: var(--sidebar-fg, #94a3b8);
  margin: 4px 0 0 0;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  opacity: 0.8;
}

.sidebar-menu-wrap {
  flex: 1;
  overflow-y: auto;
}

/* ===== Sidebar Nav — design spec ===== */
.sidebar-nav {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sidebar-nav button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 12px;
  border-radius: var(--radius, 8px);
  font-size: 13px;
  font-weight: 400;
  color: var(--sidebar-fg, #94a3b8);
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  transition: all 150ms ease;
  font-family: inherit;
}

.sidebar-nav button:hover {
  background: var(--sidebar-hover, rgba(255, 255, 255, 0.05));
  color: var(--sidebar-active, #f8fafc);
}

.sidebar-nav button.active {
  background: var(--accent, #0891b2);
  color: #ffffff;
}

.sidebar-nav button svg {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.sidebar-nav .sidebar-section {
  padding: 16px 12px 8px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: rgba(255, 255, 255, 0.25);
  margin-top: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

/* Sidebar footer — design spec style */
.sidebar-footer {
  padding: 12px 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  color: var(--sidebar-fg, #94a3b8);
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  opacity: 0.7;
  flex-shrink: 0;
  background: var(--sidebar-bg, #0f172a);
}

/* ===== Topbar — design spec ===== */
.app-topbar {
  background: rgba(255, 255, 255, 0.88) !important;
  backdrop-filter: blur(14px) !important;
  border-bottom: 1px solid var(--border-soft, #e2e8f0) !important;
}

.topbar-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 28px;
  height: 64px;
}

.topbar-title-text {
  font-size: 15px;
  font-weight: 600;
  color: var(--topbar-title, #0f172a);
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* Status dot + text */
.topbar-actions .status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--muted, #94a3b8);
  transition: all 0.3s;
}

.topbar-actions .status-dot.running {
  background: var(--success, #10b981);
  animation: pulse-dot 2s infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.topbar-actions .status-text {
  font-size: 12px;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  color: var(--muted, #64748b);
}

/* Listen pill — design spec style */
.listen-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border: 1px solid var(--border-soft, #e2e8f0);
  border-radius: var(--radius, 8px);
  background: var(--surface, #ffffff);
  font-size: 11px;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  color: var(--muted, #64748b);
}

.status-dot-sm {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
  background: var(--muted, #94a3b8);
}

.status-dot-sm.online {
  background: var(--success, #10b981);
}

.listen-addr {
  font-family: inherit;
}

/* Topbar action buttons */
.topbar-btn {
  width: 32px;
  height: 32px;
  display: inline-grid;
  place-items: center;
  border-radius: var(--radius, 8px);
  border: 1px solid var(--border-soft, #e2e8f0);
  background: var(--surface, #ffffff);
  color: var(--muted, #64748b);
  cursor: pointer;
  transition: all 150ms ease;
}

.topbar-btn:hover {
  color: var(--fg, #0f172a);
  border-color: var(--border, #cbd5e1);
  background: var(--surface-alt, #f1f5f9);
}

.topbar-btn-danger:hover {
  color: var(--danger, #ef4444);
  border-color: var(--danger, #ef4444);
}

/* Theme toggle — design spec */
.theme-toggle {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: var(--radius, 8px);
  border: 1px solid var(--border-soft, #e2e8f0);
  background: var(--surface, #ffffff);
  color: var(--muted, #64748b);
  cursor: pointer;
  transition: all 150ms ease;
}

.theme-toggle:hover {
  border-color: var(--accent, #0891b2);
  color: var(--accent, #0891b2);
}

/* ===== Content ===== */
.app-content {
  background: var(--content-bg, #f8fafc);
}

/* ===== Main Footer ===== */
.main-footer {
  padding: 16px 28px;
  border-top: 1px solid var(--border-soft, #e2e8f0);
  font-size: 11px;
  color: var(--muted, #64748b);
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  text-align: center;
  background: var(--surface, #ffffff);
}

/* ===== Error Boundary ===== */
.error-boundary {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 400px;
  text-align: center;
  gap: 12px;
}

.error-boundary-icon {
  font-size: 48px;
}

.error-boundary-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--fg, #0f172a);
  margin: 0;
}

.error-boundary-message {
  font-size: 13px;
  color: var(--muted, #64748b);
  max-width: 400px;
  word-break: break-all;
  margin: 0;
}
</style>
