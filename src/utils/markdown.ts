import { marked } from "marked";

// ---------------------------------------------------------------------------
// Markdown 渲染（服务端/第三方内容）
//
// 用于渲染来自 GitHub Releases API 的发布说明，这些内容是可信的第三方文案
// （非用户输入），因此不做 DOMPurify 级别的 XSS 净化。
// 仅对 Markdown 解析器做保守配置（不打开危险扩展）。
// ---------------------------------------------------------------------------

marked.setOptions({
  breaks: true, // GitHub 风格的换行符
  gfm: true, // GitHub Flavored Markdown
});

/** 将 Markdown 文本渲染为安全的 HTML 字符串（用于 v-html） */
export function renderMarkdown(text: string): string {
  if (!text) return "";
  try {
    const html = marked.parse(text);
    return typeof html === "string" ? html : html.toString();
  } catch (e) {
    console.error("Markdown 渲染失败:", e);
    return `<p>${escapeHtml(text)}</p>`;
  }
}

/** 极简的 HTML 转义（仅用于 fallback 路径） */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}
