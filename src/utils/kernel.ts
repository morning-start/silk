import { kernelApi, type KernelInfo, type KernelInstallResult, type KernelStatus } from "../api/kernel";

// ---------------------------------------------------------------------------
// 协议内核更新
//
// 与 utils/updater.ts（应用更新）结构一致，但对象是 prism.wasm：
// 检查 → 下载 → 后端校验（sha256 + ABI 探测）→ 原子替换 → 提示重启。
//
// 内核是进程级单例（Rust 侧 LazyLock），换文件不会热生效，
// 因此安装成功后必须重启应用，界面据此引导。
// ---------------------------------------------------------------------------

/** 检查结果：`available` 为 false 且无 `error` 时表示内核已是最新 */
export interface KernelCheckResult {
  available: boolean;
  /** 当前已安装版本（无法反查时为 undefined） */
  currentVersion?: string;
  /** 最新版本（已去掉 `v` 前缀） */
  version?: string;
  date?: string;
  body?: string;
  releaseUrl?: string;
  assetSize?: number;
  /** 能否安装（程序目录抢占或发布缺资产时为 false） */
  canInstall: boolean;
  /** 不可安装的原因 */
  blockedReason?: string;
  /** 检查失败时的错误信息（区别于「无更新」） */
  error?: string;
}

/** 内核来源的可读名称 */
export function sourceLabel(source: string): string {
  switch (source) {
    case "exe_dir":
      return "程序目录";
    case "data_dir":
      return "应用数据目录";
    case "embedded":
      return "内嵌版本";
    default:
      return source;
  }
}

/** 体积格式化（KB / MB） */
export function formatSize(bytes?: number | null): string {
  if (!bytes || bytes <= 0) return "";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/**
 * 检查结果的短时缓存
 *
 * 与软件更新同理：GitHub 匿名调用限额为每小时 60 次（按 IP 计），
 * 而关于页每次进入都会静默检查。缓存 5 分钟避免来回切页面把额度耗光。
 */
const CACHE_TTL_MS = 5 * 60 * 1000;
let cached: { at: number; result: KernelCheckResult } | null = null;

/**
 * 检查内核更新
 *
 * @param force 跳过缓存强制请求（「检查更新」按钮用）
 */
export async function checkKernelUpdate(force = false): Promise<KernelCheckResult> {
  if (!force && cached && Date.now() - cached.at < CACHE_TTL_MS) {
    return cached.result;
  }

  let result: KernelCheckResult;
  try {
    result = toResult(await kernelApi.check());
  } catch (error) {
    console.error("检查内核更新失败:", error);
    return {
      available: false,
      canInstall: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  // 只缓存成功结果：失败多为网络或限额问题，应当允许立即重试
  cached = { at: Date.now(), result };
  return result;
}

function toResult(info: KernelInfo): KernelCheckResult {
  return {
    available: info.available,
    currentVersion: info.current_version ?? undefined,
    version: info.latest_version ?? undefined,
    date: info.published_at ?? undefined,
    body: info.notes ?? undefined,
    releaseUrl: info.release_url ?? undefined,
    assetSize: info.asset_size ?? undefined,
    canInstall: info.can_install,
    blockedReason: info.blocked_reason ?? undefined,
  };
}

/** 读取当前内核状态 */
export async function getKernelStatus(): Promise<KernelStatus> {
  return kernelApi.getStatus();
}

/**
 * 下载并安装最新内核
 *
 * 后端负责 sha256 校验与 ABI 探测，任一不通过都不会落盘（旧内核保持原样）。
 * 成功后需重启应用才生效。
 */
export async function installKernelUpdate(): Promise<KernelInstallResult> {
  return kernelApi.install();
}

/** 重启应用 */
export async function restartApp(): Promise<void> {
  return kernelApi.restart();
}
