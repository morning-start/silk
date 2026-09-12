<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
import { useRouter } from "vue-router";
import { formatMs } from "../utils/format";
import { NIcon, NButton, useMessage } from "naive-ui";
import {
  ArrowForwardOutline,
  CopyOutline,
  KeyOutline,
  RefreshOutline,
} from "@vicons/ionicons5";
import AppPageShell from "../components/AppPageShell.vue";
import { statsApi } from "../api/stats";
import { configApi } from "../api/config";
import type { DashboardStats, RequestLog } from "../api";
import { useGatewayStore } from "../stores/gateway";
import { useDataChangeSignal } from "../composables/useCrossStoreNotify";
import { useConfirm } from "../utils/confirm";

const router = useRouter();
const gatewayStore = useGatewayStore();
const message = useMessage();
const { confirm } = useConfirm();

const loading = ref(false);
const error = ref<string | null>(null);
const stats = ref<DashboardStats | null>(null);
const recentLogs = ref<RequestLog[]>([]);
const logsLoading = ref(false);
const bindAddress = computed(() => gatewayStore.status?.address ?? "127.0.0.1:1877");
const gatewayKey = ref("");

const isRunning = computed(() => gatewayStore.status?.running ?? false);

/** 今日失败数：错误统计只在真正 >0 时才该被看见 */
const todayErrors = computed(
  () => (stats.value?.today_requests || 0) - (stats.value?.today_success || 0)
);

function deltaLabel(): string {
  if (!stats.value?.yesterday_requests) return "较昨日 0%";
  const pct = (stats.value.today_requests / stats.value.yesterday_requests - 1) * 100;
  return `${pct >= 0 ? "+" : ""}${pct.toFixed(1)}% 较昨日`;
}

function statusTone(status?: number | null): string {
  if (!status) return "s-badge--neutral";
  if (status < 300) return "s-badge--success";
  if (status < 400) return "s-badge--warn";
  return "s-badge--danger";
}

/** 方法标签的色调映射：与状态码标签共用同一组 s-badge 语义色 */
function methodTone(method?: string): string {
  switch ((method || "GET").toUpperCase()) {
    case "POST":
      return "success";
    case "PUT":
    case "PATCH":
      return "warn";
    case "DELETE":
      return "danger";
    default:
      return "neutral";
  }
}

async function loadData() {
  loading.value = true;
  logsLoading.value = true;
  error.value = null;
  try {
    const [s, logs, key] = await Promise.all([
      statsApi.dashboard(),
      statsApi.recentRequests(10),
      configApi.getBuiltinGatewayKey(),
    ]);
    stats.value = s;
    recentLogs.value = logs;
    gatewayKey.value = key.plain_key;
  } catch (e: any) {
    error.value = e.message || "加载仪表盘数据失败";
  } finally {
    loading.value = false;
    logsLoading.value = false;
  }
}

function goToLogs() {
  router.push("/logs");
}

function copyGatewayAddress() {
  navigator.clipboard.writeText(`http://${bindAddress.value}`).then(() => {
    message.success("本地地址已复制");
  }).catch(() => {
    message.error("复制失败");
  });
}

function copyGatewayKey() {
  navigator.clipboard.writeText(gatewayKey.value).then(() => {
    message.success("API Key 已复制");
  }).catch(() => {
    message.error("复制失败");
  });
}

function resetGatewayKey() {
  confirm({
    title: "刷新本地 API Key",
    description: "重新生成网关的内置 Key，旧 Key 立即作废。",
    impacts: [
      "正在使用旧 Key 的客户端会立刻收到鉴权失败",
      "需要把新 Key 同步更新到各客户端配置中",
    ],
    positiveText: "刷新",
    onConfirm: async () => {
      const res = await configApi.resetBuiltinGatewayKey();
      gatewayKey.value = res.plain_key;
      message.success("API Key 已刷新，请更新客户端配置");
    },
    onError: (e: any) => message.error(e?.message || "刷新 API Key 失败"),
  });
}

onMounted(() => {
  loadData();
  gatewayStore.fetchStatus();
});

// 跨 Store 联动
const providersSignal = useDataChangeSignal("providers");

