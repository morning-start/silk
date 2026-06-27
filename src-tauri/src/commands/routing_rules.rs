use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{NewRoutingRule, RoutingRule, UpdateRoutingRule};
use crate::persistence::RoutingRuleRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有路由规则
#[tauri::command]
pub async fn list_routing_rules(
    _state: State<'_, AppState>,
) -> Result<Vec<RoutingRuleResponse>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let rules = RoutingRuleRepo::find_all(pool)
        .await
        .map_err(|e| format!("查询路由规则失败: {e}"))?;

    Ok(rules.into_iter().map(RoutingRuleResponse::from).collect())
}

/// 获取单个路由规则
#[tauri::command]
pub async fn get_routing_rule(
    _state: State<'_, AppState>,
    id: String,
) -> Result<RoutingRuleResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let rule = RoutingRuleRepo::find_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询路由规则失败: {e}"))?
        .ok_or("路由规则不存在")?;

    Ok(RoutingRuleResponse::from(rule))
}

/// 创建路由规则
#[tauri::command]
pub async fn create_routing_rule(
    state: State<'_, AppState>,
    payload: CreateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let new = NewRoutingRule {
        name: payload.name,
        match_path: payload.match_path,
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        target_provider_id: payload.target_provider_id,
        target_group_id: payload.target_group_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = RoutingRuleRepo::create(pool, &new)
        .await
        .map_err(|e| format!("创建路由规则失败: {e}"))?;

    // 触发路由表重载
    state.gateway.read().await.route_manager.reload(pool).await.ok();

    Ok(RoutingRuleResponse::from(rule))
}

/// 更新路由规则
#[tauri::command]
pub async fn update_routing_rule(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateRoutingRule {
        name: payload.name,
        match_path: payload.match_path,
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        target_provider_id: payload.target_provider_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = RoutingRuleRepo::update(pool, &id, &update)
        .await
        .map_err(|e| format!("更新路由规则失败: {e}"))?
        .ok_or("路由规则不存在")?;

    // 触发路由表重载
    state.gateway.read().await.route_manager.reload(pool).await.ok();

    Ok(RoutingRuleResponse::from(rule))
}

/// 删除路由规则
#[tauri::command]
pub async fn delete_routing_rule(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = RoutingRuleRepo::delete(pool, &id)
        .await
        .map_err(|e| format!("删除路由规则失败: {e}"))?;

    if deleted {
        state.gateway.read().await.route_manager.reload(pool).await.ok();
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct RoutingRuleResponse {
    pub id: String,
    pub name: String,
    pub match_path: String,
    pub match_method: String,
    pub match_content_type: Option<String>,
    pub target_provider_id: String,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub protocol_conversion: bool,
    pub model_name_override: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<RoutingRule> for RoutingRuleResponse {
    fn from(r: RoutingRule) -> Self {
        Self {
            id: r.id,
            name: r.name,
            match_path: r.match_path,
            match_method: r.match_method,
            match_content_type: r.match_content_type,
            target_provider_id: r.target_provider_id,
            inbound_protocol: r.inbound_protocol,
            outbound_protocol: r.outbound_protocol,
            protocol_conversion: r.protocol_conversion != 0,
            model_name_override: r.model_name_override,
            priority: r.priority,
            enabled: r.enabled != 0,
            created_at: r.created_at.to_string(),
            updated_at: r.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateRoutingRulePayload {
    pub name: String,
    pub match_path: String,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub target_provider_id: String,
    pub target_group_id: Option<String>,
    pub protocol_conversion: Option<bool>,
    pub model_name_override: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoutingRulePayload {
    pub name: Option<String>,
    pub match_path: Option<String>,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub target_provider_id: Option<String>,
    pub target_group_id: Option<String>,
    pub protocol_conversion: Option<bool>,
    pub model_name_override: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}
