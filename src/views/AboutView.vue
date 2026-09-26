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
import {
  checkKernelUpdate,
  formatSize,
  getKernelStatus,
  installKernelUpdate,
  restartApp,
  rollbackKernelUpdate,
  sourceLabel,
  type KernelCheckResult,
} from "../utils/kernel";
import type { KernelStatus } from "../api/kernel";
import { renderMarkdown } from "../utils/markdown";

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

// ---------------------------------------------------------------------------
// 协议内核（prism.wasm）
//
// 与「软件更新」分离：这里管的是协议转换内核，版本节奏与应用不同
// （应用 v1.1.3 / 内核 v0.1.4）。内核是进程级单例，装完必须重启才生效。
// ---------------------------------------------------------------------------
const kernelStatus = ref<KernelStatus | null>(null);
const kernelInfo = ref<KernelCheckResult | null>(null);
const kernelChecking = ref(false);
const kernelInstalling = ref(false);
const kernelRollingBack = ref(false);
const kernelInstalled = ref(false);

/** 当前内核版本展示：数据目录来源可反查，内嵌/程序目录来源无法反查 */
const kernelVersionLabel = computed(() => {
  const status = kernelStatus.value;
  if (!status) return "…";
  return status.version ? `v${status.version}` : "未知";
});

/** 内核来源的可读名称 */
const kernelSourceLabel = computed(() =>
  kernelStatus.value ? sourceLabel(kernelStatus.value.source) : ""
);

/** ABI 是否与宿主兼容（旧内核无 ABI 导出时无法判定） */
const kernelAbiCompatible = computed(() => {
  const status = kernelStatus.value;
  if (!status?.abi) return null;
  return status.abi === status.supported_abi;
});

const kernelIsLatest = computed(
  () =>
    kernelInfo.value !== null &&
    !kernelInfo.value.available &&
    !kernelInfo.value.error
);

/** 有更新且可安装时才允许点安装 */
const canInstallKernel = computed(
  () => kernelInfo.value?.available === true && kernelInfo.value?.canInstall === true
);

async function loadKernelStatus() {
  try {
    kernelStatus.value = await getKernelStatus();
  } catch (error) {
    console.error("读取内核状态失败:", error);
  }
}

async function handleCheckKernel(force = false) {
  kernelChecking.value = true;
  try {
    // 状态与检查一起刷新：安装后状态会变（来源、版本）
    await loadKernelStatus();
    kernelInfo.value = await checkKernelUpdate(force);
  } finally {
    kernelChecking.value = false;
  }
}

async function handleInstallKernel() {
  kernelInstalling.value = true;
  try {
    const result = await installKernelUpdate();
    kernelInstalled.value = true;
    message.success(
      `内核已更新至 v${result.version ?? "?"}（ABI ${result.abi}），重启后生效`
    );
    // 状态里的来源/版本已变，重新读取
    await loadKernelStatus();
    kernelInfo.value = null;
  } catch (error) {
    message.error(error instanceof Error ? error.message : String(error));
  } finally {
    kernelInstalling.value = false;
  }
}

async function handleRollbackKernel() {
  kernelRollingBack.value = true;
  try {
    const result = await rollbackKernelUpdate();
    kernelInstalled.value = true;
    message.success(
      `已回滚至 v${result.version ?? "备份版本"}（ABI ${result.abi}），重启后生效`
    );
    await loadKernelStatus();
    kernelInfo.value = null;
  } catch (error) {
    message.error(error instanceof Error ? error.message : String(error));
  } finally {
    kernelRollingBack.value = false;
  }
}

