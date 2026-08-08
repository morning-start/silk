# Phase 2 - 批次2：日志系统与错误处理

## 📋 批次概述

**批次目标**：增强日志可视化，创建用户友好的错误提示组件

**预计工期**：1周（5个工作日）

**依赖关系**：Phase 1 完成（需要错误信息转换层）

---

## 🎯 批次目标

1. 增强日志可视化组件
2. 创建用户友好的错误提示组件
3. 实现错误信息转换服务
4. 创建日志分级显示

---

## 📝 任务清单

### 任务2.1：增强日志可视化组件

**文件路径**：`src/views/LogsView.vue`

**任务描述**：
添加分级显示、搜索、过滤功能

**实现步骤**：

1. 增强日志列表组件
```vue
<!-- src/views/LogsView.vue -->
<template>
  <div class="logs-view">
    <n-card title="请求日志">
      <!-- 工具栏 -->
      <template #header-extra>
        <n-space>
          <n-input 
            v-model:value="searchQuery" 
            placeholder="搜索日志..."
            clearable
            style="width: 200px"
          >
            <template #prefix>
              <n-icon><SearchOutline /></n-icon>
            </template>
          </n-input>
          
          <n-select
            v-model:value="selectedLevel"
            :options="levelOptions"
            placeholder="日志级别"
            style="width: 120px"
            clearable
          />
          
          <n-select
            v-model:value="selectedProvider"
            :options="providerOptions"
            placeholder="AI服务"
            style="width: 150px"
            clearable
          />
          
          <n-date-picker
            v-model:value="dateRange"
            type="daterange"
            style="width: 250px"
          />
          
          <n-button @click="refreshLogs">
            <template #icon>
              <n-icon><RefreshOutline /></n-icon>
            </template>
            刷新
          </n-button>
          
          <n-button @click="exportLogs">
            <template #icon>
              <n-icon><DownloadOutline /></n-icon>
            </template>
            导出
          </n-button>
        </n-space>
      </template>
      
      <!-- 日志统计 -->
      <div class="log-stats">
        <n-statistic label="总请求数" :value="stats.totalRequests" />
        <n-statistic label="成功率" :value="stats.successRate" suffix="%" />
        <n-statistic label="平均响应时间" :value="stats.avgResponseTime" suffix="ms" />
        <n-statistic label="总Token数" :value="stats.totalTokens" />
      </div>
      
      <!-- 日志表格 -->
      <n-data-table
        :columns="columns"
        :data="filteredLogs"
        :pagination="pagination"
        :row-class-name="rowClassName"
        @update:page="handlePageChange"
        @update:page-size="handlePageSizeChange"
      />
    </n-card>
    
    <!-- 日志详情对话框 -->
    <n-modal v-model:show="showDetail" style="width: 800px">
      <n-card title="请求详情">
        <n-descriptions bordered :column="2">
          <n-descriptions-item label="请求ID">
            {{ selectedLog?.request_id }}
          </n-descriptions-item>
          <n-descriptions-item label="时间">
            {{ formatTime(selectedLog?.created_at) }}
          </n-descriptions-item>
          <n-descriptions-item label="AI服务">
            {{ selectedLog?.provider_name }}
          </n-descriptions-item>
          <n-descriptions-item label="模型">
            {{ selectedLog?.model_name }}
          </n-descriptions-item>
          <n-descriptions-item label="状态">
            <n-tag :type="getStatusType(selectedLog?.status_code)">
              {{ selectedLog?.status_code }}
            </n-tag>
          </n-descriptions-item>
          <n-descriptions-item label="响应时间">
            {{ selectedLog?.total_duration_ms }}ms
          </n-descriptions-item>
          <n-descriptions-item label="输入Token">
            {{ selectedLog?.tokens_input }}
          </n-descriptions-item>
          <n-descriptions-item label="输出Token">
            {{ selectedLog?.tokens_output }}
          </n-descriptions-item>
        </n-descriptions>
        
        <n-divider />
        
        <n-collapse>
          <n-collapse-item title="请求内容" name="request">
            <n-code 
              :code="formatJson(selectedLog?.request_body)" 
              language="json"
            />
          </n-collapse-item>
          <n-collapse-item title="响应内容" name="response">
            <n-code 
              :code="formatJson(selectedLog?.response_body)" 
              language="json"
            />
          </n-collapse-item>
          <n-collapse-item title="错误信息" name="error" v-if="selectedLog?.error_message">
            <UserFriendlyError :error="selectedLog?.user_friendly_error" />
          </n-collapse-item>
        </n-collapse>
      </n-card>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { 
  NCard, NSpace, NInput, NSelect, NDatePicker, NButton, 
  NIcon, NDataTable, NStatistic, NModal, NDescriptions, 
  NDescriptionsItem, NTag, NDivider, NCollapse, NCollapseItem, 
  NCode, useMessage 
} from 'naive-ui';
import { 
  SearchOutline, RefreshOutline, DownloadOutline 
} from '@vicons/ionicons5';
import { api, type RequestLog } from '../api';
import UserFriendlyError from '../components/UserFriendlyError.vue';

const message = useMessage();

// 状态
const logs = ref<RequestLog[]>([]);
const loading = ref(false);
const searchQuery = ref('');
const selectedLevel = ref<string | null>(null);
const selectedProvider = ref<string | null>(null);
const dateRange = ref<[number, number] | null>(null);
const showDetail = ref(false);
const selectedLog = ref<RequestLog | null>(null);

// 分页
const pagination = ref({
  page: 1,
  pageSize: 20,
  showSizePicker: true,
  pageSizes: [10, 20, 50, 100],
  onChange: (page: number) => {
    pagination.value.page = page;
  },
  onUpdatePageSize: (pageSize: number) => {
    pagination.value.pageSize = pageSize;
    pagination.value.page = 1;
  },
});

// 统计数据
const stats = computed(() => {
  const total = logs.value.length;
  const success = logs.value.filter(l => l.status_code >= 200 && l.status_code < 300).length;
  const avgTime = total > 0 
    ? Math.round(logs.value.reduce((sum, l) => sum + (l.total_duration_ms || 0), 0) / total)
    : 0;
  const totalTokens = logs.value.reduce((sum, l) => sum + (l.tokens_input || 0) + (l.tokens_output || 0), 0);
  
  return {
    totalRequests: total,
    successRate: total > 0 ? Math.round((success / total) * 100) : 0,
    avgResponseTime: avgTime,
    totalTokens,
  };
});

// 选项
const levelOptions = [
  { label: '成功 (2xx)', value: 'success' },
  { label: '客户端错误 (4xx)', value: 'client_error' },
  { label: '服务器错误 (5xx)', value: 'server_error' },
];

const providerOptions = computed(() => {
  const providers = [...new Set(logs.value.map(l => l.provider_name).filter(Boolean))];
  return providers.map(p => ({ label: p, value: p }));
});

// 表格列定义
const columns = [
  { title: '时间', key: 'created_at', width: 180, render: (row: RequestLog) => formatTime(row.created_at) },
  { title: 'AI服务', key: 'provider_name', width: 120 },
  { title: '模型', key: 'model_name', width: 150 },
  { 
    title: '状态', 
    key: 'status_code', 
    width: 80,
    render: (row: RequestLog) => {
      const type = getStatusType(row.status_code);
      return `<n-tag type="${type}" size="small">${row.status_code}</n-tag>`;
    }
  },
  { title: '响应时间', key: 'total_duration_ms', width: 100, render: (row: RequestLog) => `${row.total_duration_ms || 0}ms` },
  { title: '输入Token', key: 'tokens_input', width: 100 },
  { title: '输出Token', key: 'tokens_output', width: 100 },
  {
    title: '操作',
    key: 'actions',
    width: 80,
    render: (row: RequestLog) => {
      return `<n-button size="small" @click="showLogDetail(row)">详情</n-button>`;
    }
  },
];

// 过滤后的日志
const filteredLogs = computed(() => {
  let result = logs.value;
  
  // 搜索过滤
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase();
    result = result.filter(log => 
      log.request_id?.toLowerCase().includes(query) ||
      log.provider_name?.toLowerCase().includes(query) ||
      log.model_name?.toLowerCase().includes(query) ||
      log.error_message?.toLowerCase().includes(query)
    );
  }
  
  // 级别过滤
  if (selectedLevel.value) {
    result = result.filter(log => {
      const status = log.status_code || 0;
      switch (selectedLevel.value) {
        case 'success': return status >= 200 && status < 300;
        case 'client_error': return status >= 400 && status < 500;
        case 'server_error': return status >= 500;
        default: return true;
      }
    });
  }
  
  // 提供商过滤
  if (selectedProvider.value) {
    result = result.filter(log => log.provider_name === selectedProvider.value);
  }
  
  // 日期过滤
  if (dateRange.value) {
    const [start, end] = dateRange.value;
    result = result.filter(log => {
      const logTime = new Date(log.created_at).getTime();
      return logTime >= start && logTime <= end;
    });
  }
  
  return result;
});

// 方法
async function refreshLogs() {
  loading.value = true;
  try {
    logs.value = await api.listLogs({ limit: 1000 });
  } catch (error: any) {
    message.error(error.message || '加载日志失败');
  } finally {
    loading.value = false;
  }
}

function exportLogs() {
  const csv = convertToCsv(filteredLogs.value);
  downloadCsv(csv, 'silk_logs.csv');
}

function showLogDetail(log: RequestLog) {
  selectedLog.value = log;
  showDetail.value = true;
}

function handlePageChange(page: number) {
  pagination.value.page = page;
}

function handlePageSizeChange(pageSize: number) {
  pagination.value.pageSize = pageSize;
  pagination.value.page = 1;
}

// 工具函数
function formatTime(dateStr: string | undefined): string {
  if (!dateStr) return '-';
  return new Date(dateStr).toLocaleString('zh-CN');
}

function getStatusType(statusCode: number | undefined): 'success' | 'warning' | 'error' {
  if (!statusCode) return 'error';
  if (statusCode >= 200 && statusCode < 300) return 'success';
  if (statusCode >= 400 && statusCode < 500) return 'warning';
  return 'error';
}

function formatJson(data: any): string {
  if (!data) return '{}';
  try {
    return JSON.stringify(data, null, 2);
  } catch {
    return String(data);
  }
}

function rowClassName(row: RequestLog): string {
  const status = row.status_code || 0;
  if (status >= 500) return 'error-row';
  if (status >= 400) return 'warning-row';
  return '';
}

function convertToCsv(logs: RequestLog[]): string {
  const headers = ['时间', 'AI服务', '模型', '状态码', '响应时间', '输入Token', '输出Token'];
  const rows = logs.map(log => [
    formatTime(log.created_at),
    log.provider_name || '',
    log.model_name || '',
    log.status_code || '',
    log.total_duration_ms || 0,
    log.tokens_input || 0,
    log.tokens_output || 0,
  ]);
  
  return [headers, ...rows].map(row => row.join(',')).join('\n');
}

function downloadCsv(csv: string, filename: string) {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
  const link = document.createElement('a');
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  link.click();
}

// 初始化
onMounted(() => {
  refreshLogs();
});
</script>

<style scoped>
.logs-view {
  padding: 20px;
}

.log-stats {
  display: flex;
  gap: 40px;
  margin-bottom: 20px;
  padding: 20px;
  background: #f8f9fa;
  border-radius: 8px;
}

:deep(.error-row) {
  background-color: #fff2f0;
}

:deep(.warning-row) {
  background-color: #fffbe6;
}
</style>
```

