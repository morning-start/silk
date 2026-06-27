use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 模型映射（模型广场）
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
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 创建模型映射的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewModelMapping {
    pub model_name: String,
    pub provider_group_id: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// 更新模型映射的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateModelMapping {
    pub model_name: Option<String>,
    pub provider_group_id: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Option<Vec<String>>,
    pub enabled: Option<bool>,
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
