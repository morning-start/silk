<script setup lang="ts">
import { computed, onMounted, ref, onErrorCaptured, type Component } from "vue";
import { useRouter, useRoute } from "vue-router";
import { getVersion } from "@tauri-apps/api/app";
import {
  NLayout,
  NLayoutSider,
  NLayoutHeader,
  NLayoutContent,
  NLayoutFooter,
  NIcon,
  NButton,
  useMessage,
} from "naive-ui";
import {
  PowerOutline,
  ReloadOutline,
  StopOutline,
  GridOutline,
  ServerOutline,
  CubeOutline,
  LayersOutline,
  DocumentTextOutline,
  SettingsOutline,
  InformationCircleOutline,
} from "@vicons/ionicons5";
import { useGatewayStore } from "./stores/gateway";

const { isDark } = defineProps<{ isDark: boolean }>();
const emit = defineEmits<{ "toggle-theme": [] }>();

const router = useRouter();
const route = useRoute();
const gatewayStore = useGatewayStore();
const message = useMessage();

interface NavItem {
  label: string;
  path: string;
  icon: Component;
  /** 仅命中自身路径 */
  exact?: boolean;
  /** 命中路径前缀（如 /providers/xxx） */
  prefix?: boolean;
}

interface NavGroup {
  title?: string;
  items: NavItem[];
}

/** 侧栏导航：分组声明，避免模板里重复 7 段同样的按钮 */
const navGroups: NavGroup[] = [
  {
    items: [
      { label: "仪表盘", path: "/dashboard", exact: true, icon: GridOutline },
    ],
  },
  {
    title: "核心工作流",
    items: [
      { label: "渠道", path: "/providers", prefix: true, icon: ServerOutline },
      { label: "模型", path: "/model-square", exact: true, icon: CubeOutline },
      { label: "预设", path: "/presets", exact: true, icon: LayersOutline },
      { label: "日志", path: "/logs", exact: true, icon: DocumentTextOutline },
    ],
  },
  {
    title: "系统",
    items: [
      { label: "设置", path: "/settings", exact: true, icon: SettingsOutline },
      { label: "关于", path: "/about", exact: true, icon: InformationCircleOutline },
    ],
  },
];

function isActive(item: NavItem) {
  if (item.prefix) return route.path.startsWith(item.path);
  if (item.exact) return route.path === item.path;
  return route.path.startsWith(item.path);
}

function handleNav(path: string) {
  router.push(path);
}

function toggleTheme() {
  emit("toggle-theme");
}

