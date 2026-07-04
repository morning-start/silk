use serde::{Deserialize, Serialize};

use crate::error::{bad_request, require_db, require_found, validate_non_empty, ServiceError};
use crate::models::{NewRoutingRule, RoutingRule, UpdateRoutingRule};
use crate::persistence::RoutingRuleRepo;
use crate::AppState;
use crate::{impl_crud_get, impl_crud_list};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// CRUD（list / get 由宏生成，create / update / delete 手写）
// ---------------------------------------------------------------------------

impl_crud_list!(RoutingRuleResponse, RoutingRuleRepo, "路由规则");
impl_crud_get!(RoutingRuleResponse, RoutingRuleRepo, "路由规则");

/// 创建路由规则
pub async fn create(
    state: &AppState,
    payload: CreateRoutingRulePayload,
) -> Result<RoutingRuleResponse, ServiceError> {
    let pool = require_db()?;
    validate_create_payload(&payload)?;

    let new = NewRoutingRule {
        name: payload.name.trim().to_string(),
        match_host: payload.match_host,
        match_path: payload.match_path.trim().to_string(),
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        inbound_protocol: payload.inbound_protocol,
        outbound_protocol: payload.outbound_protocol,
        target_provider_id: payload.target_provider_id.trim().to_string(),
        target_group_id: payload.target_group_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = RoutingRuleRepo::create(pool, &new).await?;

    state.reload_routes(pool).await;
    state.refresh_lookup().await;

    Ok(RoutingRuleResponse::from(rule))
}

pub async fn update(
    state: &AppState,
    id: String,
    payload: UpdateRoutingRulePayload,
) -> Result<RoutingRuleResponse, ServiceError> {
    let pool = require_db()?;
    validate_update_payload(&payload)?;

    let update = UpdateRoutingRule {
        name: payload.name.map(|name| name.trim().to_string()),
        match_host: payload.match_host,
        match_path: payload.match_path.map(|path| path.trim().to_string()),
        match_method: payload.match_method,
        match_content_type: payload.match_content_type,
        inbound_protocol: payload.inbound_protocol,
        outbound_protocol: payload.outbound_protocol,
        target_provider_id: payload
            .target_provider_id
            .map(|provider_id| provider_id.trim().to_string()),
        target_group_id: payload.target_group_id,
        protocol_conversion: payload.protocol_conversion,
        model_name_override: payload.model_name_override,
        priority: payload.priority,
        enabled: payload.enabled,
        ..Default::default()
    };

    let rule = require_found(
        RoutingRuleRepo::update(pool, &id, &update).await?,
        "路由规则",
    )?;

    state.reload_routes(pool).await;
    state.refresh_lookup().await;

    Ok(RoutingRuleResponse::from(rule))
}

pub async fn delete(state: &AppState, id: String) -> Result<bool, ServiceError> {
    let pool = require_db()?;
    let deleted = RoutingRuleRepo::delete(pool, &id).await?;

    if deleted {
        state.reload_routes(pool).await;
        state.refresh_lookup().await;
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

fn validate_create_payload(payload: &CreateRoutingRulePayload) -> Result<(), ServiceError> {
    validate_non_empty("路由名称", &payload.name)?;
    validate_match_path(&payload.match_path)?;
    validate_method(payload.match_method.as_deref())?;
    validate_non_empty("目标渠道", &payload.target_provider_id)?;
    validate_priority(payload.priority)?;
    Ok(())
}

fn validate_update_payload(payload: &UpdateRoutingRulePayload) -> Result<(), ServiceError> {
    if let Some(name) = &payload.name {
        validate_non_empty("路由名称", name)?;
    }
    if let Some(path) = &payload.match_path {
        validate_match_path(path)?;
    }
    validate_method(payload.match_method.as_deref())?;
    if let Some(provider_id) = &payload.target_provider_id {
        validate_non_empty("目标渠道", provider_id)?;
    }
    validate_priority(payload.priority)?;
    Ok(())
}

fn validate_match_path(path: &str) -> Result<(), ServiceError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return bad_request("匹配路径不能为空");
    }
    if !trimmed.starts_with('/') {
        return bad_request("匹配路径必须以 / 开头");
    }
    Ok(())
}

fn validate_method(method: Option<&str>) -> Result<(), ServiceError> {
    if let Some(method) = method {
        if !matches!(method, "GET" | "POST" | "PUT" | "DELETE" | "*") {
            return bad_request("匹配方法无效");
        }
    }
    Ok(())
}

fn validate_priority(priority: Option<i64>) -> Result<(), ServiceError> {
    if let Some(priority) = priority {
        if priority < 0 {
            return bad_request("路由优先级不能为负数");
        }
    }
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn validate_route_rejects_empty_required_fields() {
        let mut payload = valid_create_payload();
        payload.name = " ".to_string();
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.match_path = " ".to_string();
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.target_provider_id = " ".to_string();
        assert_bad_request(validate_create_payload(&payload));
    }

    #[test]
    fn validate_route_rejects_invalid_method_path_and_priority() {
        let mut payload = valid_create_payload();
        payload.match_path = "v1/chat".to_string();
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.match_method = Some("PATCH".to_string());
        assert_bad_request(validate_create_payload(&payload));

        let mut payload = valid_create_payload();
        payload.priority = Some(-1);
        assert_bad_request(validate_create_payload(&payload));
    }

    #[test]
    fn validate_route_accepts_valid_create_payload() {
        validate_create_payload(&valid_create_payload()).expect("valid route");
    }

    fn valid_create_payload() -> CreateRoutingRulePayload {
        CreateRoutingRulePayload {
            name: "chat".to_string(),
            match_host: None,
            match_path: "/v1/chat/completions".to_string(),
            match_method: Some("POST".to_string()),
            match_content_type: None,
            inbound_protocol: None,
            outbound_protocol: None,
            target_provider_id: "provider-1".to_string(),
            target_group_id: None,
            protocol_conversion: Some(true),
            model_name_override: None,
            priority: Some(100),
            enabled: Some(true),
        }
    }

    fn assert_bad_request(result: Result<(), ServiceError>) {
        assert!(matches!(result, Err(ServiceError::BadRequest { .. })));
    }
}