**验收标准**：
- [ ] 日志列表正常显示
- [ ] 搜索功能正常
- [ ] 过滤功能正常
- [ ] 分页功能正常
- [ ] 导出功能正常

---

### 任务2.2：创建用户友好的错误提示组件

**文件路径**：`src/components/UserFriendlyError.vue`

**任务描述**：
显示用户友好的错误信息，隐藏技术细节

**实现步骤**：

1. 创建错误提示组件
```vue
<!-- src/components/UserFriendlyError.vue -->
<template>
  <n-alert
    v-if="error"
    :title="error.title"
    :type="alertType"
    :description="error.message"
    closable
    @close="$emit('close')"
  >
    <template #icon>
      <n-icon :size="24">
        <component :is="iconComponent" />
      </n-icon>
    </template>
    
    <template #action>
      <n-space>
        <n-button 
          v-if="error.suggestion" 
          size="small" 
          @click="showSuggestion = true"
        >
          查看建议
        </n-button>
        <n-button 
          size="small" 
          @click="$emit('retry')"
        >
          重试
        </n-button>
      </n-space>
    </template>
    
    <template #default>
      <div class="error-details">
        <p class="error-message">{{ error.message }}</p>
        <p v-if="showSuggestion && error.suggestion" class="error-suggestion">
          <n-icon :size="14"><InformationCircleOutline /></n-icon>
          {{ error.suggestion }}
        </p>
      </div>
    </template>
  </n-alert>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { NAlert, NButton, NSpace, NIcon } from 'naive-ui';
import { 
  AlertCircleOutline, 
  WarningOutline, 
  InformationCircleOutline,
  CheckmarkCircleOutline 
} from '@vicons/ionicons5';

interface UserFriendlyError {
  title: string;
  message: string;
  suggestion?: string;
  error_type: 'Authentication' | 'RateLimit' | 'ServiceUnavailable' | 'BadRequest' | 'Network' | 'Timeout' | 'Unknown';
}

const props = defineProps<{
  error: UserFriendlyError | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'retry'): void;
}>();

const showSuggestion = ref(false);

const alertType = computed(() => {
  if (!props.error) return 'error';
  switch (props.error.error_type) {
    case 'Authentication':
    case 'RateLimit':
      return 'warning';
    case 'ServiceUnavailable':
    case 'Timeout':
      return 'error';
    case 'BadRequest':
      return 'info';
    default:
      return 'error';
  }
});

const iconComponent = computed(() => {
  if (!props.error) return AlertCircleOutline;
  switch (props.error.error_type) {
    case 'Authentication':
    case 'RateLimit':
      return WarningOutline;
    case 'ServiceUnavailable':
    case 'Timeout':
      return AlertCircleOutline;
    case 'BadRequest':
      return InformationCircleOutline;
    default:
      return AlertCircleOutline;
  }
});
</script>

<style scoped>
.error-details {
  margin-top: 8px;
}

.error-message {
  margin: 0 0 8px 0;
  font-size: 14px;
}

.error-suggestion {
  margin: 0;
  font-size: 12px;
  color: #666;
  display: flex;
  align-items: center;
  gap: 4px;
}
</style>
```

