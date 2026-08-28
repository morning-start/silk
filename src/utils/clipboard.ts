import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";

/**
 * 复制文本到剪贴板
 */
export async function copyToClipboard(text: string): Promise<void> {
  await writeText(text);
}

/**
 * 从剪贴板读取文本
 */
export async function pasteFromClipboard(): Promise<string> {
  return await readText();
}

/**
 * 复制并显示成功提示（配合 naive-ui message 使用）
 */
export async function copyWithFeedback(
  text: string,
  _label?: string
): Promise<boolean> {
  try {
    await copyToClipboard(text);
    return true;
  } catch (error) {
    console.error("复制失败:", error);
    return false;
  }
}
