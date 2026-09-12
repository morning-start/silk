use serde::{Deserialize, Serialize};

use crate::error::{bad_request, require_db, require_found, validate_non_empty, ServiceError};
use crate::impl_crud_delete;
use crate::models::{
    MappingChannelInfo, ModelMapping, NewMappingChannel, NewModelMapping, UpdateModelMapping,
};
use crate::persistence::ModelMappingRepo;

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ModelMappingResponse {
    pub id: String,
    pub model_name: String,
    pub strategy: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub capabilities: Vec<String>,
    pub description: String,
    pub enabled: bool,
    pub channels: Vec<MappingChannelInfo>,
    pub created_at: String,
    pub updated_at: String,
    /// 是否支持思考（来自内置模型目录补充，DB 无此字段）
    pub reasoning: Option<bool>,
    /// 支持的输入类型（来自内置模型目录补充，DB 无此字段）
    pub input_types: Vec<String>,
}

impl ModelMappingResponse {
    pub fn from_model(m: ModelMapping, channels: Vec<MappingChannelInfo>) -> Self {
        // 内置模型目录补充：仅模型池（model_mappings）响应，渠道模型不受影响。
        // 合并规则：DB 字段有值优先，空值/缺省从目录（按 model_name 精确匹配）补。
        let entry = crate::application::model_catalog::get_entry(&m.model_name);
        Self::merge(m, entry.as_ref(), channels)
    }

