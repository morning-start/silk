import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 软件更新：GitHub Releases REST API
// 对应后端 `commands::updater`
//
// 与 @tauri-apps/plugin-updater 的分工：
// - 本模块只读发布元数据，回答「有没有新版本 / 新版本是什么 / 去哪下载」；
// - 插件负责静默下载安装，需要发布里同时存在 latest.json 与 .sig，
//   缺失时 auto_update_ready 为 false，前端降级为打开下载页。
// ---------------------------------------------------------------------------

/** 更新检查结果，字段与后端 `UpdateInfo` 一一对应 */
export interface AppUpdateInfo {
  /** 当前运行版本 */
  current_version: string;
  /** 最新发布版本（已去掉 `v` 前缀）；仓库无正式发布时为 null */
  latest_version: string | null;
  /** 是否有比当前更新的正式发布 */
  available: boolean;
  /** 发布标题 */
  name: string | null;
  /** 发布说明（Markdown） */
  notes: string | null;
  /** 发布时间（ISO 8601） */
  published_at: string | null;
  /** 发布页地址，可在浏览器打开 */
  release_url: string | null;
  /** 当前平台安装包文件名 */
  asset_name: string | null;
  /** 当前平台安装包下载地址 */
  download_url: string | null;
  /** 发布里是否带 latest.json，即能否静默自动安装 */
  auto_update_ready: boolean;
}

export const updaterApi = {
  /** 检查软件更新（后端自动使用配置的全局代理） */
  checkAppUpdate: (): Promise<AppUpdateInfo> =>
    invoke<AppUpdateInfo>("check_app_update"),
};
