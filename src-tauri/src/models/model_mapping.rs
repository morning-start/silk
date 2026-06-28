use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 模型映射（模型池）
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ModelMapping {
    pub id: String,
    pub model_name: String,
    pub provider_group_id: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    /// 能力标签 JSON 数组
    pub capabilities: String,
    pub description: String,
    pub vendor: String,
    pub knowledge_cutoff: Option<String>,
    pub model_family: String,
    pub reference_url: Option<String>,
    /// 负载均衡策略
    pub strategy: String,
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 创建模型映射的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewModelMapping {
    pub model_name: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    /// 关联的渠道列表
    pub channels: Option<Vec<NewMappingChannel>>,
}

/// 更新模型映射的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateModelMapping {
    pub model_name: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    /// 关联的渠道列表（替换旧列表）
    pub channels: Option<Vec<NewMappingChannel>>,
}

/// 新建/更新时提交的关联渠道
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMappingChannel {
    pub provider_id: String,
    /// 该渠道选中的远程模型名列表（空数组 = 使用 mapping 的 model_name）
    pub selected_models: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// 模型映射关联渠道（DB 行）
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ModelMappingChannel {
    pub id: String,
    pub mapping_id: String,
    pub provider_id: String,
    /// JSON 数组
    pub selected_models: String,
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
}

impl ModelMappingChannel {
    pub fn selected_models_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.selected_models).unwrap_or_default()
    }
}

/// 关联渠道 + 渠道详情（用于响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingChannelInfo {
    pub id: String,
    pub mapping_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub provider_protocols: Vec<String>,
    pub provider_models: Vec<String>, // 渠道的全部模型
    pub provider_models_count: i64,
    pub provider_health: Option<String>,
    /// 该渠道选中的远程模型名列表
    pub selected_models: Vec<String>,
    pub enabled: bool,
}

/// 分组内渠道概要信息（废弃，保留兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupProviderInfo {
    pub id: String,
    pub name: String,
    pub protocols: Vec<String>,
    pub models_count: i64,
    pub health_status: Option<String>,
}

impl ModelMapping {
    /// 解析能力标签为 Vec
    pub fn capabilities_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.capabilities).unwrap_or_default()
    }

    /// 获取能力标签显示名称
    pub fn capability_labels(&self) -> Vec<(&str, &str)> {
        self.capabilities_vec()
            .iter()
            .filter_map(|c| match c.as_str() {
                "thinking" => Some(("思考", "think")),
                "image" => Some(("识图", "image")),
                "text" => Some(("文本", "text")),
                "draw" => Some(("生图", "draw")),
                "code" => Some(("代码", "code")),
                "audio" => Some(("语音", "audio")),
                _ => None,
            })
            .collect()
    }
}
