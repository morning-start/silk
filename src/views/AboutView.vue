<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  NAlert,
  NButton,
  NCard,
  NIcon,
  NProgress,
  NText,
  useMessage,
} from "naive-ui";
import {
  DownloadOutline,
  InformationCircleOutline,
  LogoGithub,
  PersonOutline,
  RefreshOutline,
  ShieldCheckmarkOutline,
} from "@vicons/ionicons5";
import {
  checkForUpdates,
  downloadAndInstall,
  type UpdateInfo,
} from "../utils/updater";

const REPO_URL = "https://github.com/morning-start/silk";
const AUTHOR_URL = "https://github.com/morning-start";

const message = useMessage();

const version = ref("");
const checking = ref(false);
const installing = ref(false);
const progress = ref(0);
const updateInfo = ref<UpdateInfo | null>(null);

const isLatest = computed(
  () => updateInfo.value !== null && !updateInfo.value.available && !updateInfo.value.error
);

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

async function handleCheckUpdate() {
  checking.value = true;
  updateInfo.value = null;
  try {
    updateInfo.value = await checkForUpdates();
  } finally {
    checking.value = false;
  }
}

async function handleInstall() {
  installing.value = true;
  progress.value = 0;
  try {
    const ok = await downloadAndInstall((p) => {
      progress.value = Math.round(p * 100);
    });
    if (ok) {
      message.success("更新已下载并安装，应用即将重启");
    } else {
      message.error("更新失败，请稍后重试");
    }
  } finally {
    installing.value = false;
  }
}

onMounted(async () => {
  version.value = await getVersion();
  // 静默检查一次，有新版本直接展示在页面中
  updateInfo.value = await checkForUpdates();
});
</script>

<template>
  <div class="about-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <div class="page-head">
          <h2 class="page-title">关于</h2>
          <p class="page-desc">版本信息、项目资料与软件更新</p>
        </div>
      </div>
    </div>

    <NCard :bordered="false" class="about-card" size="small" title="应用信息">
      <div class="about-hero">
        <div class="about-logo">S</div>
        <div class="about-name">
          <h3>Silk 丝路</h3>
          <p class="about-version">本地 AI 多模型中转网关 · v{{ version }}</p>
        </div>
      </div>
      <p class="about-intro">
        Silk 是运行在你桌面的 AI 多模型网关（Tauri 2 + Rust/Axum + Vue 3）。
        一个本地端点 <code>http://127.0.0.1:1877</code>，OpenAI Chat / Claude Messages / OpenAI Responses
        三大协议任意互转；上游超时自动重试、换 Key、换渠道；API Key 用 AES-GCM 加密存本地。
        纯本地运行，零云端上传数据。
      </p>
      <div class="about-items">
        <div class="about-item">
          <NIcon size="16" class="about-item-icon"><LogoGithub /></NIcon>
          <span class="about-item-label">仓库地址</span>
          <a href="#" @click.prevent="openLink(REPO_URL)">{{ REPO_URL }}</a>
        </div>
        <div class="about-item">
          <NIcon size="16" class="about-item-icon"><PersonOutline /></NIcon>
          <span class="about-item-label">作者简介</span>
          <a href="#" @click.prevent="openLink(AUTHOR_URL)">morning-start</a>
          <NText depth="3" class="about-item-hint">独立开发者，专注本地优先的 AI 工具链</NText>
        </div>
        <div class="about-item">
          <NIcon size="16" class="about-item-icon"><ShieldCheckmarkOutline /></NIcon>
          <span class="about-item-label">许可证</span>
          <span>AGPL-3.0</span>
        </div>
      </div>
    </NCard>

    <NCard :bordered="false" class="about-card" size="small" title="软件更新">
      <div class="update-hero">
        <NIcon size="20" class="update-icon"><InformationCircleOutline /></NIcon>
        <div>
          <div class="update-title">检查更新</div>
          <div class="update-desc">当前版本 v{{ version }}，更新包从 GitHub Releases 下载并自动安装。</div>
        </div>
        <NButton
          type="primary"
          size="small"
          :loading="checking"
          @click="handleCheckUpdate"
        >
          <template #icon><NIcon><RefreshOutline /></NIcon></template>
          检查更新
        </NButton>
      </div>

      <!-- 检查中 -->
      <div v-if="checking" class="update-status">
        <NText depth="3">正在检查更新…</NText>
      </div>

      <!-- 有新版本 -->
      <div v-else-if="updateInfo?.available" class="update-available">
        <NAlert type="info" :bordered="false" title="发现新版本" class="update-alert">
          新版本 <strong>v{{ updateInfo.version }}</strong>
          <span v-if="updateInfo.date">（发布于 {{ formatDate(updateInfo.date) }}）</span> 可以更新。
        </NAlert>
        <pre v-if="updateInfo.body" class="update-notes">{{ updateInfo.body }}</pre>
        <div v-if="installing" class="update-progress">
          <NProgress
            type="line"
            :percentage="progress"
            :height="8"
            :show-indicator="true"
          />
          <NText depth="3" style="font-size: 12px">
            {{ progress < 100 ? `正在下载更新… ${progress}%` : "下载完成，正在安装…" }}
          </NText>
        </div>
        <div class="update-actions">
          <NButton
            type="primary"
            size="small"
            :loading="installing"
            :disabled="installing"
            @click="handleInstall"
          >
            <template #icon><NIcon><DownloadOutline /></NIcon></template>
            {{ installing ? "更新中…" : "下载并安装" }}
          </NButton>
        </div>
      </div>

      <!-- 已是最新 -->
      <div v-else-if="isLatest" class="update-status">
        <NAlert type="success" :bordered="false" class="update-alert">
          已是最新版本（v{{ version }}）
        </NAlert>
      </div>

      <!-- 检查失败 -->
      <div v-else-if="updateInfo?.error" class="update-status">
        <NAlert type="warning" :bordered="false" class="update-alert" title="检查更新失败">
          无法连接到更新服务器，请检查网络后重试。
          <span style="font-size: 12px">（{{ updateInfo.error }}）</span>
        </NAlert>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.about-page {
  max-width: 760px;
}

