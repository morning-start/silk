import { ref } from "vue";

/**
 * 轻量级跨 Store 事件通知机制
 *
 * 用于解耦 Store 间的间接依赖关系。例如：
 * - 路由规则/渠道变更 → Dashboard 自动刷新统计
 * - 渠道变更 → 模型广场视图刷新
 *
 * 使用方式：
 * ```ts
 * // 发送方（如 providers store）
 * import { notifyDataChanged } from "../composables/useCrossStoreNotify";
 * notifyDataChanged("providers");
 *
 * // 接收方（如 Dashboard view）
 * import { onDataChanged } from "../composables/useCrossStoreNotify";
 * onMounted(() => {
 *   watch(onDataChanged("providers"), () => loadData());
 * });
 * ```
 */

/** 已支持的通知事件类型 */
export type DataChangeEvent = "providers" | "routingRules" | "groups" | "gatewaySettings";

const _eventVersion = ref<Record<string, number>>({});

/**
 * 通知其他 Store/View：指定类型的数据已变更
 */
export function notifyDataChanged(type: DataChangeEvent) {
  _eventVersion.value[type] = (_eventVersion.value[type] ?? 0) + 1;
  // 触发响应式更新（ref 替换触发 watch）
  _eventVersion.value = { ..._eventVersion.value };
}

/**
 * 监听指定类型的数据变更
 * 返回一个数字，每次数据变更时递增，
 * 可与 Vue `watch()` 配合使用
 */
export function useDataChangeSignal(type: DataChangeEvent) {
  return () => _eventVersion.value[type] ?? 0;
}