const isRunning = computed(() => gatewayStore.status?.running ?? false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:1877");
const appVersion = ref("");

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

onMounted(async () => {
  gatewayStore.initStatus();
  appVersion.value = await getVersion();
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
    <!-- ============ Sidebar ============ -->
    <NLayoutSider :width="236" :native-scrollbar="false" class="app-sidebar" bordered>
      <div class="sidebar-inner">
        <div class="sidebar-brand">
          <div class="brand-mark"></div>
          <div class="brand-text">
            <span class="brand-name">Silk</span>
            <span class="brand-sub">本地 AI 网关</span>
          </div>
        </div>

        <nav class="sidebar-nav">
          <template v-for="(group, gi) in navGroups" :key="gi">
            <div v-if="group.title" class="nav-section">{{ group.title }}</div>
            <button
              v-for="item in group.items"
              :key="item.path"
              class="nav-item"
              :class="{ active: isActive(item) }"
              @click="handleNav(item.path)"
            >
              <NIcon :size="16" :component="item.icon" />
              <span>{{ item.label }}</span>
            </button>
          </template>
        </nav>

        <div class="sidebar-footer">
          <span class="footer-dot" :class="{ online: isRunning }"></span>
          <span class="footer-text">v{{ appVersion || "0.0.0" }}</span>
        </div>
      </div>
    </NLayoutSider>

    <!-- ============ Main ============ -->
    <NLayout class="main-area">
      <NLayoutHeader bordered class="app-topbar">
        <div class="topbar-inner">
          <div class="topbar-heading">
            <span class="topbar-kicker">SILK / LOCAL GATEWAY</span>
            <span class="topbar-title">{{ route.meta?.title || "仪表盘" }}</span>
          </div>

          <div class="topbar-actions">
            <div class="status-pill" :class="{ online: isRunning }">
              <span class="status-dot"></span>
              <span class="status-addr">{{ bindAddress }}</span>
              <span class="status-sep"></span>
              <span class="status-label">{{ isRunning ? "运行中" : "已停止" }}</span>
            </div>

            <template v-if="isRunning">
              <button class="icon-btn" title="重启网关" @click="restartGateway">
                <NIcon :size="15"><ReloadOutline /></NIcon>
              </button>
              <button class="icon-btn icon-btn--danger" title="停止网关" @click="stopGateway">
                <NIcon :size="15"><StopOutline /></NIcon>
              </button>
            </template>
            <button v-else class="icon-btn icon-btn--go" title="启动网关" @click="startGateway">
              <NIcon :size="15"><PowerOutline /></NIcon>
            </button>

            <button
              class="icon-btn"
              :title="isDark ? '切换浅色' : '切换深色'"
              @click="toggleTheme"
            >
              <NIcon :size="15">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                  <template v-if="isDark">
                    <circle cx="12" cy="12" r="4.2" />
                    <path d="M12 2.5v2M12 19.5v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2.5 12h2M19.5 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
                  </template>
                  <template v-else>
                    <path d="M20.5 13.2A8.5 8.5 0 1 1 10.8 3.5a6.8 6.8 0 0 0 9.7 9.7z" />
                  </template>
                </svg>
              </NIcon>
            </button>
          </div>
        </div>
      </NLayoutHeader>

      <NLayoutContent class="app-content" content-style="padding: 24px 28px;">
        <div v-if="errorInfo" class="error-boundary">
          <div class="error-boundary-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 7.8v5M12 16.2h.01" />
            </svg>
          </div>
          <h3 class="error-boundary-title">页面渲染出错</h3>
          <p class="error-boundary-message">{{ errorInfo.message }}</p>
          <NButton type="primary" size="small" @click="errorInfo = null">重试</NButton>
        </div>

        <router-view v-else v-slot="{ Component }">
          <transition name="page-fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </NLayoutContent>

      <NLayoutFooter bordered class="main-footer">
        <span>Silk v{{ appVersion }}</span>
        <span class="footer-sep">·</span>
        <span>纯本地私有化多模型中转网关</span>
        <span class="footer-sep">·</span>
        <span>数据不出本机</span>
      </NLayoutFooter>
    </NLayout>
  </NLayout>
</template>

<style scoped>
/* ================================================================
   Layout
   ================================================================ */
.app-layout,
.main-area {
  height: 100vh;
}

.main-area :deep(.n-layout-scroll-container) {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

.main-area :deep(.n-layout-header),
.main-area :deep(.n-layout-footer) {
  flex-shrink: 0;
}

.main-area :deep(.n-layout-content) {
  flex: 1;
  min-height: 0;
}

/* ================================================================
   Sidebar — 极简深色侧栏
   ================================================================ */
.app-sidebar {
  background: var(--sidebar-bg) !important;
  border-right: 1px solid var(--sidebar-border) !important;
}

.sidebar-inner {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 52px;
  padding: 0 16px;
  border-bottom: 1px solid var(--sidebar-border);
  flex-shrink: 0;
}

.brand-mark {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: var(--gradient);
  flex-shrink: 0;
  position: relative;
}

.brand-mark::after {
  content: "";
  position: absolute;
  inset: 0;
  margin: auto;
  width: 10px;
  height: 2px;
  border-radius: 1px;
  background: rgba(255, 255, 255, 0.9);
  box-shadow: 0 4px 0 rgba(255, 255, 255, 0.5);
  transform: translateY(-2px);
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
  min-width: 0;
}

.brand-name {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--sidebar-active);
  letter-spacing: -0.01em;
}

.brand-sub {
  font-size: 10px;
  color: var(--sidebar-fg);
  letter-spacing: 0.02em;
}

.sidebar-nav {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px 8px 12px;
}

.nav-section {
  padding: 12px 8px 5px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.3);
  font-family: var(--font-mono);
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 32px;
  padding: 0 10px;
  margin-bottom: 1px;
  border: none;
  border-radius: var(--radius, 8px);
  background: transparent;
  color: var(--sidebar-fg);
  font-family: inherit;
  font-size: 13px;
  font-weight: 500;
  text-align: left;
  cursor: pointer;
  position: relative;
  transition: background-color var(--transition), color var(--transition);
}

.nav-item :deep(.n-icon) {
  color: currentColor;
  opacity: 0.8;
  flex-shrink: 0;
}

.nav-item:hover {
  background: var(--sidebar-hover);
  color: #e5e7eb;
}

.nav-item.active {
  background: var(--sidebar-active-bg);
  color: var(--sidebar-active-fg);
  font-weight: 600;
}

/* 激活态：左侧 2px 实色竖线 */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: -8px;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: var(--sidebar-active-rail);
}

.sidebar-footer {
  display: flex;
  align-items: center;
  gap: 7px;
  height: 36px;
  padding: 0 16px;
  border-top: 1px solid var(--sidebar-border);
  flex-shrink: 0;
}

.footer-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #4b5563;
}

