<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { NButton, NIcon, NProgress, NTag, useMessage } from "naive-ui";
import {
  CheckmarkCircleOutline,
  DownloadOutline,
  LogoGithub,
  OpenOutline,
  PersonOutline,
  RefreshOutline,
  ShieldCheckmarkOutline,
} from "@vicons/ionicons5";
import AppPageShell from "../components/AppPageShell.vue";
import { useGatewayStore } from "../stores/gateway";
import { copyWithFeedback } from "../utils/clipboard";
import { checkForUpdates, downloadAndInstall, type UpdateInfo } from "../utils/updater";

const REPO_URL = "https://github.com/morning-start/silk";
const REPO_LABEL = "morning-start/silk";
const AUTHOR_NAME = "morning-start";
const AUTHOR_URL = "https://github.com/morning-start";

/** 网关支持的协议族，与 prism 的 provider 名一致 */
const PROTOCOLS = ["openai", "messages", "responses", "gemini"];

const TECH_STACK = "Tauri 2 · Rust / Axum · Vue 3 · SQLite";

/** 本地性与密钥存储的事实说明，与后端实现保持一致 */
const PRIVACY_FACTS = [
  "配置与请求日志写入本机 SQLite 数据库，不随任何服务上传。",
  "渠道 API Key 使用 AES-GCM 加密后落盘。",
  "网关 Key 只保留 SHA-256 哈希，不保存明文。",
  "除「检查更新」访问 GitHub Releases 外，不发起其他外部请求。",
];

const message = useMessage();
const gatewayStore = useGatewayStore();

const version = ref("");
const checking = ref(false);
const installing = ref(false);
const progress = ref(0);
const updateInfo = ref<UpdateInfo | null>(null);

const running = computed(() => gatewayStore.status?.running ?? false);
const endpoint = computed(
  () => `http://${gatewayStore.status?.address ?? "127.0.0.1:1877"}`
);
const remoteAllowed = computed(
  () => gatewayStore.status?.settings.allow_remote ?? false
);
const isLatest = computed(
  () =>
    updateInfo.value !== null &&
    !updateInfo.value.available &&
    !updateInfo.value.error
);

/** 有签名清单时可静默安装，否则只能引导到下载地址 */
const installLabel = computed(() => {
  if (installing.value) return "更新中…";
  return updateInfo.value?.autoUpdateReady === false ? "前往下载" : "下载并安装";
});

function formatDate(date?: string): string {
  if (!date) return "";
  const d = new Date(date);
  if (Number.isNaN(d.getTime())) return date;
  return d.toLocaleString("zh-CN");
}

async function openLink(url: string) {
  try {
    await openUrl(url);
  } catch (error) {
    console.error("打开链接失败:", error);
  }
}

async function copyEndpoint() {
  if (await copyWithFeedback(endpoint.value)) {
    message.success("本地地址已复制");
  } else {
    message.error("复制失败");
  }
}

async function handleCheckUpdate() {
  checking.value = true;
  updateInfo.value = null;
  try {
    // 手动点击必须跳过缓存，否则用户会以为按钮没生效
    updateInfo.value = await checkForUpdates(true);
  } finally {
    checking.value = false;
  }
}

async function handleInstall() {
  installing.value = true;
  progress.value = 0;
  try {
    // 把当前检查结果传下去：静默安装不可用时用它取下载地址
    const outcome = await downloadAndInstall(
      (p) => {
        progress.value = Math.round(p * 100);
      },
      updateInfo.value ?? undefined
    );

    if (outcome === "installed") {
      message.success("更新已下载并安装，应用即将重启");
    } else if (outcome === "opened-download") {
      message.info("已在浏览器打开安装包下载，完成后请手动运行安装程序");
    } else {
      message.error("更新失败，请稍后重试");
    }
  } finally {
    installing.value = false;
  }
}

onMounted(async () => {
  version.value = await getVersion();
  // 网关状态只用于展示当前实例的监听地址与远程访问开关，取不到时保留默认值
  await gatewayStore.fetchStatus().catch(() => undefined);
  // 静默检查一次，有新版本直接展示在页面中
  updateInfo.value = await checkForUpdates();
});
</script>

