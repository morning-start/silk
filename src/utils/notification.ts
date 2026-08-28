import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

/**
 * 发送系统通知（自动请求权限）
 */
export async function notify(title: string, body?: string): Promise<void> {
  let permissionGranted = await isPermissionGranted();
  if (!permissionGranted) {
    const permission = await requestPermission();
    permissionGranted = permission === "granted";
  }
  if (permissionGranted) {
    await sendNotification({ title, body });
  }
}

/**
 * 网关事件通知
 */
export const gatewayNotifications = {
  started(address: string) {
    notify("网关已启动", `监听地址: ${address}`);
  },
  stopped() {
    notify("网关已停止");
  },
  startFailed(error: string) {
    notify("网关启动失败", error);
  },
  stopFailed(error: string) {
    notify("网关停止失败", error);
  },
};
