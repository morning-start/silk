/** 将毫秒格式化为可读字符串：<1000 显示 ms，<60000 显示 s，否则显示 m */
export function formatMs(ms: number | null): string {
  if (ms == null) return "-";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}