async function handleRestartApp() {
  try {
    await restartApp();
  } catch (error) {
    // 重启失败通常是权限或平台差异，提示用户手动重启即可
    console.error("重启应用失败:", error);
    message.error("重启失败，请手动退出并重新打开应用");
  }
}

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
  // 内核状态与检查同样静默执行（结果有 5 分钟缓存，来回切页面不会耗光 API 额度）
  await handleCheckKernel();
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

              <div v-if="updateInfo.body" class="ab-notes md-render" v-html="renderMarkdown(updateInfo.body)"></div>

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

        <!-- 协议内核：prism.wasm 的版本、ABI 与下载更新 -->
        <section class="s-card">
          <header class="s-card-head">
            <span class="ab-kicker">协议内核</span>
            <span class="s-card-meta">{{ kernelVersionLabel }}</span>
          </header>
          <div class="s-card-body">
            <p class="ab-note ab-hint">
              跨协议转换由 prism.wasm 提供。下载后会校验 SHA-256 并试运行探测 ABI，
              两道关都通过才替换；替换后需重启应用生效。
            </p>

            <dl class="ab-kv">
              <div class="ab-kv-row">
                <dt>当前版本</dt>
                <dd>
                  <span class="ab-strong text-mono">{{ kernelVersionLabel }}</span>
                  <span class="ab-note">{{ kernelSourceLabel }}</span>
                </dd>
              </div>
              <div class="ab-kv-row">
                <dt>ABI</dt>
                <dd>
                  <template v-if="kernelStatus?.abi">
                    <NTag size="tiny" :type="kernelAbiCompatible ? 'success' : 'error'">
                      {{ kernelStatus.abi }}
                    </NTag>
                    <span class="ab-note">
                      {{ kernelAbiCompatible ? "与宿主兼容" : `宿主需要 ${kernelStatus.supported_abi}` }}
                    </span>
                  </template>
                  <template v-else>
                    <NTag size="tiny" type="warning">未知</NTag>
                    <span class="ab-note">该内核不支持 ABI 探测，建议更新</span>
                  </template>
                </dd>
              </div>
            </dl>

            <div v-if="kernelChecking" class="ab-line ab-muted">正在检查内核更新…</div>

            <template v-else-if="kernelInfo?.available">
              <div class="ab-line">
                <span class="ab-dot ab-dot--accent"></span>
                发现新内核
                <span class="ab-strong text-mono">v{{ kernelInfo.version }}</span>
                <span v-if="kernelInfo.assetSize" class="ab-note">
                  {{ formatSize(kernelInfo.assetSize) }}
                </span>
                <span v-if="kernelInfo.date" class="ab-note">
                  发布于 {{ formatDate(kernelInfo.date) }}
                </span>
              </div>

              <div v-if="kernelInfo.body" class="ab-notes md-render" v-html="renderMarkdown(kernelInfo.body)"></div>

              <p v-if="kernelInfo.blockedReason" class="ab-line ab-line--error">
                <span class="ab-dot ab-dot--error"></span>
                {{ kernelInfo.blockedReason }}
              </p>

              <div class="ab-actions">
                <NButton
                  type="primary"
                  size="small"
                  :loading="kernelInstalling"
                  :disabled="!canInstallKernel || kernelInstalling"
                  @click="handleInstallKernel"
                >
                  <template #icon><NIcon><DownloadOutline /></NIcon></template>
                  {{ kernelInstalling ? "安装中…" : "下载并安装" }}
                </NButton>
                <NButton
                  v-if="kernelInfo.releaseUrl"
                  size="small"
                  quaternary
                  @click="openLink(kernelInfo.releaseUrl!)"
                >
                  查看发布页
                </NButton>
              </div>
            </template>

            <div v-else-if="kernelIsLatest" class="ab-line ab-line--ok">
              <span class="ab-dot ab-dot--ok"></span>
              内核已是最新版本
            </div>

            <div v-else-if="kernelInfo?.error" class="ab-line ab-line--error">
              <span class="ab-dot ab-dot--error"></span>
              检查内核更新失败
              <span class="ab-note">{{ kernelInfo.error }}</span>
            </div>

            <div v-else class="ab-line ab-muted">
              暂未获取到内核更新信息。
            </div>

            <!-- 安装成功：内核是进程级单例，必须重启才生效 -->
            <div v-if="kernelInstalled" class="ab-line ab-line--ok ab-restart">
              <span class="ab-dot ab-dot--ok"></span>
              新内核已安装，重启后生效
              <NButton size="tiny" type="primary" @click="handleRestartApp">
                <template #icon><NIcon><RefreshOutline /></NIcon></template>
                立即重启
              </NButton>
            </div>

            <div v-if="kernelStatus && !kernelStatus.updatable" class="ab-line ab-muted">
              {{ kernelStatus.updatable_reason }}
            </div>

            <!-- 回滚：更新后起不来时的自救出口 -->
            <div v-if="kernelStatus?.backup_available" class="ab-line ab-line--warn ab-rollback">
              <span class="ab-dot ab-dot--warn"></span>
              <span>
                可回滚至
                <span class="ab-strong text-mono">
                  {{ kernelStatus.backup_version ? `v${kernelStatus.backup_version}` : "上一版内核" }}
                </span>
              </span>
              <NButton
                size="tiny"
                :loading="kernelRollingBack"
                :disabled="kernelRollingBack || !kernelStatus.updatable"
                @click="handleRollbackKernel"
              >
                回滚
              </NButton>
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
.ab-dot--warn { background: var(--warn); }

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
.ab-line--warn { color: var(--warn); }
.ab-muted { font-size: var(--fs-sm); color: var(--muted); }

