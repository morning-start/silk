# Phase 2 - 批次4：界面优化与帮助系统

## 📋 批次概述

**批次目标**：优化主界面布局、创建统一状态提示、优化设置页面、添加帮助系统

**预计工期**：1周（5个工作日）

**依赖关系**：无

---

## 🎯 批次目标

1. 优化主界面布局
2. 创建统一状态提示组件
3. 优化设置页面
4. 添加帮助系统

---

## 📝 任务清单

### 任务4.1：优化主界面布局

**文件路径**：`src/AppContent.vue`

**任务描述**：
重新设计主界面，更符合普通用户习惯

**实现步骤**：

1. 优化主界面布局
```vue
<!-- src/AppContent.vue -->
<template>
  <n-layout has-sider style="height: 100vh">
    <!-- 侧边栏 -->
    <n-layout-sider
      bordered
      :width="240"
      :collapsed-width="64"
      :collapsed="collapsed"
      show-trigger
      @collapse="collapsed = true"
      @expand="collapsed = false"
    >
      <div class="sidebar-header">
        <img src="@/assets/logo.svg" alt="Silk" class="logo" />
        <h1 v-if="!collapsed" class="app-title">Silk</h1>
      </div>
      
      <n-menu
        :collapsed="collapsed"
        :collapsed-width="64"
        :collapsed-icon-size="22"
        :options="menuOptions"
        :value="activeMenu"
        @update:value="handleMenuChange"
      />
      
      <div class="sidebar-footer">
        <n-button 
          quaternary 
          circle 
          @click="toggleTheme"
        >
          <template #icon>
            <n-icon>
              <component :is="isDark ? SunnyOutline : MoonOutline" />
            </n-icon>
          </template>
        </n-button>
        
        <n-button 
          quaternary 
          circle 
          @click="showHelp = true"
        >
          <template #icon>
            <n-icon><HelpOutline /></n-icon>
          </template>
        </n-button>
      </div>
    </n-layout-sider>
    
    <!-- 主内容区 -->
    <n-layout>
      <!-- 顶部状态栏 -->
      <n-layout-header bordered style="height: 64px; padding: 0 24px">
        <div class="header-content">
          <div class="header-left">
            <n-breadcrumb>
              <n-breadcrumb-item>
                <n-icon><HomeOutline /></n-icon>
              </n-breadcrumb-item>
              <n-breadcrumb-item>{{ currentPageTitle }}</n-breadcrumb-item>
            </n-breadcrumb>
          </div>
          
          <div class="header-right">
            <GatewayStatus />
            <n-divider vertical />
            <UserAvatar />
          </div>
        </div>
      </n-layout-header>
      
      <!-- 页面内容 -->
      <n-layout-content content-style="padding: 24px;" :native-scrollbar="false">
        <router-view />
      </n-layout-content>
    </n-layout>
    
    <!-- 帮助系统 -->
    <HelpSystem v-model:show="showHelp" />
  </n-layout>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { 
  NLayout, NLayoutSider, NLayoutHeader, NLayoutContent, 
  NMenu, NButton, NIcon, NBreadcrumb, NBreadcrumbItem, 
  NDivider 
} from 'naive-ui';
import { 
  HomeOutline, MoonOutline, SunnyOutline, HelpOutline,
  GridOutline, ServerOutline, SettingsOutline, 
  AnalyticsOutline, DocumentTextOutline, 
  SwapHorizontalOutline, PersonOutline
} from '@vicons/ionicons5';
import GatewayStatus from './components/GatewayStatus.vue';
import UserAvatar from './components/UserAvatar.vue';
import HelpSystem from './components/HelpSystem.vue';

const router = useRouter();
const route = useRoute();

const collapsed = ref(false);
const isDark = ref(false);
const showHelp = ref(false);

const activeMenu = computed(() => route.path);

const currentPageTitle = computed(() => {
  const titles: Record<string, string> = {
    '/': '仪表盘',
    '/providers': 'AI服务',
    '/model-mappings': '模型映射',
    '/logs': '请求日志',
    '/analytics': '统计分析',
    '/settings': '设置',
  };
  return titles[route.path] || '仪表盘';
});

const menuOptions = [
  {
    label: '仪表盘',
    key: '/',
    icon: renderIcon(HomeOutline),
  },
  {
    label: 'AI服务',
    key: '/providers',
    icon: renderIcon(ServerOutline),
  },
  {
    label: '模型映射',
    key: '/model-mappings',
    icon: renderIcon(SwapHorizontalOutline),
  },
  {
    label: '请求日志',
    key: '/logs',
    icon: renderIcon(DocumentTextOutline),
  },
  {
    label: '统计分析',
    key: '/analytics',
    icon: renderIcon(AnalyticsOutline),
  },
  {
    label: '设置',
    key: '/settings',
    icon: renderIcon(SettingsOutline),
  },
];

function renderIcon(icon: any) {
  return () => h(NIcon, null, { default: () => h(icon) });
}

function handleMenuChange(key: string) {
  router.push(key);
}

function toggleTheme() {
  isDark.value = !isDark.value;
  document.body.classList.toggle('dark', isDark.value);
}
</script>

<style scoped>
.sidebar-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 16px;
  border-bottom: 1px solid #eee;
}

.logo {
  width: 32px;
  height: 32px;
}

.app-title {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
}

.sidebar-footer {
  position: absolute;
  bottom: 20px;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  gap: 8px;
}

.header-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 100%;
}

.header-left {
  display: flex;
  align-items: center;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 16px;
}
</style>
```