    /// 目录合并（纯函数，便于单测；entry 为 None 时行为与旧版一致）
    fn merge(
        m: ModelMapping,
        entry: Option<&crate::application::model_catalog::ModelCatalogEntry>,
        channels: Vec<MappingChannelInfo>,
    ) -> Self {
        let capabilities = m.capabilities_vec();
        let description = m.description;
        Self {
            id: m.id,
            model_name: m.model_name,
            strategy: m.strategy,
            max_input_tokens: m
                .max_input_tokens
                .or_else(|| entry.and_then(|e| e.max_input_tokens)),
            max_context_tokens: m
                .max_context_tokens
                .or_else(|| entry.and_then(|e| e.max_context_tokens)),
            max_output_tokens: m
                .max_output_tokens
                .or_else(|| entry.and_then(|e| e.max_output_tokens)),
            capabilities: if capabilities.is_empty() {
                entry
                    .map(|e| e.capabilities.clone())
                    .unwrap_or_default()
            } else {
                capabilities
            },
            description: if description.is_empty() {
                entry.map(|e| e.description.clone()).unwrap_or_default()
            } else {
                description
            },
            enabled: m.enabled != 0,
            channels,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
            reasoning: entry.and_then(|e| e.reasoning),
            input_types: entry.map(|e| e.input_types.clone()).unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Payload Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateModelMappingPayload {
    pub model_name: String,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    pub channels: Option<Vec<NewMappingChannel>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelMappingPayload {
    pub model_name: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub capabilities: Option<Vec<String>>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
    pub channels: Option<Vec<NewMappingChannel>>,
}

// ---------------------------------------------------------------------------
// CRUD（delete 由宏生成，list / get / create / update 手写）
// ---------------------------------------------------------------------------

impl_crud_delete!(ModelMappingRepo);

/// 查询所有模型映射
pub async fn list() -> Result<Vec<ModelMappingResponse>, ServiceError> {
    let pool = require_db()?;
    let mappings = ModelMappingRepo::find_all(pool).await?;
    let mut result = Vec::with_capacity(mappings.len());
    for m in mappings {
        let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &m.id)
            .await
            .unwrap_or_default();
        result.push(ModelMappingResponse::from_model(m, channels));
    }
    Ok(result)
}

/// 根据 ID 查询模型映射
pub async fn get(id: String) -> Result<ModelMappingResponse, ServiceError> {
    let pool = require_db()?;
    let mapping = require_found(ModelMappingRepo::find_by_id(pool, &id).await?, "模型映射")?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 根据模型名称查询
pub async fn find_by_name(
    model_name: String,
) -> Result<Option<ModelMappingResponse>, ServiceError> {
    let pool = require_db()?;
    let mapping = match ModelMappingRepo::find_by_model_name(pool, &model_name).await? {
        Some(m) => m,
        None => return Ok(None),
    };
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(Some(ModelMappingResponse::from_model(mapping, channels)))
}

/// 创建模型映射
pub async fn create(
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, ServiceError> {
    let pool = require_db()?;
    validate_create_payload(&payload)?;

    let new = NewModelMapping {
        model_name: payload.model_name.trim().to_string(),
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        capabilities: payload.capabilities,
        description: payload.description,
        strategy: payload.strategy,
        enabled: payload.enabled,
        channels: payload.channels,
    };
    let mapping = ModelMappingRepo::create(pool, &new).await?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 更新模型映射
pub async fn update(
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, ServiceError> {
    let pool = require_db()?;
    validate_update_payload(&payload)?;

    let update = UpdateModelMapping {
        model_name: payload.model_name.map(|name| name.trim().to_string()),
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        capabilities: payload.capabilities,
        description: payload.description,
        strategy: payload.strategy,
        enabled: payload.enabled,
        channels: payload.channels,
    };
    let mapping = require_found(
        ModelMappingRepo::update(pool, &id, &update).await?,
        "模型映射",
    )?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

fn validate_create_payload(payload: &CreateModelMappingPayload) -> Result<(), ServiceError> {
    validate_non_empty("模型池名称", &payload.model_name)?;
    validate_strategy(payload.strategy.as_deref())?;
    validate_positive_i64(payload.max_input_tokens, "最大输入 Token")?;
    validate_positive_i64(payload.max_context_tokens, "最大上下文 Token")?;
    validate_positive_i64(payload.max_output_tokens, "最大输出 Token")?;
    validate_channels(payload.channels.as_deref())?;
    Ok(())
}

fn validate_update_payload(payload: &UpdateModelMappingPayload) -> Result<(), ServiceError> {
    if let Some(name) = &payload.model_name {
        validate_non_empty("模型池名称", name)?;
    }
    validate_strategy(payload.strategy.as_deref())?;
    validate_positive_i64(payload.max_input_tokens, "最大输入 Token")?;
    validate_positive_i64(payload.max_context_tokens, "最大上下文 Token")?;
    validate_positive_i64(payload.max_output_tokens, "最大输出 Token")?;
    validate_channels(payload.channels.as_deref())?;
    Ok(())
}

fn validate_channels(channels: Option<&[NewMappingChannel]>) -> Result<(), ServiceError> {
    if let Some(channels) = channels {
        if channels.is_empty() {
            return bad_request("模型池至少需要一个渠道");
        }
        if channels
            .iter()
            .any(|channel| channel.provider_id.trim().is_empty())
        {
            return bad_request("模型池渠道不能为空");
        }
        if channels.iter().any(|channel| {
            channel
                .selected_models
                .as_ref()
                .map(|models| models.iter().any(|model| model.name.trim().is_empty()))
                .unwrap_or(false)
        }) {
            return bad_request("模型池渠道模型名不能为空");
        }
        if channels.iter().any(|channel| {
            channel
                .selected_models
                .as_ref()
                .map(|models| models.iter().any(|model| model.weight <= 0))
                .unwrap_or(false)
        }) {
            return bad_request("模型池渠道模型权重必须大于 0");
        }
    }
    Ok(())
}

fn validate_strategy(strategy: Option<&str>) -> Result<(), ServiceError> {
    if let Some(strategy) = strategy {
        if !matches!(
            strategy,
            "round_robin" | "weighted" | "least_conn"
        ) {
            return bad_request("模型池策略无效");
        }
    }
    Ok(())
}

fn validate_positive_i64(value: Option<i64>, field: &str) -> Result<(), ServiceError> {
    if let Some(value) = value {
        if value <= 0 {
            return bad_request(&format!("{field}必须大于 0"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::models::SelectedModel;

    #[test]
    fn validate_mapping_rejects_empty_name_and_channels() {
        let mut payload = valid_create_payload();
        payload.model_name = " ".to_string();
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.channels = Some(Vec::new());
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.channels = Some(vec![NewMappingChannel {
            provider_id: " ".to_string(),
            selected_models: Some(vec![SelectedModel { name: "gpt-4o".to_string(), weight: 1 }]),
            enabled: Some(true),
        }]);
        assert_bad_request(validate_create_payload(&payload));
    }

    #[test]
    fn validate_mapping_rejects_invalid_numbers_and_strategy() {
        let mut payload = valid_create_payload();
        payload.max_input_tokens = Some(0);
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.strategy = Some("random".to_string());
        assert_bad_request(validate_create_payload(&payload));
    }

    #[test]
    fn validate_mapping_rejects_empty_selected_model() {
        let mut payload = valid_create_payload();
        payload.channels = Some(vec![NewMappingChannel {
            provider_id: "provider-1".to_string(),
            selected_models: Some(vec![SelectedModel { name: " ".to_string(), weight: 1 }]),
            enabled: Some(true),
        }]);
        assert_bad_request(validate_create_payload(&payload));
    }

    #[test]
    fn validate_mapping_accepts_valid_create_payload() {
        validate_create_payload(&valid_create_payload()).expect("valid mapping");
    }

    fn valid_create_payload() -> CreateModelMappingPayload {
        CreateModelMappingPayload {
            model_name: "gpt-4o".to_string(),
            max_input_tokens: Some(128000),
            max_context_tokens: Some(128000),
            max_output_tokens: Some(4096),
            capabilities: Some(vec!["vision".to_string()]),
            description: Some("test".to_string()),
            strategy: Some("round_robin".to_string()),
            enabled: Some(true),
            channels: Some(vec![NewMappingChannel {
                provider_id: "provider-1".to_string(),
                selected_models: Some(vec![SelectedModel { name: "gpt-4o".to_string(), weight: 1 }]),
                enabled: Some(true),
            }]),
        }
    }

    fn assert_bad_request(result: Result<(), ServiceError>) {
        assert!(matches!(result, Err(ServiceError::BadRequest { .. })));
    }
}

#[cfg(test)]
mod catalog_merge_tests {
    use super::*;
    use crate::application::model_catalog::ModelCatalog;
    use crate::models::ModelMapping;

    /// 构造一个仅 model_name 有值、其余元数据全空的模型映射（模拟用户建池时未填元数据）
    fn empty_mapping(model_name: &str) -> ModelMapping {
        ModelMapping {
            id: format!("mapping-{model_name}"),
            model_name: model_name.to_string(),
            max_input_tokens: None,
            max_context_tokens: None,
            max_output_tokens: None,
            capabilities: "[]".to_string(),
            description: String::new(),
            vendor: String::new(),
            knowledge_cutoff: None,
            model_family: String::new(),
            reference_url: None,
            strategy: "round_robin".to_string(),
            enabled: 1,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
        }
    }

    /// 从 JSON 片段解析出单条目录（纯数据，不触全局 OnceLock，测试可并行）
    fn entry_from(json: &str) -> Option<crate::application::model_catalog::ModelCatalogEntry> {
        let file: crate::application::model_catalog::ModelCatalogFile =
            serde_json::from_str(json).expect("测试 JSON 合法");
        let catalog = ModelCatalog::load_from_file(file);
        catalog.get("gpt-4o").cloned()
    }

    #[test]
    fn catalog_fills_empty_db_fields() {
        let entry = entry_from(
            r#"{
                "version": 1,
                "models": [
                    {
                        "model_name": "gpt-4o",
                        "max_context_tokens": 128000,
                        "capabilities": ["chat", "vision"],
                        "description": "OpenAI 旗舰",
                        "reasoning": false,
                        "input_types": ["text", "image"]
                    }
                ]
            }"#,
        );
        let resp = ModelMappingResponse::merge(empty_mapping("gpt-4o"), entry.as_ref(), Vec::new());
        assert_eq!(resp.max_context_tokens, Some(128000));
        assert_eq!(resp.capabilities, vec!["chat", "vision"]);
        assert_eq!(resp.description, "OpenAI 旗舰");
        assert_eq!(resp.reasoning, Some(false));
        assert_eq!(resp.input_types, vec!["text", "image"]);
    }

    #[test]
    fn db_values_take_precedence_over_catalog() {
        let entry = entry_from(
            r#"{
                "version": 1,
                "models": [
                    {
                        "model_name": "gpt-4o",
                        "max_context_tokens": 128000,
                        "capabilities": ["vision"],
                        "description": "目录描述"
                    }
                ]
            }"#,
        );
        let mut mapping = empty_mapping("gpt-4o");
        mapping.max_context_tokens = Some(200000);
        mapping.capabilities = "[\"chat\"]".to_string();
        mapping.description = "用户自定义".to_string();

        let resp = ModelMappingResponse::merge(mapping, entry.as_ref(), Vec::new());
        assert_eq!(resp.max_context_tokens, Some(200000));
        assert_eq!(resp.capabilities, vec!["chat"]);
        assert_eq!(resp.description, "用户自定义");
    }

    #[test]
    fn unknown_model_keeps_empty_fields_without_error() {
        let resp =
            ModelMappingResponse::merge(empty_mapping("unknown-model"), None, Vec::new());
        assert_eq!(resp.max_context_tokens, None);
        assert!(resp.capabilities.is_empty());
        assert_eq!(resp.reasoning, None);
        assert!(resp.input_types.is_empty());
    }
}
