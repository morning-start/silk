use serde::{Deserialize, Serialize};

use crate::error::{require_db, require_found, ServiceError};
use crate::models::{GatewayKey, NewGatewayKey, UpdateGatewayKey};
use crate::persistence::GatewayKeyRepo;
use crate::{impl_crud_list, impl_crud_get, impl_crud_delete};

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GatewayKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
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
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
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
pub async fn create(payload: CreateGatewayKeyPayload) -> Result<CreateGatewayKeyResponse, ServiceError> {
    let pool = require_db()?;
    let key_value = if payload.key_value.is_empty() {
        format!("sk-gw-{}", uuid::Uuid::new_v4().to_string().replace("-", ""))
    } else {
        payload.key_value
    };
    let new = NewGatewayKey {
        name: payload.name,
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
pub async fn update(id: String, payload: UpdateGatewayKeyPayload) -> Result<GatewayKeyResponse, ServiceError> {
    let pool = require_db()?;
    let update = UpdateGatewayKey {
        name: payload.name,
        enabled: payload.enabled,
        expires_at: payload.expires_at,
        max_concurrent: payload.max_concurrent,
    };
    let key = require_found(GatewayKeyRepo::update(pool, &id, &update).await?, "网关 Key")?;
    Ok(GatewayKeyResponse::from(key))
}