2. 创建网关状态组件
```vue
<!-- src/components/GatewayStatus.vue -->
<template>
  <div class="gateway-status">
    <n-tag :type="statusType" size="small">
      <template #icon>
        <n-icon :size="12">
          <component :is="statusIcon" />
        </n-icon>
      </template>
      {{ statusText }}
    </n-tag>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { NTag, NIcon } from 'naive-ui';
import { 
  CheckmarkCircleOutline, 
  CloseCircleOutline, 
  SyncOutline 
} from '@vicons/ionicons5';
import { useGatewayStore } from '../stores/gateway';

const gatewayStore = useGatewayStore();

const statusType = computed(() => {
  if (gatewayStore.status?.running) return 'success';
  return 'default';
});

const statusIcon = computed(() => {
  if (gatewayStore.status?.running) return CheckmarkCircleOutline;
  return CloseCircleOutline;
});

const statusText = computed(() => {
  if (gatewayStore.status?.running) return '网关运行中';
  return '网关已停止';
});
</script>
```

**验收标准**：
- [ ] 主界面布局合理
- [ ] 侧边栏折叠功能正常
- [ ] 主题切换功能正常
- [ ] 网关状态显示正常

---

### 任务4.2：创建统一状态提示组件

**文件路径**：`src/components/StatusIndicator.vue`

**任务描述**：
统一Loading、Success、Error提示样式

**实现步骤**：

