import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 协议内核（prism.wasm）下载与更新
// 对应后端 `commands::kernel`
//
// 与「软件更新」（api/updater.ts）的分工：
// - 软件更新管 silk 应用本身（GitHub Releases 的安装包）；
// - 本模块管协议转换内核 prism.wasm（GitHub Releases 的 wasm 产物）。
// 两者都走 GitHub Releases 并沿用全局代理，但比对对象与资产形态不同。
// ---------------------------------------------------------------------------

/** 当前内核状态，字段与后端 `KernelStatus` 一一对应 */
export interface KernelStatus {
  /** 已安装内核版本（仅数据目录来源可反查，其余为 null） */
  version: string | null;
  /** 当前运行内核的 ABI（内嵌旧内核无此导出时为 null） */
  abi: string | null;
  /** 当前运行内核的 IR schema 版本 */
  ir_schema: string | null;
  /** 内核来源：exe_dir / data_dir / embedded */
  source: string;
  /** 实际加载路径（内嵌来源为 null） */
  path: string | null;
  /** 宿主支持的 ABI */
  supported_abi: string;
  /** 是否可被本功能更新（程序目录抢占时为 false） */
  updatable: boolean;
  /** 不可更新时的原因说明 */
  updatable_reason: string | null;
}

/** 内核更新检查结果，字段与后端 `KernelInfo` 一一对应 */
export interface KernelInfo {
  current_version: string | null;
  latest_version: string | null;
  available: boolean;
  name: string | null;
  notes: string | null;
  published_at: string | null;
  release_url: string | null;
  asset_size: number | null;
  can_install: boolean;
  blocked_reason: string | null;
}

/** 安装结果，字段与后端 `KernelInstallResult` 一一对应 */
export interface KernelInstallResult {
  version: string | null;
  abi: string;
  source: string;
  /** 内核是进程级单例，安装后恒需重启才生效 */
  requires_restart: boolean;
}

export const kernelApi = {
  /** 读取当前内核状态（版本 / ABI / 来源） */
  getStatus: (): Promise<KernelStatus> => invoke<KernelStatus>("get_kernel_status"),

  /** 检查内核更新（后端自动使用配置的全局代理） */
  check: (): Promise<KernelInfo> => invoke<KernelInfo>("check_kernel_update"),

  /** 下载并安装最新内核（sha256 校验 + ABI 探测通过后才落盘） */
  install: (): Promise<KernelInstallResult> =>
    invoke<KernelInstallResult>("install_kernel_update"),

  /** 重启应用（内核更新后生效用） */
  restart: (): Promise<void> => invoke<void>("restart_app"),
};
