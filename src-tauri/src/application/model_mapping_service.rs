use serde::{Deserialize, Serialize};

use crate::error::{require_db, require_found, ServiceError};
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
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Vec<String>,
    pub description: String,
    pub enabled: bool,
    pub channels: Vec<MappingChannelInfo>,
    pub created_at: String,
    pub updated_at: String,
}

impl ModelMappingResponse {
    pub fn from_model(m: ModelMapping, channels: Vec<MappingChannelInfo>) -> Self {
        let capabilities = m.capabilities_vec();
        Self {
            id: m.id,
            model_name: m.model_name,
            strategy: m.strategy,
            max_input_tokens: m.max_input_tokens,
            max_context_tokens: m.max_context_tokens,
            max_output_tokens: m.max_output_tokens,
            input_price_per_1m: m.input_price_per_1m,
            output_price_per_1m: m.output_price_per_1m,
            capabilities,
            description: m.description,
            enabled: m.enabled != 0,
            channels,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
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
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
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
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
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
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
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
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
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
    validate_non_negative_f64(payload.input_price_per_1m, "输入价格")?;
    validate_non_negative_f64(payload.output_price_per_1m, "输出价格")?;
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
    validate_non_negative_f64(payload.input_price_per_1m, "输入价格")?;
    validate_non_negative_f64(payload.output_price_per_1m, "输出价格")?;
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
                .map(|models| models.iter().any(|model| model.trim().is_empty()))
                .unwrap_or(false)
        }) {
            return bad_request("模型池渠道模型名不能为空");
        }
    }
    Ok(())
}

fn validate_strategy(strategy: Option<&str>) -> Result<(), ServiceError> {
    if let Some(strategy) = strategy {
        if !matches!(
            strategy,
            "round_robin" | "weighted" | "least_conn" | "failover"
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

fn validate_non_negative_f64(value: Option<f64>, field: &str) -> Result<(), ServiceError> {
    if let Some(value) = value {
        if !value.is_finite() || value < 0.0 {
            return bad_request(&format!("{field}不能为负数"));
        }
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ServiceError> {
    if value.trim().is_empty() {
        return bad_request(&format!("{field}不能为空"));
    }
    Ok(())
}

fn bad_request<T>(message: &str) -> Result<T, ServiceError> {
    Err(ServiceError::BadRequest {
        message: message.to_string(),
        code: None,
    })
}

#[cfg(test)]
mod validation_tests {
    use super::*;

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
            selected_models: Some(vec!["gpt-4o".to_string()]),
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
        payload.input_price_per_1m = Some(-0.01);
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
            selected_models: Some(vec![" ".to_string()]),
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
            input_price_per_1m: Some(5.0),
            output_price_per_1m: Some(15.0),
            capabilities: Some(vec!["vision".to_string()]),
            description: Some("test".to_string()),
            strategy: Some("round_robin".to_string()),
            enabled: Some(true),
            channels: Some(vec![NewMappingChannel {
                provider_id: "provider-1".to_string(),
                selected_models: Some(vec!["gpt-4o".to_string()]),
                enabled: Some(true),
            }]),
        }
    }

    fn assert_bad_request(result: Result<(), ServiceError>) {
        assert!(matches!(result, Err(ServiceError::BadRequest { .. })));
    }
}