**验收标准**：
- [ ] 错误提示正常显示
- [ ] 不同错误类型样式正确
- [ ] 建议信息显示正常
- [ ] 关闭和重试功能正常

---

### 任务2.3：实现错误信息转换服务

**文件路径**：`src/services/errorConverter.ts`

**任务描述**：
前端调用后端错误转换API

**实现步骤**：

1. 创建错误转换服务
```typescript
// src/services/errorConverter.ts

import { invoke } from '@tauri-apps/api/core';

export interface UserFriendlyError {
  title: string;
  message: string;
  suggestion?: string;
  error_type: 'Authentication' | 'RateLimit' | 'ServiceUnavailable' | 'BadRequest' | 'Network' | 'Timeout' | 'Unknown';
}

export interface GatewayError {
  type: string;
  message: string;
  status?: number;
  details?: any;
}

export class ErrorConverterService {
  /**
   * 将技术错误转换为用户友好信息
   */
  static async convertError(error: GatewayError): Promise<UserFriendlyError> {
    try {
      // 调用后端错误转换API
      const result = await invoke<UserFriendlyError>('convert_error_to_user_friendly', {
        error: error,
      });
      return result;
    } catch (e) {
      // 如果后端调用失败，使用前端默认转换
      return this.convertErrorLocally(error);
    }
  }
  
  /**
   * 本地错误转换（当后端不可用时使用）
   */
  private static convertErrorLocally(error: GatewayError): UserFriendlyError {
    const status = error.status || 0;
    
    if (status === 401 || status === 403) {
      return {
        title: '认证失败',
        message: 'AI服务认证失败，请检查您的API密钥是否正确',
        suggestion: '请在设置中重新配置API密钥',
        error_type: 'Authentication',
      };
    }
    
    if (status === 429) {
      return {
        title: '请求过于频繁',
        message: 'AI服务请求过于频繁，请稍后再试',
        suggestion: '您可以尝试减少请求频率或等待一段时间',
        error_type: 'RateLimit',
      };
    }
    
    if (status >= 500) {
      return {
        title: '服务暂时不可用',
        message: 'AI服务暂时不可用，请稍后再试',
        suggestion: '请稍后重试，或联系服务提供商',
        error_type: 'ServiceUnavailable',
      };
    }
    
    if (error.type === 'timeout') {
      return {
        title: '请求超时',
        message: '请求处理超时，请稍后再试',
        suggestion: '请检查网络连接，或稍后重试',
        error_type: 'Timeout',
      };
    }
    
    if (error.type === 'network') {
      return {
        title: '网络错误',
        message: '网络连接出现问题，请检查您的网络',
        suggestion: '请检查网络连接后重试',
        error_type: 'Network',
      };
    }
    
    return {
      title: '请求失败',
      message: 'AI服务出现问题，请稍后再试',
      suggestion: '如问题持续，请联系技术支持',
      error_type: 'Unknown',
    };
  }
  
  /**
   * 从异常对象中提取错误信息
   */
  static extractErrorFromException(exception: any): GatewayError {
    if (typeof exception === 'string') {
      return { type: 'unknown', message: exception };
    }
    
    if (exception.message) {
      return {
        type: exception.type || 'unknown',
        message: exception.message,
        status: exception.status,
        details: exception.details,
      };
    }
    
    return { type: 'unknown', message: '未知错误' };
  }
}
```