.footer-dot.online {
  background: var(--success);
}

.footer-text {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--sidebar-fg);
}

/* ================================================================
   Topbar — 极简顶栏
   ================================================================ */
.app-topbar {
  height: 52px;
  background: var(--topbar-bg) !important;
  border-bottom: 1px solid var(--topbar-border) !important;
}

.topbar-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 100%;
  padding: 0 20px;
  gap: 16px;
}

.topbar-heading {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.topbar-kicker {
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.08em;
  white-space: nowrap;
}

.topbar-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--topbar-title);
  letter-spacing: -0.01em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

/* 状态胶囊：极简 pill */
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 26px;
  padding: 0 10px;
  margin-right: 2px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  font-size: 11px;
  color: var(--muted);
  font-weight: 500;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--muted);
  flex-shrink: 0;
}

.status-pill.online .status-dot {
  background: var(--success);
}

.status-addr {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--fg-2);
}

.status-sep {
  width: 1px;
  height: 10px;
  background: var(--border);
}

.status-label {
  font-weight: 600;
}

.status-pill.online .status-label {
  color: var(--success);
}

/* ================================================================
   Icon Button — 极简
   ================================================================ */
.icon-btn {
  width: 28px;
  height: 28px;
  display: inline-grid;
  place-items: center;
  border-radius: var(--radius, 8px);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--muted);
  cursor: pointer;
  transition: background-color var(--transition), color var(--transition),
    border-color var(--transition);
}

.icon-btn:hover {
  color: var(--fg);
  background: var(--surface-alt);
  border-color: var(--muted);
}

.icon-btn--danger:hover {
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 30%, var(--border));
  background: color-mix(in srgb, var(--danger) 8%, var(--surface));
}

.icon-btn--go:hover {
  color: var(--success);
  border-color: color-mix(in srgb, var(--success) 30%, var(--border));
  background: color-mix(in srgb, var(--success) 8%, var(--surface));
}

body.dark .icon-btn:hover {
  border-color: #262626;
}

/* ================================================================
   Content & Footer
   ================================================================ */
.app-content {
  background: var(--content-bg);
}

.main-footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 32px;
  padding: 0 20px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--muted);
  background: var(--surface);
  border-top: 1px solid var(--border);
}

.footer-sep {
  opacity: 0.4;
}

/* ================================================================
   Error Boundary
   ================================================================ */
.error-boundary {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 380px;
  gap: 12px;
  padding: 48px 32px;
  text-align: center;
  border-radius: var(--radius-lg, 10px);
  background: var(--card-bg);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-card);
}

.error-boundary-icon {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 10%, var(--surface));
}

.error-boundary-icon svg {
  width: 20px;
  height: 20px;
}

.error-boundary-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--fg);
}

.error-boundary-message {
  font-size: 12.5px;
  color: var(--muted);
  max-width: 420px;
  word-break: break-all;
}

@media (max-width: 680px) {
  .app-topbar {
    height: 48px;
  }

  .topbar-inner {
    padding: 0 14px;
    gap: 8px;
  }

  .topbar-kicker,
  .status-sep,
  .status-label {
    display: none;
  }

  .status-pill {
    gap: 6px;
    padding: 0 8px;
  }

  .status-addr {
    max-width: 128px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .main-footer {
    justify-content: flex-start;
    overflow: hidden;
    white-space: nowrap;
    padding: 0 14px;
  }

  .main-footer span:nth-of-type(n + 2) {
    display: none;
  }
}
</style>
