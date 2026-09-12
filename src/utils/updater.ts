import { check, type Update } from "@tauri-apps/plugin-updater";
import { openUrl } from "@tauri-apps/plugin-opener";
import { updaterApi, type AppUpdateInfo } from "../api/updater";

// ---------------------------------------------------------------------------
// 软件更新
//
// 分两层：
// 1. 「有没有新版本」走 GitHub Releases REST API（本文件的 checkForUpdates），
//    只需网络可达，不依赖发布里是否带签名产物 —— 这是唯一可靠的检查途径。
// 2. 「下载并安装」优先走 tauri-plugin-updater 静默安装（要求发布带
//    latest.json + .sig）；缺少签名产物时降级为浏览器下载安装包。
//
// 之所以不再用插件的 check() 做版本检查：插件的 endpoint 指向
// `releases/latest/download/latest.json`，一旦发布缺该文件就是 404，
// 会被当成「检查失败」，而不是「已是最新版本」。
// ---------------------------------------------------------------------------

/** 检查结果：`available` 为 false 且无 `error` 时表示确已是最新版本 */
export interface UpdateCheckResult {
  available: boolean;
  /** 最新版本号（已去掉 `v` 前缀） */
  version?: string;
  /** 发布时间（ISO 8601） */
  date?: string;
  /** 发布说明（Markdown） */
  body?: string;
  /** 发布页地址 */
  releaseUrl?: string;
  /** 当前平台安装包下载地址 */
  downloadUrl?: string;
  /** 当前平台安装包文件名 */
  assetName?: string;
  /** 能否静默自动安装（发布带 latest.json 时为 true） */
  autoUpdateReady?: boolean;
  /** 检查失败时的错误信息（区别于「无更新」） */
  error?: string;
}

/** 兼容旧命名的类型别名 */
export type UpdateInfo = UpdateCheckResult;

/** 安装结果：区分「已静默安装」与「已转为浏览器下载」 */
export type InstallOutcome = "installed" | "opened-download" | "failed";

/**
 * 检查结果的短时缓存
 *
 * GitHub 匿名调用限额为每小时 60 次（按 IP 计），而关于页每次进入都会静默检查一次。
 * 缓存 5 分钟可以避免来回切页面把额度耗光 —— 额度耗尽会返回 403，
 * 用户看到的就是「检查更新失败」。
 */
const CACHE_TTL_MS = 5 * 60 * 1000;
let cached: { at: number; result: UpdateCheckResult } | null = null;

/**
 * 检查应用更新
 *
 * 通过后端调用 GitHub Releases REST API；后端会沿用「设置 → 请求与限流」
 * 里配置的全局代理，因此国内网络在配好代理后可正常检查。
 *
 * @param force 跳过缓存强制请求（「检查更新」按钮用）
 */
export async function checkForUpdates(force = false): Promise<UpdateCheckResult> {
  if (!force && cached && Date.now() - cached.at < CACHE_TTL_MS) {
    return cached.result;
  }

  let result: UpdateCheckResult;
  try {
    result = toResult(await updaterApi.checkAppUpdate());
  } catch (error) {
    console.error("检查更新失败:", error);
    return {
      available: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  // 只缓存成功结果：失败多为网络或限额问题，应当允许立即重试
  cached = { at: Date.now(), result };
  return result;
}

function toResult(info: AppUpdateInfo): UpdateCheckResult {
  return {
    available: info.available,
    version: info.latest_version ?? undefined,
    date: info.published_at ?? undefined,
    body: info.notes ?? undefined,
    releaseUrl: info.release_url ?? undefined,
    downloadUrl: info.download_url ?? undefined,
    assetName: info.asset_name ?? undefined,
    autoUpdateReady: info.auto_update_ready,
  };
}

/** 调起插件做静默安装；返回 false 表示当前发布无法静默安装 */
async function trySilentInstall(
  onProgress?: (progress: number) => void
): Promise<boolean> {
  let update: Update | null = null;
  try {
    update = await check({ timeout: 15000 });
  } catch (error) {
    // 发布缺 latest.json/.sig 时会在这里抛错，属于预期内的降级场景
    console.warn("静默安装不可用，将降级为浏览器下载:", error);
    return false;
  }

  if (!update) return false;

  let downloaded = 0;
  let contentLength = 0;

  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        downloaded = 0;
        contentLength = event.data.contentLength ?? 0;
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        if (contentLength > 0 && onProgress) {
          onProgress(Math.min(downloaded / contentLength, 1));
        }
        break;
      case "Finished":
        onProgress?.(1);
        break;
    }
  });

  return true;
}

/**
 * 下载并安装更新
 *
 * 优先静默安装；当前发布没有签名产物时打开安装包下载页，由用户手动安装。
 *
 * @param onProgress 进度回调（0-1），仅在静默安装时触发
 * @param fallback 静态检查结果，用于在静默安装不可用时获取下载地址
 */
export async function downloadAndInstall(
  onProgress?: (progress: number) => void,
  fallback?: UpdateCheckResult
): Promise<InstallOutcome> {
  try {
    if (fallback?.autoUpdateReady !== false) {
      if (await trySilentInstall(onProgress)) return "installed";
    }

    // 降级：直接下载当前平台安装包，比让用户自己在发布页里挑更省事
    const target = fallback?.downloadUrl ?? fallback?.releaseUrl;
    if (target) {
      await openUrl(target);
      return "opened-download";
    }

    console.error("下载更新失败：没有可用的下载地址");
    return "failed";
  } catch (error) {
    console.error("下载更新失败:", error);
    return "failed";
  }
}