/* 回滚提示：可回滚时右侧按钮需要被推到行尾 */
.ab-rollback {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}
.ab-rollback > span:nth-child(2) { flex: 1; min-width: 0; }

.ab-notes {
  margin: var(--sp-3) 0 0;
  padding: var(--sp-3) var(--sp-4);
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-sm);
  background: var(--surface-alt);
  font-size: var(--fs-sm);
  line-height: 1.7;
  color: var(--fg-2);
}

/* Markdown 渲染内容（内核与应用更新卡片的发布说明） */
.ab-notes.md-render > :first-child { margin-top: 0; }
.ab-notes.md-render > :last-child { margin-bottom: 0; }

.ab-notes.md-render h1,
.ab-notes.md-render h2 {
  margin: 1.2em 0 0.4em;
  font-size: 1rem;
  font-weight: 600;
  color: var(--fg);
}
.ab-notes.md-render h3,
.ab-notes.md-render h4 {
  margin: 1em 0 0.3em;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--fg);
}
.ab-notes.md-render p {
  margin: 0.5em 0;
}
.ab-notes.md-render ul,
.ab-notes.md-render ol {
  margin: 0.4em 0;
  padding-left: 1.5em;
}
.ab-notes.md-render li {
  margin: 0.2em 0;
}
.ab-notes.md-render code {
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--surface);
  font-family: var(--font-mono);
  font-size: 0.9em;
  color: var(--fg-2);
}
.ab-notes.md-render pre {
  margin: 0.6em 0;
  padding: var(--sp-2) var(--sp-3);
  border-radius: var(--radius-sm);
  background: var(--surface);
  overflow-x: auto;
}
.ab-notes.md-render pre code {
  padding: 0;
  background: transparent;
}
.ab-notes.md-render a {
  color: var(--accent);
  text-decoration: underline;
}
.ab-notes.md-render strong {
  font-weight: 600;
  color: var(--fg);
}
.ab-notes.md-render blockquote {
  margin: 0.5em 0;
  padding-left: var(--sp-3);
  border-left: 3px solid var(--border);
  color: var(--muted);
}
.ab-notes.md-render hr {
  margin: 1em 0;
  border: none;
  border-top: 1px solid var(--border-soft);
}
.ab-notes.md-render img {
  max-width: 100%;
  border-radius: var(--radius-sm);
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

/* 安装成功后的重启引导：与上方状态行留出间距，按钮紧跟文字 */
.ab-restart {
  margin-top: var(--sp-3);
  padding-top: var(--sp-3);
  border-top: 1px solid var(--border-soft);
}
</style>
