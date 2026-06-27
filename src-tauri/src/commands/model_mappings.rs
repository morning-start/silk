use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{ModelMapping, NewModelMapping, UpdateModelMapping};
use crate::persistence::ModelMappingRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有模型映射
#[tauri::command]
pub async fn list_model_mappings(
    _state: State<'_, AppState>,
) -> Result<Vec<ModelMappingResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let mappings = ModelMappingRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询模型映射失败: {e}"))?;

    Ok(mappings.into_iter().map(ModelMappingResponse::from).collect())
}

/// 获取单个模型映射
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

    Ok(ModelMappingResponse::from(mapping))
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

    Ok(ModelMappingResponse::from(mapping))
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
        provider_group_id: payload.provider_group_id,
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
        capabilities: payload.capabilities,
        enabled: payload.enabled,
    };

    let mapping = ModelMappingRepo::create(pool, &new)
        .await
        .map_err(|e| format!("创建模型映射失败: {e}"))?;

    Ok(ModelMappingResponse::from(mapping))
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
        provider_group_id: payload.provider_group_id,
        max_input_tokens: payload.max_input_tokens,
        max_context_tokens: payload.max_context_tokens,
        max_output_tokens: payload.max_output_tokens,
        input_price_per_1m: payload.input_price_per_1m,
        output_price_per_1m: payload.output_price_per_1m,
        capabilities: payload.capabilities,
        enabled: payload.enabled,
    };

    let mapping = ModelMappingRepo::update(pool, &id, &update)
        .await
        .map_err(|e| format!("更新模型映射失败: {e}"))?
        .ok_or("模型映射不存在")?;

    Ok(ModelMappingResponse::from(mapping))
}

/// 删除模型映射
#[tauri::command]
pub async fn delete_model_mapping(
    _state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = ModelMappingRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除模型映射失败: {e}"))?;

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ModelMappingResponse {
    pub id: String,
    pub model_name: String,
    pub provider_group_id: Option<String>,
    pub max_input_tokens: Option<i64>,
    pub max_context_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub input_price_per_1m: Option<f64>,
    pub output_price_per_1m: Option<f64>,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ModelMapping> for ModelMappingResponse {
    fn from(m: ModelMapping) -> Self {
        let capabilities = m.capabilities_vec();
        Self {
            id: m.id,
            model_name: m.model_name,
            provider_group_id: m.provider_group_id,
            max_input_tokens: m.max_input_tokens,
            max_context_tokens: m.max_context_tokens,
            max_output_tokens: m.max_output_tokens,
            input_price_per_1m: m.input_price_per_1m,
            output_price_per_1m: m.output_price_per_1m,
            capabilities,
            enabled: m.enabled != 0,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateModelMappingPayload {
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

#[derive(Debug, Deserialize)]
pub struct UpdateModelMappingPayload {
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