2. 在组件中使用错误转换服务
```typescript
// 示例：在组件中使用
import { ErrorConverterService } from '@/services/errorConverter';

async function handleApiError(error: any) {
  const gatewayError = ErrorConverterService.extractErrorFromException(error);
  const userFriendlyError = await ErrorConverterService.convertError(gatewayError);
  
  // 显示用户友好的错误信息
  showErrorDialog(userFriendlyError);
}
```

**验收标准**：
- [ ] 错误转换服务功能完整
- [ ] 支持所有错误类型
- [ ] 本地转换逻辑正确
- [ ] 后端API调用正常

---

### 任务2.4：创建日志分级显示

**文件路径**：`src/components/LogViewer.vue`

**任务描述**：
区分技术日志和用户日志，分级显示

**实现步骤**：

1. 创建日志查看器组件
```vue
<!-- src/components/LogViewer.vue -->
<template>
  <div class="log-viewer">
    <n-card title="系统日志">
      <template #header-extra>
        <n-space>
          <n-select
            v-model:value="selectedLevel"
            :options="levelOptions"
            placeholder="日志级别"
            style="width: 120px"
            clearable
          />
          <n-switch v-model:value="autoScroll">
            <template #checked>自动滚动</template>
            <template #unchecked>手动滚动</template>
          </n-switch>
          <n-button @click="clearLogs">清空</n-button>
        </n-space>
      </template>
      
      <div class="log-container" ref="logContainer">
        <div 
          v-for="(log, index) in filteredLogs" 
          :key="index"
          class="log-entry"
          :class="log.level"
        >
          <span class="log-time">{{ formatTime(log.timestamp) }}</span>
          <n-tag :type="getLevelType(log.level)" size="small">
            {{ log.level.toUpperCase() }}
          </n-tag>
          <span class="log-source">{{ log.source }}</span>
          <span class="log-message">{{ log.message }}</span>
        </div>
      </div>
    </n-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { NCard, NSpace, NSelect, NSwitch, NButton, NTag } from 'naive-ui';

interface LogEntry {
  timestamp: string;
  level: 'debug' | 'info' | 'warn' | 'error';
  source: string;
  message: string;
  userFriendly?: boolean;
}

const props = defineProps<{
  logs: LogEntry[];
}>();

const selectedLevel = ref<string | null>(null);
const autoScroll = ref(true);
const logContainer = ref<HTMLElement | null>(null);

const levelOptions = [
  { label: 'DEBUG', value: 'debug' },
  { label: 'INFO', value: 'info' },
  { label: 'WARN', value: 'warn' },
  { label: 'ERROR', value: 'error' },
];

const filteredLogs = computed(() => {
  if (!selectedLevel.value) {
    return props.logs;
  }
  return props.logs.filter(log => log.level === selectedLevel.value);
});

watch(filteredLogs, () => {
  if (autoScroll.value) {
    nextTick(() => {
      if (logContainer.value) {
        logContainer.value.scrollTop = logContainer.value.scrollHeight;
      }
    });
  }
});

function formatTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString('zh-CN');
}

function getLevelType(level: string): 'success' | 'info' | 'warning' | 'error' {
  switch (level) {
    case 'debug':
      return 'info';
    case 'info':
      return 'success';
    case 'warn':
      return 'warning';
    case 'error':
      return 'error';
    default:
      return 'info';
  }
}

function clearLogs() {
  // 清空日志
}
</script>

<style scoped>
.log-viewer {
  height: 100%;
}

.log-container {
  height: 400px;
  overflow-y: auto;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 10px;
  border-radius: 4px;
}

.log-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid #333;
}

.log-entry.debug {
  color: #888;
}

.log-entry.info {
  color: #4fc1ff;
}

.log-entry.warn {
  color: #cca700;
}

.log-entry.error {
  color: #f44747;
}

.log-time {
  color: #888;
  min-width: 80px;
}

.log-source {
  color: #569cd6;
  min-width: 100px;
}

.log-message {
  flex: 1;
  word-break: break-all;
}
</style>
```

**验收标准**：
- [ ] 日志分级显示正常
- [ ] 过滤功能正常
- [ ] 自动滚动功能正常
- [ ] 样式显示正确

---

## 📦 批次交付物

1. `src/views/LogsView.vue` - 增强的日志视图
2. `src/components/UserFriendlyError.vue` - 用户友好的错误提示组件
3. `src/services/errorConverter.ts` - 错误信息转换服务
4. `src/components/LogViewer.vue` - 日志查看器组件

---

## ✅ 批次验收标准

- [ ] 日志可视化增强功能完整
- [ ] 错误提示组件功能正常
- [ ] 错误转换服务功能完整
- [ ] 日志分级显示功能正常
- [ ] 所有组件通过测试
- [ ] 代码通过ESLint检查

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务2.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务2.2 | 1天 | 第2天下午 | 第3天 |
| 任务2.3 | 1天 | 第4天 | 第4天 |
| 任务2.4 | 1天 | 第5天 | 第5天 |