.about-card {
  margin-bottom: 16px;
}

/* ===== 应用信息 ===== */
.about-card {
  position: relative;
  overflow: hidden;
}

.about-card::before {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0;
  height: 2px;
  background: var(--gradient, linear-gradient(90deg, #06b6d4, #6366f1));
  opacity: 0.6;
}

.about-hero {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 16px;
}

.about-logo {
  width: 56px;
  height: 56px;
  border-radius: 16px;
  background: var(--gradient, linear-gradient(135deg, #06b6d4, #6366f1));
  color: #fff;
  font-size: 28px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: var(--shadow-accent, 0 8px 20px -6px rgba(8, 145, 178, 0.4));
}

.about-name h3 {
  margin: 0 0 4px;
  font-size: 18px;
  font-weight: 600;
  color: var(--fg, #0f172a);
}

.about-version {
  margin: 0;
  font-size: 13px;
  color: var(--muted, #64748b);
}

.about-intro {
  margin: 0 0 16px;
  font-size: 13px;
  line-height: 1.8;
  color: var(--fg, #334155);
}

.about-intro code {
  background: var(--surface-alt, #f1f5f9);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
  color: var(--accent, #0891b2);
}

.about-items {
  border-top: 1px solid var(--border-soft, #e2e8f0);
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.about-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.about-item-icon {
  color: var(--muted, #64748b);
  flex-shrink: 0;
}

.about-item-label {
  color: var(--muted, #64748b);
  width: 72px;
  flex-shrink: 0;
}

.about-item a {
  color: var(--accent, #0891b2);
  text-decoration: none;
}

.about-item a:hover {
  text-decoration: underline;
}

.about-item-hint {
  margin-left: 8px;
  font-size: 12px;
}

/* ===== 软件更新 ===== */
.update-hero {
  display: flex;
  align-items: center;
  gap: 12px;
}

.update-icon {
  color: var(--accent, #0891b2);
  flex-shrink: 0;
}

.update-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--fg, #0f172a);
}

.update-desc {
  font-size: 12px;
  color: var(--muted, #64748b);
  margin-top: 2px;
}

.update-hero .n-button {
  margin-left: auto;
}

.update-status {
  margin-top: 16px;
}

.update-alert {
  margin-top: 16px;
}

.update-available .update-alert {
  margin-bottom: 12px;
}

.update-notes {
  margin: 0 0 12px;
  padding: 12px 14px;
  background: var(--surface-alt, #f1f5f9);
  border-radius: 8px;
  font-size: 12px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--fg, #334155);
  max-height: 220px;
  overflow-y: auto;
}

.update-progress {
  margin-bottom: 12px;
}

.update-actions {
  display: flex;
  gap: 8px;
}
</style>
