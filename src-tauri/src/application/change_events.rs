use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// 跨进程「数据已变更」事件名。
///
/// 前后端共享的契约：后端在写事务提交后 `emit`，前端 `listen` 后失效对应缓存并重拉。
/// 事件名必须与前端的 `listen("data-changed")` 完全一致。
pub const DATA_CHANGED_EVENT: &str = "data-changed";

/// 事件载荷。`entity` 取值需与前端 `DataChangeEvent` 对齐：
/// `providers` / `groups` / `gatewaySettings`（分别对应渠道、模型映射、网关设置）。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedPayload {
    pub entity: String,
}

/// 在「写事务提交之后」调用，通知前端某类领域数据已变更。
///
/// 这是数据流原则（后端单一数据源 + 事件驱动失效）的 Push 机制：
/// 前端收到后失效本地缓存并重拉，保证多视图与后端一致，且不依赖前端本地另存一份真相。
///
/// `emit` 失败（如窗口已关闭）不应影响主流程，故忽略其返回值。
pub fn emit_data_changed(app: &AppHandle, entity: &str) {
    let _ = app.emit(
        DATA_CHANGED_EVENT,
        DataChangedPayload {
            entity: entity.to_string(),
        },
    );
}
