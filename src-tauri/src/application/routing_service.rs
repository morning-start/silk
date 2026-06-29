use serde::{Deserialize, Serialize};

use crate::error::{require_db, require_found, ServiceError};
use crate::models::{NewRoutingRule, RoutingRule, UpdateRoutingRule};
use crate::persistence::RoutingRuleRepo;
use crate::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct RoutingRuleResponse {
    pub id: String,
    pub name: String,
    pub match_host: Option<String>,
    pub match_path: String,
    pub match_method: String,
    pub match_content_type: Option<String>,
    pub target_provider_id: String,
    pub target_group_id: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub protocol_conversion: bool,
    pub model_name_override: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoutingRulePayload {
    pub name: String,
    pub match_host: Option<String>,
    pub match_path: String,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
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
    pub match_host: Option<String>,
    pub match_path: Option<String>,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub target_provider_id: Option<String>,
    pub target_group_id: Option<String>,
    pub protocol_conversion: Option<bool>,
    pub model_name_override: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

pub async fn list() -> Result<Vec<RoutingRuleResponse>, ServiceError> {
    let pool = require_db()?;
    let rules = RoutingRuleRepo::find_all(pool).await?;

    Ok(rules.into_iter().map(RoutingRuleResponse::from).collect())
}

pub async fn get(id: String) -> Result<RoutingRuleResponse, ServiceError> {
    let pool = require_db()?;
    let rule = require_found(RoutingRuleRepo::find_by_id(pool, &id).await?, "路由规则")?;

    Ok(RoutingRuleResponse::from(rule))
}

pub async fn create(
    state: &AppState,
    payload: CreateRoutingRulePayload,
) -> Result<RoutingRuleResponse, ServiceError> {
    let pool = require_db()?;

    let new = NewRoutingRule {
        name: payload.name,
        match_host: payload.match_host,
        match_path: payload.match_path,
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        inbound_protocol: payload.inbound_protocol,
        outbound_protocol: payload.outbound_protocol,
        target_provider_id: payload.target_provider_id,
        target_group_id: payload.target_group_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = RoutingRuleRepo::create(pool, &new).await?;

    state.reload_routes(pool).await;

    Ok(RoutingRuleResponse::from(rule))
}

pub async fn update(
    state: &AppState,
    id: String,
    payload: UpdateRoutingRulePayload,
) -> Result<RoutingRuleResponse, ServiceError> {
    let pool = require_db()?;

    let update = UpdateRoutingRule {
        name: payload.name,
        match_host: payload.match_host,
        match_path: payload.match_path,
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        inbound_protocol: payload.inbound_protocol,
        outbound_protocol: payload.outbound_protocol,
        target_provider_id: payload.target_provider_id,
        target_group_id: payload.target_group_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = require_found(RoutingRuleRepo::update(pool, &id, &update).await?, "路由规则")?;

    state.reload_routes(pool).await;

    Ok(RoutingRuleResponse::from(rule))
}

pub async fn delete(state: &AppState, id: String) -> Result<bool, ServiceError> {
    let pool = require_db()?;
    let deleted = RoutingRuleRepo::delete(pool, &id).await?;

    if deleted {
        state.reload_routes(pool).await;
    }

    Ok(deleted)
}

impl From<RoutingRule> for RoutingRuleResponse {
    fn from(r: RoutingRule) -> Self {
        Self {
            id: r.id,
            name: r.name,
            match_host: r.match_host,
            match_path: r.match_path,
            match_method: r.match_method,
            match_content_type: r.match_content_type,
            target_provider_id: r.target_provider_id,
            target_group_id: r.target_group_id,
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