1. 创建状态提示组件
```vue
<!-- src/components/StatusIndicator.vue -->
<template>
  <div class="status-indicator" v-if="visible">
    <!-- Loading状态 -->
    <div v-if="status === 'loading'" class="status-loading">
      <n-spin :size="size" />
      <span v-if="message" class="status-message">{{ message }}</span>
    </div>
    
    <!-- Success状态 -->
    <div v-else-if="status === 'success'" class="status-success">
      <n-icon :size="size" color="#18a058">
        <CheckmarkCircleOutline />
      </n-icon>
      <span v-if="message" class="status-message">{{ message }}</span>
    </div>
    
    <!-- Error状态 -->
    <div v-else-if="status === 'error'" class="status-error">
      <n-icon :size="size" color="#d03050">
        <CloseCircleOutline />
      </n-icon>
      <span v-if="message" class="status-message">{{ message }}</span>
    </div>
    
    <!-- Warning状态 -->
    <div v-else-if="status === 'warning'" class="status-warning">
      <n-icon :size="size" color="#f0a020">
        <WarningOutline />
      </n-icon>
      <span v-if="message" class="status-message">{{ message }}</span>
    </div>
    
    <!-- Info状态 -->
    <div v-else-if="status === 'info'" class="status-info">
      <n-icon :size="size" color="#2080f0">
        <InformationCircleOutline />
      </n-icon>
      <span v-if="message" class="status-message">{{ message }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { NSpin, NIcon } from 'naive-ui';
import { 
  CheckmarkCircleOutline, 
  CloseCircleOutline, 
  WarningOutline, 
  InformationCircleOutline 
} from '@vicons/ionicons5';

type Status = 'loading' | 'success' | 'error' | 'warning' | 'info' | 'idle';

const props = withDefaults(defineProps<{
  status: Status;
  message?: string;
  size?: number;
  autoHide?: boolean;
  duration?: number;
}>(), {
  size: 24,
  autoHide: false,
  duration: 3000,
});

const emit = defineEmits<{
  (e: 'hide'): void;
}>();

const visible = computed(() => {
  return props.status !== 'idle';
});

// 自动隐藏
if (props.autoHide && props.status !== 'loading') {
  setTimeout(() => {
    emit('hide');
  }, props.duration);
}
</script>

<style scoped>
.status-indicator {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.status-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #2080f0;
}

.status-success {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #18a058;
}

.status-error {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #d03050;
}

.status-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #f0a020;
}

.status-info {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #2080f0;
}

.status-message {
  font-size: 14px;
}
</style>
```

**验收标准**：
- [ ] 所有状态类型显示正常
- [ ] 图标和颜色正确
- [ ] 自动隐藏功能正常
- [ ] 响应式布局正常

---

### 任务4.3：优化设置页面

**文件路径**：`src/views/SettingsView.vue`

**任务描述**：
简化设置项，添加说明和示例

**实现步骤**：

