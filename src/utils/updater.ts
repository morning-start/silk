import {
  check,
} from "@tauri-apps/plugin-updater";

export interface UpdateInfo {
  available: boolean;
  version?: string;
  date?: string;
  body?: string;
}

/**
 * 检查应用更新
 */
export async function checkForUpdates(): Promise<UpdateInfo> {
  try {
    const update = await check();
    if (update) {
      return {
        available: true,
        version: update.version,
        date: update.date,
        body: update.body,
      };
    }
    return { available: false };
  } catch (error) {
    console.error("检查更新失败:", error);
    return { available: false };
  }
}

/**
 * 下载并安装更新
 * @param onProgress 进度回调（0-1）
 */
export async function downloadAndInstall(
  onProgress?: (progress: number) => void
): Promise<boolean> {
  try {
    const update = await check();
    if (!update) return false;

    let downloaded = 0;
    let contentLength = 0;

    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          contentLength = event.data.contentLength ?? 0;
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          if (contentLength > 0 && onProgress) {
            onProgress(downloaded / contentLength);
          }
          break;
        case "Finished":
          if (onProgress) onProgress(1);
          break;
      }
    });

    return true;
  } catch (error) {
    console.error("下载更新失败:", error);
    return false;
  }
}
