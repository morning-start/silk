use serde::Serialize;

use crate::error::ServiceError;

/// 网关 Key 的简单响应（仅包含明文 key）
#[derive(Debug, Serialize, Clone)]
pub struct GatewayKeyResponse {
    pub id: String,
    pub name: String,
    pub plain_key: String,
}

/// 获取内置 Key 的明文值（供 profile 注入使用）
pub fn builtin_key_value() -> String {
    crate::application::api_key_service::get_api_key()
}

/// 获取内置 Key 信息
pub fn get_builtin_key() -> Result<GatewayKeyResponse, ServiceError> {
    let key = crate::application::api_key_service::get_api_key();
    Ok(GatewayKeyResponse {
        id: "builtin".to_string(),
        name: "builtin".to_string(),
        plain_key: key,
    })
}

/// 刷新内置 Key，返回新的明文
pub fn reset_builtin_key() -> Result<GatewayKeyResponse, ServiceError> {
    let key = crate::application::api_key_service::reset_api_key()?;
    Ok(GatewayKeyResponse {
        id: "builtin".to_string(),
        name: "builtin".to_string(),
        plain_key: key,
    })
}