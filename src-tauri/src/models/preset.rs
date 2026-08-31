use serde::{Deserialize, Serialize};

/// Preset（预设）—— 对齐 cc-switch 的 Provider 概念（SSOT：DB 为准，
/// 切换时由 harness writer 投影到 live 配置文件）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    /// 供应商配置快照（按 agent 格式，切换时注入网关信息后写 live）
    pub settings_config: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// 新建 Preset 载荷（对齐 cc-switch `with_id` + category/sort_index）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPreset {
    pub name: String,
    pub agent_type: String,
    pub settings_config: serde_json::Value,
    pub category: Option<String>,
    pub notes: Option<String>,
    pub sort_index: Option<i64>,
}

/// 更新 Preset 载荷（字段级更新）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePreset {
    pub name: Option<String>,
    pub settings_config: Option<serde_json::Value>,
    pub category: Option<String>,
    pub notes: Option<String>,
    pub sort_index: Option<i64>,
}
