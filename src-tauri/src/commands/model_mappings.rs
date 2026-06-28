use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{
    MappingChannelInfo, ModelMapping, NewMappingChannel, NewModelMapping, UpdateModelMapping,
};
use crate::persistence::ModelMappingRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有模型映射（含关联渠道）
#[tauri::command]
pub async fn list_model_mappings(
    _state: State<'_, AppState>,
) -> Result<Vec<ModelMappingResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mappings = ModelMappingRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询模型映射失败: {e}"))?;

    let mut result = Vec::with_capacity(mappings.len());
    for m in mappings {
        let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &m.id)
            .await
            .unwrap_or_default();
        result.push(ModelMappingResponse::from_model(m, channels));
    }
    Ok(result)
}

/// 获取单个模型映射（含关联渠道）
#[tauri::command]
pub async fn get_model_mapping(
    _state: State<'_, AppState>,
    id: String,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mapping = ModelMappingRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;

    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 根据模型名称查询
#[tauri::command]
pub async fn find_model_mapping_by_name(
    _state: State<'_, AppState>,
    model_name: String,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mapping = ModelMappingRepo::find_by_model_name(pool, &model_name)
        .await
        .map_err(|e| format!("查询模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;

    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 创建模型映射
#[tauri::command]
pub async fn create_model_mapping(
    _state: State<'_, AppState>,
    payload: CreateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

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

    let mapping = ModelMappingRepo::create(pool, &new)
        .await
        .map_err(|e| format!("创建模型映射失败: {e}"))?;

    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 更新模型映射
#[tauri::command]
pub async fn update_model_mapping(
    _state: State<'_, AppState>,
    id: String,
    payload: UpdateModelMappingPayload,
) -> Result<ModelMappingResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

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

    let mapping = ModelMappingRepo::update(pool, &id, &update)
        .await
        .map_err(|e| format!("更新模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;

    let channels = ModelMappingRepo::find_channels_by_mapping_id(pool, &mapping.id)
        .await
        .unwrap_or_default();
    Ok(ModelMappingResponse::from_model(mapping, channels))
}

/// 删除模型映射
#[tauri::command]
pub async fn delete_model_mapping(_state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = ModelMappingRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除模型映射失败: {e}"))?;

    Ok(deleted)
}

/// 获取分组内的渠道信息（废弃，保留用于旧路由规则页面）
#[tauri::command]
pub async fn get_group_providers(
    _state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<crate::models::GroupProviderInfo>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    ModelMappingRepo::find_group_providers(pool, &group_id)
        .await
        .map_err(|e| format!("查询分组渠道失败: {e}"))
}

// ---------------------------------------------------------------------------
// Types
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
    fn from_model(m: ModelMapping, channels: Vec<MappingChannelInfo>) -> Self {
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
