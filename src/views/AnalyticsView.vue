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
import { api, type ProviderStats } from "../api";

const loading = ref(false);
const error = ref<string | null>(null);

const period = ref(30);
const providerStats = ref<ProviderStats[]>([]);

const columns: DataTableColumns<ProviderStats> = [
  {
    title: "模型",
    key: "provider_name",
    render(row) {
      return h("strong", {}, { default: () => row.provider_name || "未知" });
    },
  },
  {
    title: "请求数",
    key: "request_count",
    render(row) {
      return h("span", { class: "num" }, row.request_count.toLocaleString());
    },
  },
  {
    title: "Token 消耗",
    key: "total_tokens",
    render(row) {
      return h("span", { class: "num" }, (row.total_tokens / 1000).toFixed(1) + "K");
    },
  },
  {
    title: "平均响应",
    key: "avg_duration_ms",
    render(row) {
      return h("span", { class: "num" }, row.avg_duration_ms + "ms");
    },
  },
  {
    title: "估算费用",
    key: "total_tokens",
    render(row) {
      const estimatedCost = (row.total_tokens / 1000000 * 3).toFixed(2);
      return h("span", { class: "num" }, `$${estimatedCost}`);
    },
  },
];

const totalTokens = ref(0);
const totalRequests = ref(0);
const avgResp = ref(0);
const totalCost = ref(0);

async function loadData(days: number) {
  loading.value = true;
  error.value = null;
  period.value = days;
  try {
    const [providers] = await Promise.all([
      api.statsByProvider(10),
    ]);
    providerStats.value = providers;

    totalRequests.value = providers.reduce((s, p) => s + p.request_count, 0);
    totalTokens.value = providers.reduce((s, p) => s + p.total_tokens, 0);
    avgResp.value = providers.length > 0
      ? Math.round(providers.reduce((s, p) => s + p.avg_duration_ms, 0) / providers.length)
      : 0;
    totalCost.value = providers.reduce((s, p) => s + (p.total_tokens / 1000000 * 3), 0);
  } catch (e: any) {
    error.value = e.message || "加载用量数据失败";
  } finally {
    loading.value = false;
  }
}

onMounted(() => loadData(30));
</script>

<template>
  <div class="analytics">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">用量分析</h2>
      </div>
      <div class="toolbar-right">
        <NSpace>
          <NButton :type="period === 7 ? 'primary' : 'default'" secondary size="small" @click="loadData(7)">7天</NButton>
          <NButton :type="period === 30 ? 'primary' : 'default'" secondary size="small" @click="loadData(30)">30天</NButton>
          <NButton :type="period === 90 ? 'primary' : 'default'" secondary size="small" @click="loadData(90)">90天</NButton>
        </NSpace>
      </div>
    </div>

    <NSpin :show="loading">
      <!-- Error State -->
      <div v-if="error" class="error-state">
        <div class="error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:48px;height:48px;color:#ef4444"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h3 class="error-title">用量数据加载失败</h3>
        <p class="error-desc">{{ error }}</p>
        <NButton type="primary" @click="loadData(period)">重新加载</NButton>
      </div>

      <template v-if="!error">
      <NGrid :x-gap="16" :y-gap="16" :cols="4" class="mb-16">
        <NGi>
          <NCard :bordered="false" class="stat-card">
            <div class="stat-label">总 Token 消耗</div>
            <div class="stat-value accent">{{ (totalTokens / 1000).toFixed(1) }}<span class="stat-unit">K</span></div>
            <div class="stat-sub">{{ period }} 天累计</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard :bordered="false" class="stat-card">
            <div class="stat-label">总请求数</div>
            <div class="stat-value">{{ totalRequests.toLocaleString() }}</div>
            <div class="stat-sub">{{ period }} 天累计</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard :bordered="false" class="stat-card">
            <div class="stat-label">平均响应时间</div>
            <div class="stat-value success">{{ avgResp }}<span class="stat-unit">ms</span></div>
            <div class="stat-sub">整体延迟</div>
          </NCard>
        </NGi>
        <NGi>
          <NCard :bordered="false" class="stat-card">
            <div class="stat-label">总费用（估算）</div>
            <div class="stat-value accent">${{ totalCost.toFixed(2) }}</div>
            <div class="stat-sub">按 $3/1M tokens 估算</div>
          </NCard>
        </NGi>
      </NGrid>

      <NCard title="Token 消耗趋势" :bordered="false" class="mb-16 section-card" size="small">
        <div class="chart-area">
          <div class="chart-placeholder">
            <NText depth="3">时序图表区域 · {{ period }}天 Token 消耗与费用趋势</NText>
          </div>
        </div>
      </NCard>

      <NCard title="模型用量排行" :bordered="false" class="section-card" size="small">
        <NDataTable
          :columns="columns"
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
.analytics {
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

.stat-card {
  border-radius: 12px;
}

.stat-label {
  font-size: 13px;
  color: var(--text-color-3, #64748b);
  margin-bottom: 4px;
}

.stat-value {
  font-size: 24px;
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
