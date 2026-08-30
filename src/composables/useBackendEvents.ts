import { listen, type Event } from "@tauri-apps/api/event";
import { notifyDataChanged, type DataChangeEvent } from "./useCrossStoreNotify";

/**
 * 后端 → 前端 的数据变更桥接（数据流原则的 Push 机制）。
 *
 * 后端在写事务提交后 `emit("data-changed", { entity })`，
 * 这里在应用入口监听一次，把事件转发为前端既有的跨 Store 信号
 * （`notifyDataChanged`）。这样：
 *  - 所有 `useDataChangeSignal(...)` 订阅者（如 Dashboard 的 "providers"）自动失效并重拉；
 *  - 后端自主触发的数据变更（健康检查发现、网关重启等）也能送达前端；
 *  - 复用既有机制，无需每个视图单独监听 Tauri 事件。
 *
 * `entity` 取值需与后端 `change_events::emit_data_changed` 及前端
 * `DataChangeEvent` 对齐：providers / groups / gatewaySettings。
 */
let initPromise: Promise<void> | null = null;

export function initBackendEvents(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = listen<{ entity: string }>(
    "data-changed",
    (event: Event<{ entity: string }>) => {
      notifyDataChanged(event.payload.entity as DataChangeEvent);
    },
  ).then(() => undefined);
  return initPromise;
}