watch(
  [providersSignal],
  () => { loadData(); },
  { flush: "post" }
);
</script>

<template>
  <AppPageShell
    title="仪表盘"
    desc="网关运行概览：实时状态、请求量级与最近请求。"
    :loading="loading"
    :error="error"
    reload-text="重新加载"
    @reload="loadData"
  >
    <template #actions>
      <NButton size="small" :loading="loading" @click="loadData">
        <template #icon><NIcon :size="14"><RefreshOutline /></NIcon></template>
        刷新数据
      </NButton>
    </template>

    <!-- 运行状态：左侧文字结论，右侧深色快照面板 -->
    <section class="s-card">
      <div class="s-card-body p-split">
        <div class="p-col">
          <span class="dash-kicker">SILK / LOCAL RUNTIME</span>

          <span class="s-badge" :class="isRunning ? 's-badge--success' : 's-badge--neutral'">
            <span class="s-dot" :class="{ 'is-online': isRunning }"></span>
            {{ isRunning ? "网关在线" : "网关离线" }}
          </span>

          <h2 class="dash-heading">
            {{ isRunning ? "Silk 本地网关运行中" : "Silk 本地网关未启动" }}
          </h2>

          <p class="dash-desc">
            <template v-if="isRunning">
              本地监听 <span class="text-mono">{{ bindAddress }}</span>，今日已处理
              <span class="text-mono">{{ stats?.today_requests?.toLocaleString() || 0 }}</span> 次请求
            </template>
            <template v-else>网关尚未启动，配置好渠道后即可启动网关转发请求</template>
          </p>

          <div v-if="isRunning && stats" class="dash-badges">
            <span class="s-badge s-badge--success">可用渠道 {{ stats.active_providers }}</span>
            <span class="s-badge s-badge--accent">运行正常</span>
          </div>

          <div class="dash-actions">
            <NButton type="primary" size="small" @click="copyGatewayAddress">
              <template #icon><NIcon :size="14"><CopyOutline /></NIcon></template>
              复制本地 API 地址
            </NButton>
            <NButton size="small" @click="copyGatewayKey">
              <template #icon><NIcon :size="14"><KeyOutline /></NIcon></template>
              复制 API Key
            </NButton>
            <NButton size="small" @click="resetGatewayKey">
              <template #icon><NIcon :size="14"><RefreshOutline /></NIcon></template>
              刷新 API Key
            </NButton>
          </div>
        </div>

        <div class="dash-console">
          <div class="dash-console-head">
            <div class="dash-console-title">运行快照</div>
            <div class="dash-console-sub">连通度与排障重点</div>
          </div>
          <div class="dash-console-grid">
            <div class="dash-metric">
              <div class="dash-metric-label">平均响应</div>
              <div class="dash-metric-value">
                {{ stats?.today_avg_duration_ms ? Math.round(stats.today_avg_duration_ms) + "ms" : "-" }}
              </div>
            </div>
            <div class="dash-metric">
              <div class="dash-metric-label">今日请求</div>
              <div class="dash-metric-value">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
            </div>
            <div class="dash-metric">
              <div class="dash-metric-label">Token 消耗</div>
              <div class="dash-metric-value">
                {{ stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) + "K" : "-" }}
              </div>
            </div>
            <div class="dash-metric">
              <div class="dash-metric-label">错误请求</div>
              <div class="dash-metric-value" :class="{ 'is-danger': todayErrors > 0 }">{{ todayErrors }}</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- 关键指标 -->
    <div class="s-stats">
      <div class="s-stat s-stat--accent">
        <div class="s-stat-label">今日请求数</div>
        <div class="s-stat-value">{{ stats?.today_requests?.toLocaleString() || 0 }}</div>
        <div class="s-stat-sub">{{ deltaLabel() }}</div>
      </div>
      <div class="s-stat s-stat--success">
        <div class="s-stat-label">平均响应时间</div>
        <div class="s-stat-value">
          {{ Math.round(stats?.today_avg_duration_ms || 0) }}<span class="s-stat-unit">ms</span>
        </div>
        <div class="s-stat-sub">网关整体延迟</div>
      </div>
      <div class="s-stat s-stat--indigo">
        <div class="s-stat-label">今日 Token 消耗</div>
        <div class="s-stat-value">
          {{ (stats?.today_tokens ? (stats.today_tokens / 1000).toFixed(1) : "0") + "K" }}
        </div>
        <div class="s-stat-sub">总 Token 用量</div>
      </div>
      <div class="s-stat s-stat--neutral">
        <div class="s-stat-label">活跃渠道</div>
        <div class="s-stat-value">{{ stats?.active_providers || 0 }}</div>
        <div class="s-stat-sub">全部已配置</div>
      </div>
    </div>

    <!-- 最近请求 -->
    <section class="s-card">
      <header class="s-card-head">
        <h2 class="s-card-title">最新请求</h2>
        <NButton quaternary size="small" @click="goToLogs">
          全部日志
          <template #icon><NIcon :size="14"><ArrowForwardOutline /></NIcon></template>
        </NButton>
      </header>

      <div class="s-card-body s-card-body--flush">
        <div v-if="logsLoading" class="s-state">
          <p class="s-state-desc">加载中…</p>
        </div>

        <div v-else-if="recentLogs.length === 0" class="s-state">
          <h3 class="s-state-title">暂无请求记录</h3>
          <p class="s-state-desc">网关还没有处理过请求。启动网关并发出第一次调用后，这里会出现记录。</p>
        </div>

        <div v-else class="s-table-wrap">
          <table class="s-table">
            <thead>
              <tr>
                <th>时间</th>
                <th>方法</th>
                <th>路径</th>
                <th>状态</th>
                <th class="is-num">耗时</th>
                <th>渠道</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="log in recentLogs" :key="log.id">
                <td class="text-mono">
                  {{ (log.timestamp || "").length > 8 ? (log.timestamp || "").slice(11, 19) : (log.timestamp || "-") }}
                </td>
                <td>
                  <span class="s-badge s-badge--mono" :class="'s-badge--' + methodTone(log.method)">
                    {{ log.method || "GET" }}
                  </span>
                </td>
                <td class="dash-path text-mono">{{ log.path || "-" }}</td>
                <td>
                  <span class="s-badge" :class="statusTone(log.response_status)">
                    {{ log.response_status || "-" }}
                  </span>
                </td>
                <td class="num is-num">{{ formatMs(log.total_duration_ms) }}</td>
                <td>{{ log.provider_name || log.provider_id || "-" }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>
  </AppPageShell>
</template>

<style scoped>
/* 页面级布局、卡片、统计、徽标、表格全部走 style.css 的 p-* / s-* 规范，
   这里只保留本页特有的「运行状态 hero」造型。 */

.dash-kicker {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--accent);
}

