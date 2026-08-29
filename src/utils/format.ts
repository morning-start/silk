/** 将毫秒格式化为可读字符串：<1000 显示 ms，<60000 显示 s，否则显示 m */
export function formatMs(ms: number | null): string {
  if (ms == null) return "-";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}

/** 格式化每百万 token 的价格 */
export function formatPrice(val: number | null): string {
  if (val == null) return "-";
  return `$${val}/1M`;
}

/** 格式化 token 数量：>=1000 折算为 K */
export function formatTokens(val: number | null): string {
  if (val == null) return "-";
  if (val >= 1000) return `${val / 1000}K`;
  return `${val}`;
}
