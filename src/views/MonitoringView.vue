<script setup lang="ts">
import { ref, onMounted, h } from "vue";
import {
  NCard,
  NGrid,
  NGi,
  NButton,
  NDataTable,
  NText,
  NSpace,
  NSpin,
  type DataTableColumns,
} from "naive-ui";
import { api, type HourlyStats, type ProviderStats } from "../api";

const loading = ref(false);
const error = ref<string | null>(null);

const hourlyData = ref<HourlyStats[]>([]);
const providerStats = ref<ProviderStats[]>([]);

const timeRange = ref(24);

const providerColumns: DataTableColumns<ProviderStats> = [
  { title: "渠道", key: "provider_name" },
  {
    title: "请求占比",
    key: "request_count",
    render(row) {
      const total = providerStats.value.reduce((s, p) => s + p.request_count, 0);
      const pct = total > 0 ? ((row.request_count / total) * 100).toFixed(1) : "0";
      return h("span", { class: "num" }, `${pct}%`);
    },
  },
  { title: "请求数", key: "request_count" },
  { title: "平均响应", key: "avg_duration_ms", render(row) { return h("span", { class: "num" }, `${row.avg_duration_ms}ms`); } },
  { title: "Token 消耗", key: "total_tokens", render(row) { return h("span", { class: "num" }, row.total_tokens.toLocaleString()); } },
];

async function loadData(hours: number) {
  loading.value = true;
  error.value = null;
  timeRange.value = hours;
  try {
    const [hourly, providers] = await Promise.all([
      api.hourlyStats(hours),
      api.statsByProvider(10),
    ]);
    hourlyData.value = hourly;
    providerStats.value = providers;
  } catch (e: any) {
    error.value = e.message || "加载监控数据失败";
  } finally {
    loading.value = false;
  }
}

onMounted(() => loadData(24));
</script>

<template>
  <div class="monitoring">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">实时监控</h2>
      </div>
      <div class="toolbar-right">
        <NSpace>
          <NButton :type="timeRange === 1 ? 'primary' : 'default'" secondary size="small" @click="loadData(1)">1小时</NButton>
          <NButton :type="timeRange === 24 ? 'primary' : 'default'" secondary size="small" @click="loadData(24)">24小时</NButton>
          <NButton :type="timeRange === 168 ? 'primary' : 'default'" secondary size="small" @click="loadData(168)">7天</NButton>
          <NButton quaternary size="small" @click="loadData(timeRange)">↻ 刷新</NButton>
        </NSpace>
      </div>
    </div>

    <NSpin :show="loading">
      <!-- Error State -->
      <div v-if="error" class="error-state">
        <div class="error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h3 class="error-title">监控数据加载失败</h3>
        <p class="error-desc">{{ error }}</p>
        <NButton type="primary" @click="loadData(timeRange)">重新加载</NButton>
      </div>

      <template v-if="!error">
      <!-- Metric Cards -->
      <NGrid :x-gap="16" :y-gap="16" :cols="3" class="mb-16">
        <NGi>
          <NCard :bordered="false" class="metric-card">
            <div class="stat-label">请求总数 ({{ timeRange }}h)</div>
            <div class="stat-value accent">{{ hourlyData.reduce((s, h) => s + h.request_count, 0).toLocaleString() }}</div>
            <div class="stat-sub">所有请求汇总</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard :bordered="false" class="metric-card">
            <div class="stat-label">平均响应时间</div>
            <div class="stat-value success">
              {{ hourlyData.length > 0 ? Math.round(hourlyData.reduce((s, h) => s + h.avg_duration_ms, 0) / hourlyData.length) : 0 }}
              <span class="stat-unit">ms</span>
            </div>
            <div class="stat-sub">整体平均延迟</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard :bordered="false" class="metric-card">
            <div class="stat-label">Token 消耗</div>
            <div class="stat-value accent">{{ (hourlyData.reduce((s, h) => s + h.total_tokens, 0) / 1000).toFixed(1) }}<span class="stat-unit">K</span></div>
            <div class="stat-sub">所有渠道合计</div>
          </NCard>
        </NGi>
      </NGrid>

      <!-- Chart placeholder -->
      <NCard title="请求趋势" :bordered="false" class="mb-16 section-card" size="small">
        <div class="chart-area">
          <div class="chart-placeholder">
            <NText depth="3">时序图表区域 · {{ timeRange }}小时请求量与延迟趋势</NText>
          </div>
        </div>
      </NCard>

      <!-- Provider Load -->
      <NCard title="渠道负载分布" :bordered="false" class="section-card" size="small">
        <NDataTable
          :columns="providerColumns"
          :data="providerStats"
          :bordered="false"
          :single-line="false"
          size="small"
        />
      </NCard>
      </template>
    </NSpin>
  </div>
</template>

<style scoped>
.monitoring {
  max-width: 1200px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar-right {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.mb-16 {
  margin-bottom: 16px;
}

.metric-card {
  border-radius: 12px;
}

.stat-label {
  font-size: 13px;
  color: var(--text-color-3, #64748b);
  margin-bottom: 4px;
}

.stat-value {
  font-size: 28px;
  font-weight: 700;
  line-height: 1.2;
  margin-bottom: 4px;
}

.stat-value.accent {
  color: #6366f1;
}

.stat-value.success {
  color: #22c55e;
}

.stat-unit {
  font-size: 14px;
  font-weight: 500;
  opacity: 0.6;
}

.stat-sub {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}

.section-card {
  border-radius: 12px;
}

.chart-area {
  height: 260px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--border-color, #e2e8f0);
  border-radius: 8px;
}

.chart-placeholder {
  text-align: center;
}

.num {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-size: 12px;
}
</style>
