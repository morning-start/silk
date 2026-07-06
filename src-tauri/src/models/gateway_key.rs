use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 网关 API Key
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct GatewayKey {
    pub id: String,
    pub name: String,
    /// Key 的 SHA-256 哈希（用于验证）
    pub key_hash: String,
    /// 加密存储的明文 Key（本地单机可回显）
    pub encrypted_key_value: String,
    pub enabled: i64,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 创建 GatewayKey 的输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGatewayKey {
    pub name: String,
    /// 明文 key（存储时哈希）
    pub key_value: String,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

/// 更新 GatewayKey 的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateGatewayKey {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub max_concurrent: Option<i64>,
}

impl GatewayKey {
    /// 检查 key 是否过期
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => chrono::Utc::now().naive_utc() > expiry,
            None => false,
        }
    }

    /// 检查 key 是否可用
    pub fn is_active(&self) -> bool {
        self.enabled != 0 && !self.is_expired()
    }
}