1. 优化设置页面
```vue
<!-- src/views/SettingsView.vue -->
<template>
  <div class="settings-view">
    <n-card title="设置">
      <n-tabs type="line" animated>
        <!-- 基础设置 -->
        <n-tab-pane name="basic" tab="基础设置">
          <n-form :model="settings" label-placement="left" label-width="120">
            <n-card title="网关设置" size="small">
              <n-form-item label="绑定地址">
                <n-input v-model:value="settings.bindHost" placeholder="127.0.0.1" />
                <template #help>
                  <span class="help-text">网关监听的IP地址，127.0.0.1表示仅本地访问</span>
                </template>
              </n-form-item>
              
              <n-form-item label="端口">
                <n-input-number v-model:value="settings.bindPort" :min="1024" :max="65535" />
                <template #help>
                  <span class="help-text">网关监听的端口号，建议使用1877</span>
                </template>
              </n-form-item>
              
              <n-form-item label="允许远程访问">
                <n-switch v-model:value="settings.allowRemote" />
                <template #help>
                  <span class="help-text">开启后，其他设备可以通过局域网访问网关</span>
                </template>
              </n-form-item>
            </n-card>
            
            <n-card title="启动设置" size="small" style="margin-top: 16px">
              <n-form-item label="开机自启">
                <n-switch v-model:value="settings.launchAtStartup" />
                <template #help>
                  <span class="help-text">系统启动时自动运行Silk</span>
                </template>
              </n-form-item>
              
              <n-form-item label="自动启动网关">
                <n-switch v-model:value="settings.autoStartGateway" />
                <template #help>
                  <span class="help-text">Silk启动时自动启动网关服务</span>
                </template>
              </n-form-item>
            </n-card>
            
            <n-card title="界面设置" size="small" style="margin-top: 16px">
              <n-form-item label="关闭时最小化">
                <n-switch v-model:value="settings.closeToTray" />
                <template #help>
                  <span class="help-text">点击关闭按钮时最小化到系统托盘，而不是退出</span>
                </template>
              </n-form-item>
              
              <n-form-item label="深色模式">
                <n-switch v-model:value="settings.darkMode" />
                <template #help>
                  <span class="help-text">切换深色/浅色主题</span>
                </template>
              </n-form-item>
            </n-card>
          </n-form>
        </n-tab-pane>
        
        <!-- 高级设置 -->
        <n-tab-pane name="advanced" tab="高级设置">
          <n-form :model="advancedSettings" label-placement="left" label-width="120">
            <n-card title="日志设置" size="small">
              <n-form-item label="日志保留天数">
                <n-input-number v-model:value="advancedSettings.logRetentionDays" :min="1" :max="365" />
                <template #help>
                  <span class="help-text">自动清理超过指定天数的日志</span>
                </template>
              </n-form-item>
              
              <n-form-item label="日志级别">
                <n-select
                  v-model:value="advancedSettings.logLevel"
                  :options="logLevelOptions"
                />
                <template #help>
                  <span class="help-text">设置日志记录的详细程度</span>
                </template>
              </n-form-item>
            </n-card>
            
            <n-card title="性能设置" size="small" style="margin-top: 16px">
              <n-form-item label="最大并发请求">
                <n-input-number v-model:value="advancedSettings.maxConcurrentRequests" :min="1" :max="100" />
                <template #help>
                  <span class="help-text">同时处理的最大请求数量</span>
                </template>
              </n-form-item>
              
              <n-form-item label="请求超时（秒）">
                <n-input-number v-model:value="advancedSettings.requestTimeout" :min="5" :max="300" />
                <template #help>
                  <span class="help-text">单个请求的最大处理时间</span>
                </template>
              </n-form-item>
            </n-card>
            
            <n-card title="缓存设置" size="small" style="margin-top: 16px">
              <n-form-item label="启用缓存">
                <n-switch v-model:value="advancedSettings.enableCache" />
                <template #help>
                  <span class="help-text">缓存常用响应，提高访问速度</span>
                </template>
              </n-form-item>
              
              <n-form-item label="缓存大小（MB）">
                <n-input-number v-model:value="advancedSettings.cacheSize" :min="10" :max="1000" />
                <template #help>
                  <span class="help-text">缓存占用的最大内存</span>
                </template>
              </n-form-item>
            </n-card>
          </n-form>
        </n-tab-pane>
        
        <!-- 数据管理 -->
        <n-tab-pane name="data" tab="数据管理">
          <n-space vertical>
            <n-card title="配置备份" size="small">
              <p>备份当前配置，包括AI服务、设置等。</p>
              <n-space>
                <n-button @click="exportConfig">
                  <template #icon>
                    <n-icon><DownloadOutline /></n-icon>
                  </template>
                  导出配置
                </n-button>
                <n-button @click="importConfig">
                  <template #icon>
                    <n-icon><UploadOutline /></n-icon>
                  </template>
                  导入配置
                </n-button>
              </n-space>
            </n-card>
            
            <n-card title="数据库管理" size="small">
              <p>管理本地数据库，包括备份和恢复。</p>
              <n-space>
                <n-button @click="backupDatabase">
                  <template #icon>
                    <n-icon><DownloadOutline /></n-icon>
                  </template>
                  备份数据库
                </n-button>
                <n-button @click="restoreDatabase">
                  <template #icon>
                    <n-icon><UploadOutline /></n-icon>
                  </template>
                  恢复数据库
                </n-button>
                <n-button type="error" @click="clearDatabase">
                  <template #icon>
                    <n-icon><TrashOutline /></n-icon>
                  </template>
                  清空数据库
                </n-button>
              </n-space>
            </n-card>
          </n-space>
        </n-tab-pane>
      </n-tabs>
      
      <template #action>
        <n-space>
          <n-button @click="resetSettings">重置默认</n-button>
          <n-button type="primary" @click="saveSettings">保存设置</n-button>
        </n-space>
      </template>
    </n-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { 
  NCard, NTabs, NTabPane, NForm, NFormItem, NInput, 
  NInputNumber, NSwitch, NSelect, NButton, NSpace, 
  NIcon, useMessage, useDialog 
} from 'naive-ui';
import { DownloadOutline, UploadOutline, TrashOutline } from '@vicons/ionicons5';
import { invoke } from '@tauri-apps/api/core';

const message = useMessage();
const dialog = useDialog();

const settings = ref({
  bindHost: '127.0.0.1',
  bindPort: 1877,
  allowRemote: false,
  launchAtStartup: false,
  autoStartGateway: false,
  closeToTray: true,
  darkMode: false,
});

const advancedSettings = ref({
  logRetentionDays: 30,
  logLevel: 'info',
  maxConcurrentRequests: 10,
  requestTimeout: 30,
  enableCache: true,
  cacheSize: 100,
});

const logLevelOptions = [
  { label: 'DEBUG', value: 'debug' },
  { label: 'INFO', value: 'info' },
  { label: 'WARN', value: 'warn' },
  { label: 'ERROR', value: 'error' },
];

onMounted(async () => {
  await loadSettings();
});

async function loadSettings() {
  try {
    const result = await invoke<any>('get_gateway_settings');
    settings.value = {
      bindHost: result.bind_host,
      bindPort: result.bind_port,
      allowRemote: result.allow_remote,
      launchAtStartup: result.launch_at_startup,
      autoStartGateway: result.auto_start_gateway,
      closeToTray: result.close_to_tray,
      darkMode: result.dark_mode || false,
    };
  } catch (error) {
    console.error('加载设置失败:', error);
  }
}

async function saveSettings() {
  try {
    await invoke('update_gateway_settings', {
      settings: {
        bind_host: settings.value.bindHost,
        bind_port: settings.value.bindPort,
        allow_remote: settings.value.allowRemote,
        launch_at_startup: settings.value.launchAtStartup,
        auto_start_gateway: settings.value.autoStartGateway,
        close_to_tray: settings.value.closeToTray,
        dark_mode: settings.value.darkMode,
      },
    });
    message.success('设置已保存');
  } catch (error: any) {
    message.error(error.message || '保存失败');
  }
}

function resetSettings() {
  dialog.warning({
    title: '重置设置',
    content: '确定要重置所有设置为默认值吗？',
    positiveText: '确定',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await invoke('reset_gateway_settings');
        await loadSettings();
        message.success('设置已重置');
      } catch (error: any) {
        message.error(error.message || '重置失败');
      }
    },
  });
}

async function exportConfig() {
  try {
    const config = await invoke('export_app_config');
    const blob = new Blob([JSON.stringify(config, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'silk_config.json';
    a.click();
    URL.revokeObjectURL(url);
    message.success('配置已导出');
  } catch (error: any) {
    message.error(error.message || '导出失败');
  }
}

async function importConfig() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json';
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    
    try {
      const text = await file.text();
      const config = JSON.parse(text);
      await invoke('import_app_config', { config });
      await loadSettings();
      message.success('配置已导入');
    } catch (error: any) {
      message.error(error.message || '导入失败');
    }
  };
  input.click();
}

async function backupDatabase() {
  try {
    await invoke('backup_database');
    message.success('数据库已备份');
  } catch (error: any) {
    message.error(error.message || '备份失败');
  }
}

async function restoreDatabase() {
  dialog.warning({
    title: '恢复数据库',
    content: '恢复数据库将覆盖当前数据，确定继续吗？',
    positiveText: '确定',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await invoke('restore_database');
        message.success('数据库已恢复');
      } catch (error: any) {
        message.error(error.message || '恢复失败');
      }
    },
  });
}

async function clearDatabase() {
  dialog.error({
    title: '清空数据库',
    content: '清空数据库将删除所有数据，此操作不可恢复，确定继续吗？',
    positiveText: '确定',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await invoke('clear_database');
        message.success('数据库已清空');
      } catch (error: any) {
        message.error(error.message || '清空失败');
      }
    },
  });
}
</script>

<style scoped>
.settings-view {
  padding: 20px;
}

.help-text {
  font-size: 12px;
  color: #999;
}
</style>
```

