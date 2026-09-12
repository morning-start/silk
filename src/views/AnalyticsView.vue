<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NButton, NSelect } from "naive-ui";
import { statsApi } from "../api/stats";
import type { HourlyStats, ProviderStats } from "../api";
import AppPageShell from "../components/AppPageShell.vue";

/**
 * 统计分析
 *
 * 数据口径（与后端一致，不做美化）：
 * - `hourly_stats(hours)`：按小时分组的窗口统计，时间范围可指定
 * - `stats_by_provider(limit)`：后端 SQL 固定取近 1 天，因此该面板恒定标注 24 小时
 * 时间范围超过 48 小时时，前端把小时桶按天合并，避免几百行看不出趋势。
 */

const PERIOD_OPTIONS = [
  { label: "近 24 小时", value: 24 },
  { label: "近 7 天", value: 168 },
  { label: "近 30 天", value: 720 },
];

/** 渠道面板的后端口径：近 1 天 */
const PROVIDER_WINDOW_HOURS = 24;

const periodHours = ref(24);
const loading = ref(false);
const error = ref<string | null>(null);

const hourly = ref<HourlyStats[]>([]);
const byProvider = ref<ProviderStats[]>([]);

interface Bucket {
  label: string;
  requests: number;
  tokens: number;
  avgMs: number;
}

/** 小时桶 → 展示桶：≤48 小时按小时，更长按天合并（均值按请求数加权） */
const buckets = computed<Bucket[]>(() => {
  if (hourly.value.length === 0) return [];

  if (periodHours.value <= 48) {
    return hourly.value.map((h) => ({
      label: h.hour.slice(11, 16) || h.hour,
      requests: h.request_count,
      tokens: h.total_tokens,
      avgMs: h.avg_duration_ms,
    }));
  }

  const byDay = new Map<string, { requests: number; tokens: number; weighted: number }>();
  for (const h of hourly.value) {
    const day = h.hour.slice(0, 10);
    const acc = byDay.get(day) ?? { requests: 0, tokens: 0, weighted: 0 };
    acc.requests += h.request_count;
    acc.tokens += h.total_tokens;
    acc.weighted += h.avg_duration_ms * h.request_count;
    byDay.set(day, acc);
  }

  return [...byDay.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([day, acc]) => ({
      label: day.slice(5),
      requests: acc.requests,
      tokens: acc.tokens,
      avgMs: acc.requests > 0 ? acc.weighted / acc.requests : 0,
    }));
});

/** 窗口汇总：由展示桶聚合，保证与下方列表同源 */
const summary = computed(() => {
  const items = buckets.value;
  const requests = items.reduce((sum, b) => sum + b.requests, 0);
  const tokens = items.reduce((sum, b) => sum + b.tokens, 0);
  const weighted = items.reduce((sum, b) => sum + b.avgMs * b.requests, 0);
  const peak = items.reduce<Bucket | null>(
    (max, b) => (max === null || b.requests > max.requests ? b : max),
    null
  );
  return {
    requests,
    tokens,
    avgMs: requests > 0 ? weighted / requests : 0,
    peakLabel: peak && peak.requests > 0 ? peak.label : null,
    peakRequests: peak?.requests ?? 0,
  };
});

const periodLabel = computed(
  () => PERIOD_OPTIONS.find((o) => o.value === periodHours.value)?.label ?? "近 24 小时"
);
const bucketUnit = computed(() => (periodHours.value <= 48 ? "小时" : "天"));

const providerRows = computed(() => {
  const total = byProvider.value.reduce((sum, p) => sum + p.request_count, 0);
  return byProvider.value.map((p) => ({
    name: p.provider_name || "未知渠道",
    requests: p.request_count,
    tokens: p.total_tokens,
    avgMs: p.avg_duration_ms,
    share: total > 0 ? p.request_count / total : 0,
  }));
});

const maxBucketRequests = computed(() =>
  buckets.value.reduce((max, b) => Math.max(max, b.requests), 0)
);

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1000) return `${(value / 1000).toFixed(1)}K`;
  return value.toLocaleString();
}

function formatMs(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "-";
  return value >= 1000 ? `${(value / 1000).toFixed(2)}s` : `${Math.round(value)}ms`;
}

async function loadData() {
  loading.value = true;
  error.value = null;
  try {
    const [hourlyData, providerData] = await Promise.all([
      statsApi.hourly(periodHours.value),
      statsApi.byProvider(10),
    ]);
    hourly.value = hourlyData;
    byProvider.value = providerData;
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : "加载统计数据失败";
  } finally {
    loading.value = false;
  }
}

function changePeriod(hours: number) {
  periodHours.value = hours;
  loadData();
}

onMounted(loadData);
</script>

