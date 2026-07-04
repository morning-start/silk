use serde::{Deserialize, Serialize};

use crate::error::{require_db, require_found, ServiceError};
use crate::models::{
    MappingChannelInfo, ModelMapping, NewMappingChannel, NewModelMapping, UpdateModelMapping,
};
use crate::persistence::ModelMappingRepo;
use crate::impl_crud_delete;

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
pub async fn find_by_name(model_name: String) -> Result<Option<ModelMappingResponse>, ServiceError> {
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
pub async fn create(payload: CreateModelMappingPayload) -> Result<ModelMappingResponse, ServiceError> {
    let pool = require_db()?;
    let new = NewModelMapping {
        model_name: payload.model_name,
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
pub async fn update(id: String, payload: UpdateModelMappingPayload) -> Result<ModelMappingResponse, ServiceError> {
    let pool = require_db()?;
    let update = UpdateModelMapping {
        model_name: payload.model_name,
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
    let mapping = require_found(ModelMappingRepo::update(pool, &id, &update).await?, "模型映射")?;
    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}