.dash-heading {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  line-height: 1.4;
  letter-spacing: -0.02em;
  color: var(--fg);
}

.dash-desc {
  margin: 0;
  font-size: var(--fs-base);
  color: var(--fg-2);
}

.dash-badges,
.dash-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
}

.dash-actions {
  margin-top: var(--sp-1);
}

/* 深色快照面板：本页唯一的深色表面，作为「运行态」的视觉锚点 */
.dash-console {
  padding: var(--sp-4);
  border: 1px solid var(--sidebar-border);
  border-radius: var(--radius);
  background: var(--sidebar-bg-solid);
}

.dash-console-head {
  margin-bottom: var(--sp-3);
}

.dash-console-title {
  font-size: var(--fs-md);
  font-weight: 600;
  color: var(--sidebar-active);
}

.dash-console-sub {
  margin-top: 2px;
  font-size: var(--fs-kicker);
  color: var(--sidebar-fg);
}

.dash-console-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-2);
}

.dash-metric {
  padding: 10px 12px;
  border: 1px solid var(--sidebar-border);
  border-radius: var(--radius);
  background: rgba(255, 255, 255, 0.03);
}

.dash-metric-label {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--sidebar-fg);
}

.dash-metric-value {
  margin-top: 4px;
  font-family: var(--font-mono);
  font-size: var(--fs-lg);
  font-weight: 700;
  color: var(--sidebar-active);
}

.dash-metric-value.is-danger {
  color: var(--danger);
}

/* 路径列：唯一需要限宽截断的列 */
.dash-path {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
