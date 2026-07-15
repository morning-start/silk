use serde::{Deserialize, Serialize};

use crate::error::{bad_request, require_db, require_found, ServiceError};
use crate::models::{GatewayKey, NewGatewayKey, UpdateGatewayKey};
use crate::persistence::GatewayKeyRepo;
use crate::{impl_crud_delete, impl_crud_get, impl_crud_list};

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GatewayKeyResponse {
    pub id: String,
    pub name: String,
    pub plain_key: String,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub max_concurrent: i64,
    pub is_expired: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GatewayKey> for GatewayKeyResponse {
    fn from(k: GatewayKey) -> Self {
        let is_expired = k.is_expired();
        let plain_key = match crate::crypto::decrypt(&k.encrypted_key_value) {
            Ok(key) => key,
            Err(e) => {
                tracing::warn!(%e, key_id = %k.id, "解密网关 Key 失败");
                String::new()
            }
        };
        Self {
            id: k.id,
            name: k.name,
            plain_key,
            enabled: k.enabled != 0,
            expires_at: k.expires_at.map(|d| d.to_string()),
            max_concurrent: k.max_concurrent,
            is_expired,
            created_at: k.created_at.to_string(),
            updated_at: k.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateGatewayKeyResponse {
    pub key: GatewayKeyResponse,
    pub plain_key: String,
}

// ---------------------------------------------------------------------------
// Payload Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateGatewayKeyPayload {
    pub name: String,
    pub key_value: String,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGatewayKeyPayload {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

// ---------------------------------------------------------------------------
// CRUD（list / get / delete 由宏生成）
// ---------------------------------------------------------------------------

impl_crud_list!(GatewayKeyResponse, GatewayKeyRepo, "网关 Key");
impl_crud_get!(GatewayKeyResponse, GatewayKeyRepo, "网关 Key");
impl_crud_delete!(GatewayKeyRepo);

/// 创建网关 Key
pub async fn create(
    payload: CreateGatewayKeyPayload,
) -> Result<CreateGatewayKeyResponse, ServiceError> {
    let pool = require_db()?;
    validate_name(&payload.name)?;
    validate_max_concurrent(payload.max_concurrent)?;

    let key_value = if payload.key_value.trim().is_empty() {
        format!(
            "sk-gw-{}",
            uuid::Uuid::new_v4().to_string().replace("-", "")
        )
    } else {
        payload.key_value.trim().to_string()
    };
    let new = NewGatewayKey {
        name: payload.name.trim().to_string(),
        key_value,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };
    let (key, plain_key) = GatewayKeyRepo::create(pool, &new).await?;
    Ok(CreateGatewayKeyResponse {
        key: GatewayKeyResponse::from(key),
        plain_key,
    })
}

/// 更新网关 Key
pub async fn update(
    id: String,
    payload: UpdateGatewayKeyPayload,
) -> Result<GatewayKeyResponse, ServiceError> {
    let pool = require_db()?;
    if let Some(name) = &payload.name {
        validate_name(name)?;
    }
    validate_max_concurrent(payload.max_concurrent)?;

    let update = UpdateGatewayKey {
        name: payload.name.map(|name| name.trim().to_string()),
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };
    let key = require_found(
        GatewayKeyRepo::update(pool, &id, &update).await?,
        "网关 Key",
    )?;
    Ok(GatewayKeyResponse::from(key))
}

fn validate_name(name: &str) -> Result<(), ServiceError> {
    if name.trim().is_empty() {
        return bad_request("Key 名称不能为空");
    }
    Ok(())
}

fn validate_max_concurrent(value: Option<i64>) -> Result<(), ServiceError> {
    if let Some(value) = value {
        if !(1..=1000).contains(&value) {
            return bad_request("Key 并发数必须在 1-1000 之间");
        }
    }
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_key_name() {
        assert_bad_request(validate_name(" "));
    }

    #[test]
    fn validate_rejects_invalid_concurrency() {
        assert_bad_request(validate_max_concurrent(Some(0)));
        assert_bad_request(validate_max_concurrent(Some(1001)));
    }

    #[test]
    fn validate_accepts_gateway_key_bounds() {
        validate_name("默认 Key").expect("valid name");
        validate_max_concurrent(Some(1)).expect("min concurrency");
        validate_max_concurrent(Some(1000)).expect("max concurrency");
        validate_max_concurrent(None).expect("default concurrency");
    }

    fn assert_bad_request(result: Result<(), ServiceError>) {
        assert!(matches!(result, Err(ServiceError::BadRequest { .. })));
    }
}