<template>
  <AppPageShell
    title="统计分析"
    desc="按时间与渠道汇总请求量、响应耗时与 Token 消耗。"
    :loading="loading"
    :error="error"
    :empty="!error && buckets.length === 0 && providerRows.length === 0"
    empty-title="暂无统计数据"
    empty-description="网关处理请求后，这里会按时间与渠道汇总请求量、响应耗时与 Token 消耗。"
    @reload="loadData"
  >
    <template #count>
      <span class="s-card-meta">请求量 · 耗时 · Token</span>
    </template>
    <template #actions>
      <NSelect
        :value="periodHours"
        :options="PERIOD_OPTIONS"
        style="width: 140px"
        @update:value="(value: number) => changePeriod(value)"
      />
      <NButton size="small" secondary @click="loadData">刷新</NButton>
    </template>

    <!-- 窗口汇总：与下方时段列表同源 -->
    <div class="s-stats">
      <div class="s-stat s-stat--accent">
        <div class="s-stat-label">请求数（{{ periodLabel }}）</div>
        <div class="s-stat-value">{{ summary.requests.toLocaleString() }}</div>
        <div class="s-stat-sub">
          {{ summary.peakLabel ? `峰值 ${summary.peakLabel} · ${summary.peakRequests} 次` : "暂无峰值" }}
        </div>
      </div>
      <div class="s-stat s-stat--success">
        <div class="s-stat-label">平均响应</div>
        <div class="s-stat-value">{{ formatMs(summary.avgMs) }}</div>
        <div class="s-stat-sub">按请求数加权</div>
      </div>
      <div class="s-stat s-stat--indigo">
        <div class="s-stat-label">Token 消耗</div>
        <div class="s-stat-value">{{ formatTokens(summary.tokens) }}</div>
        <div class="s-stat-sub">输入 + 输出</div>
      </div>
      <div class="s-stat s-stat--neutral">
        <div class="s-stat-label">活跃渠道（近 24 小时）</div>
        <div class="s-stat-value">{{ providerRows.length }}</div>
        <div class="s-stat-sub">按请求数排序前 10</div>
      </div>
    </div>

    <div class="s-grid-2">
      <!-- 时段分布 -->
      <section class="s-card">
        <div class="s-card-head">
          <span class="s-card-title">时段分布</span>
          <span class="s-card-meta">按{{ bucketUnit }}聚合 · {{ periodLabel }}</span>
        </div>
        <div class="bar-list">
          <div v-for="bucket in buckets" :key="bucket.label" class="bar-row">
            <span class="bar-label text-mono">{{ bucket.label }}</span>
            <div class="bar-track">
              <div
                class="bar-fill"
                :style="{ width: maxBucketRequests > 0 ? `${(bucket.requests / maxBucketRequests) * 100}%` : '0%' }"
              ></div>
            </div>
            <span class="bar-value text-mono">{{ bucket.requests.toLocaleString() }}</span>
            <span class="bar-sub text-mono">{{ formatMs(bucket.avgMs) }}</span>
          </div>
        </div>
      </section>

      <!-- 渠道分布 -->
      <section class="s-card">
        <div class="s-card-head">
          <span class="s-card-title">渠道分布</span>
          <span class="s-card-meta">近 {{ PROVIDER_WINDOW_HOURS }} 小时 · 按请求数</span>
        </div>
        <div v-if="providerRows.length === 0" class="ana-nodata">近 24 小时暂无请求记录</div>
        <div v-else class="bar-list">
          <div v-for="row in providerRows" :key="row.name" class="bar-row bar-row--provider">
            <span class="bar-label" :title="row.name">{{ row.name }}</span>
            <div class="bar-track">
              <div class="bar-fill bar-fill--share" :style="{ width: `${row.share * 100}%` }"></div>
            </div>
            <span class="bar-value text-mono">{{ (row.share * 100).toFixed(1) }}%</span>
            <span class="bar-sub text-mono">
              {{ row.requests.toLocaleString() }} 次 · {{ formatTokens(row.tokens) }} · {{ formatMs(row.avgMs) }}
            </span>
          </div>
        </div>
      </section>
    </div>
  </AppPageShell>
</template>

<style scoped>
/* 汇总卡片 / 双栏面板 / 统计卡全部走 style.css 的 s-* 规范；
   这里只保留本页特有的「横向条形」造型。 */

/* 面板内无数据时的占位，与 s-state 同一语气 */
.ana-nodata {
  padding: var(--sp-6) var(--sp-4);
  text-align: center;
  font-size: var(--fs-sm);
  color: var(--muted);
}

/* ---- 条形列表：一屏看清相对量级 ---- */
.bar-list {
  max-height: 420px;
  overflow-y: auto;
  padding: var(--sp-1) var(--sp-4) var(--sp-3);
}

.bar-row {
  display: grid;
  grid-template-columns: 46px 1fr 56px 52px;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-1) 0;
}

.bar-row--provider {
  grid-template-columns: minmax(90px, 1fr) 84px 52px 150px;
}

.bar-label {
  font-size: var(--fs-xs);
  color: var(--muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bar-track {
  height: 6px;
  border-radius: 999px;
  background: var(--surface-alt);
  overflow: hidden;
}

.bar-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  min-width: 2px;
}

.bar-fill--share {
  background: var(--brand-2);
}

.bar-value {
  font-size: var(--fs-sm);
  color: var(--fg-2);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.bar-sub {
  font-size: var(--fs-kicker);
  color: var(--muted);
  text-align: right;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