<template>
  <AppPageShell title="关于" desc="当前实例状态、软件更新与本地优先的隐私说明。">
    <template #count>
      <NTag size="small" type="info">v{{ version || "…" }}</NTag>
      <span class="ab-state" :class="{ 'is-online': running }">
        <span class="ab-dot"></span>
        {{ running ? "网关运行中" : "网关已停止" }}
      </span>
    </template>
    <template #actions>
      <NButton size="small" secondary :loading="checking" @click="handleCheckUpdate">
        <template #icon><NIcon><RefreshOutline /></NIcon></template>
        检查更新
      </NButton>
    </template>

    <div class="p-split">
      <!-- 左栏：本机状态与更新（可变高度，放在一起不互相挤压） -->
      <div class="p-col">
        <!-- 本机实例：地址与状态优先，取代原先的渐变标识块 -->
        <section class="s-card">
          <header class="s-card-head">
            <span class="ab-kicker">本机实例</span>
            <span class="s-card-meta">{{ running ? "RUNNING" : "STOPPED" }}</span>
          </header>
          <div class="s-card-body">
            <div class="ab-id">
              <span class="ab-mark">S</span>
              <div>
                <h3 class="ab-name">Silk 丝路</h3>
                <p class="ab-sub">本地 AI 多模型中转网关</p>
              </div>
            </div>

            <dl class="ab-kv">
              <div class="ab-kv-row">
                <dt>监听地址</dt>
                <dd>
                  <span class="ab-strong text-mono">{{ endpoint }}</span>
                  <NButton quaternary size="tiny" @click="copyEndpoint">复制</NButton>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>远程访问</dt>
                <dd>
                  <NTag size="tiny" :type="remoteAllowed ? 'warning' : 'success'">
                    {{ remoteAllowed ? "已允许" : "已关闭" }}
                  </NTag>
                  <span class="ab-note">
                    {{ remoteAllowed ? "局域网内其他设备可访问" : "仅本机可访问" }}
                  </span>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>协议转换</dt>
                <dd>
                  <span
                    v-for="protocol in PROTOCOLS"
                    :key="protocol"
                    class="s-chip text-mono"
                  >{{ protocol }}</span>
                  <span class="ab-note">任意互转</span>
                </dd>
              </div>
            </dl>
          </div>
        </section>

        <!-- 软件更新：状态、发布说明与安装动作收在一张卡里 -->
        <section class="s-card">
          <header class="s-card-head">
            <span class="ab-kicker">软件更新</span>
            <span class="s-card-meta">v{{ version || "…" }}</span>
          </header>
          <div class="s-card-body">
            <p class="ab-note ab-hint">
              更新取自 GitHub Releases。发布带签名清单时可自动下载安装，完成后重启应用；
              否则转为浏览器下载安装包。
            </p>

            <div v-if="checking" class="ab-line ab-muted">正在检查更新…</div>

            <template v-else-if="updateInfo?.available">
              <div class="ab-line">
                <span class="ab-dot ab-dot--accent"></span>
                发现新版本
                <span class="ab-strong text-mono">v{{ updateInfo.version }}</span>
                <span v-if="updateInfo.date" class="ab-note">
                  发布于 {{ formatDate(updateInfo.date) }}
                </span>
              </div>

              <pre v-if="updateInfo.body" class="ab-notes">{{ updateInfo.body }}</pre>

              <div v-if="installing" class="ab-progress">
                <NProgress
                  type="line"
                  :percentage="progress"
                  :height="6"
                  :show-indicator="false"
                />
                <span class="ab-note">
                  {{ progress < 100 ? `正在下载… ${progress}%` : "下载完成，正在安装…" }}
                </span>
              </div>

              <p v-if="updateInfo.autoUpdateReady === false" class="ab-note">
                本次发布未提供自动更新清单，将为你打开
                {{ updateInfo.assetName || "安装包" }} 的下载地址。
              </p>

              <div class="ab-actions">
                <NButton
                  type="primary"
                  size="small"
                  :loading="installing"
                  :disabled="installing"
                  @click="handleInstall"
                >
                  <template #icon><NIcon><DownloadOutline /></NIcon></template>
                  {{ installLabel }}
                </NButton>
                <NButton
                  v-if="updateInfo.releaseUrl"
                  size="small"
                  quaternary
                  @click="openLink(updateInfo.releaseUrl!)"
                >
                  查看发布页
                </NButton>
              </div>
            </template>

            <div v-else-if="isLatest" class="ab-line ab-line--ok">
              <span class="ab-dot ab-dot--ok"></span>
              已是最新版本
            </div>

            <div v-else-if="updateInfo?.error" class="ab-line ab-line--error">
              <span class="ab-dot ab-dot--error"></span>
              检查更新失败
              <span class="ab-note">{{ updateInfo.error }}</span>
            </div>

            <div v-else class="ab-line ab-muted">
              点击右上角「检查更新」获取最新版本信息。
            </div>
          </div>
        </section>
      </div>

      <!-- 右栏：静态资料，条目短且数量固定 -->
      <div class="p-col">
        <section class="s-card">
          <header class="s-card-head">
            <span class="ab-kicker">项目信息</span>
          </header>
          <div class="s-card-body">
            <dl class="ab-kv">
              <div class="ab-kv-row">
                <dt>仓库</dt>
                <dd>
                  <NIcon class="ab-ico"><LogoGithub /></NIcon>
                  <span class="ab-strong text-mono">{{ REPO_LABEL }}</span>
                  <NButton quaternary size="tiny" @click="openLink(REPO_URL)">
                    打开<NIcon :size="12"><OpenOutline /></NIcon>
                  </NButton>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>作者</dt>
                <dd>
                  <NIcon class="ab-ico"><PersonOutline /></NIcon>
                  <span class="ab-strong">{{ AUTHOR_NAME }}</span>
                  <NButton quaternary size="tiny" @click="openLink(AUTHOR_URL)">
                    主页<NIcon :size="12"><OpenOutline /></NIcon>
                  </NButton>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>许可证</dt>
                <dd>
                  <NIcon class="ab-ico"><ShieldCheckmarkOutline /></NIcon>
                  <span class="ab-strong text-mono">AGPL-3.0</span>
                  <span class="ab-note">可自建与修改，衍生分发需同样开源</span>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>技术栈</dt>
                <dd><span class="ab-note">{{ TECH_STACK }}</span></dd>
              </div>
            </dl>
          </div>
        </section>

        <section class="s-card">
          <header class="s-card-head">
            <span class="ab-kicker">数据与隐私</span>
            <span class="s-card-meta">LOCAL-FIRST</span>
          </header>
          <div class="s-card-body">
            <ul class="ab-facts">
              <li v-for="fact in PRIVACY_FACTS" :key="fact">
                <NIcon :size="14" class="ab-fact-ico"><CheckmarkCircleOutline /></NIcon>
                <span>{{ fact }}</span>
              </li>
            </ul>
          </div>
        </section>
      </div>
    </div>
  </AppPageShell>