**验收标准**：
- [ ] 设置页面布局合理
- [ ] 设置项说明清晰
- [ ] 保存功能正常
- [ ] 重置功能正常

---

### 任务4.4：添加帮助系统

**文件路径**：`src/components/HelpSystem.vue`

**任务描述**：
内置帮助文档、常见问题、使用教程

**实现步骤**：

1. 创建帮助系统组件
```vue
<!-- src/components/HelpSystem.vue -->
<template>
  <n-drawer v-model:show="show" :width="500" placement="right">
    <n-drawer-content title="帮助中心">
      <n-tabs type="line" animated>
        <!-- 快速开始 -->
        <n-tab-pane name="quickstart" tab="快速开始">
          <div class="help-section">
            <h3>欢迎使用 Silk</h3>
            <p>Silk 是您的个人AI总机，让您在一个地方访问所有AI服务。</p>
            
            <n-steps :current="currentStep" vertical>
              <n-step title="添加AI服务">
                <p>点击左侧菜单的"AI服务"，然后点击"添加服务"按钮。</p>
                <p>选择您要使用的AI服务（如OpenAI、Claude等），输入API密钥。</p>
              </n-step>
              <n-step title="启动网关">
                <p>在仪表盘页面，点击"启动网关"按钮。</p>
                <p>网关启动后，您可以通过本地地址访问AI服务。</p>
              </n-step>
              <n-step title="开始使用">
                <p>在您的AI客户端中，将API地址设置为Silk的地址。</p>
                <p>例如：http://127.0.0.1:1877/v1/chat/completions</p>
              </n-step>
            </n-steps>
          </div>
        </n-tab-pane>
        
        <!-- 常见问题 -->
        <n-tab-pane name="faq" tab="常见问题">
          <n-collapse>
            <n-collapse-item title="如何获取API密钥？" name="1">
              <p>每个AI服务都有自己的API密钥获取方式：</p>
              <ul>
                <li><strong>OpenAI</strong>：访问 <a href="https://platform.openai.com/api-keys" target="_blank">platform.openai.com</a></li>
                <li><strong>Claude</strong>：访问 <a href="https://console.anthropic.com/api-keys" target="_blank">console.anthropic.com</a></li>
                <li><strong>其他服务</strong>：请参考各服务的官方文档</li>
              </ul>
            </n-collapse-item>
            
            <n-collapse-item title="网关启动失败怎么办？" name="2">
              <p>网关启动失败可能有以下原因：</p>
              <ul>
                <li><strong>端口被占用</strong>：尝试修改端口号，或关闭占用端口的程序</li>
                <li><strong>权限不足</strong>：以管理员身份运行Silk</li>
                <li><strong>防火墙阻止</strong>：检查防火墙设置，允许Silk访问网络</li>
              </ul>
            </n-collapse-item>
            
            <n-collapse-item title="如何查看请求日志？" name="3">
              <p>点击左侧菜单的"请求日志"，可以查看所有AI服务的请求记录。</p>
              <p>您可以按时间、服务、状态等条件过滤日志。</p>
            </n-collapse-item>
            
            <n-collapse-item title="如何备份配置？" name="4">
              <p>在设置页面的"数据管理"标签中，点击"导出配置"按钮。</p>
              <p>配置文件将保存为JSON格式，您可以随时导入恢复。</p>
            </n-collapse-item>
            
            <n-collapse-item title="支持哪些AI服务？" name="5">
              <p>Silk 目前支持以下AI服务：</p>
              <ul>
                <li>OpenAI (ChatGPT)</li>
                <li>Claude</li>
                <li>Google Gemini</li>
                <li>文心一言</li>
                <li>通义千问</li>
                <li>DeepSeek</li>
                <li>以及其他兼容OpenAI API格式的服务</li>
              </ul>
            </n-collapse-item>
          </n-collapse>
        </n-tab-pane>
        
        <!-- 使用教程 -->
        <n-tab-pane name="tutorials" tab="使用教程">
          <n-list bordered>
            <n-list-item>
              <template #prefix>
                <n-icon color="#18a058"><DocumentTextOutline /></n-icon>
              </template>
              <n-thing 
                title="基础配置教程" 
                description="学习如何配置AI服务和网关"
              />
              <template #suffix>
                <n-button text @click="openTutorial('basic')">查看</n-button>
              </template>
            </n-list-item>
            
            <n-list-item>
              <template #prefix>
                <n-icon color="#2080f0"><DocumentTextOutline /></n-icon>
              </template>
              <n-thing 
                title="高级功能教程" 
                description="学习使用模型映射、负载均衡等高级功能"
              />
              <template #suffix>
                <n-button text @click="openTutorial('advanced')">查看</n-button>
              </template>
            </n-list-item>
            
            <n-list-item>
              <template #prefix>
                <n-icon color="#f0a020"><DocumentTextOutline /></n-icon>
              </template>
              <n-thing 
                title="故障排除指南" 
                description="解决常见问题的方法"
              />
              <template #suffix>
                <n-button text @click="openTutorial('troubleshooting')">查看</n-button>
              </template>
            </n-list-item>
          </n-list>
        </n-tab-pane>
        
        <!-- 关于 -->
        <n-tab-pane name="about" tab="关于">
          <div class="about-section">
            <div class="about-logo">
              <img src="@/assets/logo.svg" alt="Silk" />
            </div>
            <h2>Silk</h2>
            <p class="version">版本 {{ version }}</p>
            <p class="description">您的个人AI总机</p>
            
            <n-divider />
            
            <n-space vertical>
              <p><strong>官方网站</strong>：<a href="https://silk.dev" target="_blank">silk.dev</a></p>
              <p><strong>GitHub</strong>：<a href="https://github.com/silk/silk" target="_blank">github.com/silk/silk</a></p>
              <p><strong>文档</strong>：<a href="https://docs.silk.dev" target="_blank">docs.silk.dev</a></p>
            </n-space>
            
            <n-divider />
            
            <p class="copyright">© 2024 Silk. All rights reserved.</p>
          </div>
        </n-tab-pane>
      </n-tabs>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { 
  NDrawer, NDrawerContent, NTabs, NTabPane, NSteps, NStep, 
  NCollapse, NCollapseItem, NList, NListItem, NThing, 
  NButton, NIcon, NDivider, NSpace 
} from 'naive-ui';
import { DocumentTextOutline } from '@vicons/ionicons5';

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void;
}>();

const currentStep = ref(1);
const version = '1.0.0';

function openTutorial(type: string) {
  // 打开教程页面
  window.open(`/tutorials/${type}`, '_blank');
}
</script>

<style scoped>
.help-section {
  padding: 20px 0;
}

.help-section h3 {
  margin-bottom: 16px;
}

.about-section {
  text-align: center;
  padding: 40px 20px;
}

.about-logo img {
  width: 80px;
  height: 80px;
  margin-bottom: 16px;
}

.about-section h2 {
  margin: 0 0 8px 0;
}

.version {
  color: #999;
  margin-bottom: 8px;
}

.description {
  font-size: 16px;
  margin-bottom: 24px;
}

.copyright {
  color: #999;
  font-size: 12px;
}
</style>
```

**验收标准**：
- [ ] 帮助系统功能完整
- [ ] 常见问题内容准确
- [ ] 使用教程清晰易懂
- [ ] 关于页面信息正确

---

## 📦 批次交付物

1. `src/AppContent.vue` - 优化的主界面
2. `src/components/GatewayStatus.vue` - 网关状态组件
3. `src/components/StatusIndicator.vue` - 统一状态提示组件
4. `src/views/SettingsView.vue` - 优化的设置页面
5. `src/components/HelpSystem.vue` - 帮助系统组件

---

## ✅ 批次验收标准

- [ ] 主界面布局合理
- [ ] 状态提示组件功能正常
- [ ] 设置页面功能完整
- [ ] 帮助系统功能完整
- [ ] 所有组件通过测试
- [ ] 代码通过ESLint检查

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务4.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务4.2 | 1天 | 第2天下午 | 第3天 |
| 任务4.3 | 1天 | 第4天 | 第4天 |
| 任务4.4 | 1天 | 第5天 | 第5天 |
