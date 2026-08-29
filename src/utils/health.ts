/**
 * 渠道健康状态展示工具
 *
 * Provider 的 health_status 取值：healthy / unhealthy / null（未检测）
 */

/** 健康状态对应的 NaiveUI Tag 类型 */
export function healthStatusType(
  status: string | null
): "success" | "error" | "warning" | "default" {
  if (status === "healthy") return "success";
  if (status === "unhealthy") return "error";
  return "warning";
}

/** 健康状态文案；从未检测过（null）显示「未知」 */
export function healthStatusLabel(status: string | null): string {
  if (status === "healthy") return "正常";
  if (status === "unhealthy") return "异常";
  return "未知";
}