</template>

<style scoped>
/* 两栏铺满：左栏是「会变的东西」（状态、更新），右栏是「不变的东西」（资料）。
   栏宽比 1.35:1 由全局 .p-split 提供，窄窗单栏也由全局断点处理。
   卡片 / 页头 / 徽标 / 空态全部走 style.css 的 s-* 规范；
   这里只保留本页特有的标识、key-value、隐私要点与更新造型。 */

/* ---------- 页头状态 ---------- */
.ab-state {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  font-size: var(--fs-sm);
  color: var(--muted);
}

.ab-state.is-online {
  color: var(--fg-2);
}

.ab-dot {
  width: 6px;
  height: 6px;
  flex: none;
  border-radius: 50%;
  background: var(--muted);
}

.ab-state.is-online .ab-dot { background: var(--success); }
.ab-dot--accent { background: var(--accent); }
.ab-dot--ok { background: var(--success); }
.ab-dot--error { background: var(--danger); }

/* ---------- 标识 ---------- */
.ab-id {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  margin-bottom: var(--sp-3);
}

.ab-mark {
  width: 36px;
  height: 36px;
  flex: none;
  display: grid;
  place-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface-alt);
  color: var(--fg-2);
  font-family: var(--font-mono);
  font-size: var(--fs-lg);
  font-weight: 700;
}

.ab-name {
  margin: 0 0 3px;
  font-size: var(--fs-lg);
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--fg);
}

.ab-sub {
  margin: 0;
  font-size: var(--fs-sm);
  color: var(--muted);
}

/* ---------- key-value 行 ---------- */
.ab-kv {
  margin: 0;
  display: flex;
  flex-direction: column;
}

.ab-kv-row {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 9px 0;
  border-top: 1px solid var(--border-soft);
  min-width: 0;
}

.ab-kv-row dt {
  flex: none;
  width: 76px;
  font-size: var(--fs-sm);
  color: var(--muted);
}

.ab-kv-row dd {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin: 0;
  min-width: 0;
  font-size: var(--fs-base);
  color: var(--fg-2);
}

.ab-strong { color: var(--fg); }
.ab-note { font-size: var(--fs-sm); color: var(--muted); }
.ab-ico { color: var(--muted); }

/* ---------- 隐私要点 ---------- */
.ab-facts {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.ab-facts li {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-2);
  font-size: var(--fs-sm);
  line-height: 1.6;
  color: var(--fg-2);
}

.ab-fact-ico {
  flex: none;
  margin-top: 2px;
  color: var(--success);
}

/* ---------- 更新 ---------- */
.ab-hint {
  margin: 0 0 var(--sp-3);
}

.ab-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--sp-2);
  font-size: var(--fs-base);
  color: var(--fg-2);
}

.ab-line--ok { color: var(--success); }
.ab-line--error { color: var(--danger); }
.ab-muted { font-size: var(--fs-sm); color: var(--muted); }

.ab-notes {
  margin: var(--sp-3) 0 0;
  padding: var(--sp-3) var(--sp-4);
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-sm);
  background: var(--surface-alt);
  font-family: var(--font-sans);
  font-size: var(--fs-sm);
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--fg-2);
}

.ab-progress {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin-top: var(--sp-3);
}

.ab-actions {
  display: flex;
  gap: var(--sp-2);
  margin-top: var(--sp-3);
}
</style>
