use crate::crypto::hash_api_key;
use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::persistence::GatewayKeyRepo;
use axum::http::HeaderMap;

/// 从 headers 中提取认证 token
///
/// 支持多种格式：
/// - Authorization: Bearer <token>  (OpenAI 风格)
/// - Authorization: Token <token>   (其他风格)
/// - x-api-key: <token>             (Anthropic 风格，小写)
/// - X-API-Key: <token>             (Anthropic 风格，原始大小写)
fn extract_auth_token(headers: &HeaderMap) -> Option<String> {
    // 1. 尝试 Authorization: Bearer 或 Authorization: Token
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            let auth_str = auth_str.trim();
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
            if let Some(token) = auth_str.strip_prefix("Token ") {
                return Some(token.trim().to_string());
            }
        }
    }

    // 2. 尝试 x-api-key (小写)
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            return Some(key_str.trim().to_string());
        }
    }

    // 3. 尝试 X-API-Key (原始大小写)
    if let Some(api_key) = headers.get("X-API-Key") {
        if let Ok(key_str) = api_key.to_str() {
            return Some(key_str.trim().to_string());
        }
    }

    None
}

/// 认证中间件：对所有 /v1/* 请求校验网关 Key
///
/// 支持的认证方式：
/// - Authorization: Bearer <sk-gw-xxx>  (OpenAI 风格)
/// - Authorization: Token <sk-gw-xxx>   (其他风格)
/// - x-api-key: <sk-gw-xxx>             (Anthropic 风格，小写)
/// - X-API-Key: <sk-gw-xxx>             (Anthropic 风格，原始大小写)
pub async fn run(
    mut ctx: RequestContext,
    runtime: &GatewayContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 仅校验 /v1/* 路径
    if !ctx.path.starts_with("/v1/") {
        return Ok(ctx);
    }

    // 提取 token：按优先级尝试多种方式
    let bearer_token = match extract_auth_token(&ctx.headers) {
        Some(t) => t,
        None => {
            return Err(StageError::new(
                error_ctx,
                GatewayError::Unauthorized("缺少 Key".to_string()),
            ));
        }
    };

    // 哈希 token 并在数据库中查找（使用 GatewayContext 中的 pool）
    let key_hash = hash_api_key(&bearer_token);

    match GatewayKeyRepo::find_by_hash(&runtime.pool, &key_hash).await {
        Ok(Some(key)) => {
            if !key.is_active() {
                return Err(StageError::new(
                    error_ctx,
                    GatewayError::Unauthorized("Key 错误".to_string()),
                ));
            }
            // 认证通过，注入 key 名称到上下文中（日志可用）
            ctx.auth_key_name = Some(key.name);
            Ok(ctx)
        }
        Ok(None) => Err(StageError::new(
            error_ctx,
            GatewayError::Unauthorized("Key 错误".to_string()),
        )),
        Err(e) => Err(StageError::new(error_ctx, GatewayError::Database(e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer sk-test-123".parse().unwrap(),
        );
        
        let token = extract_auth_token(&headers);
        assert_eq!(token, Some("sk-test-123".to_string()));
    }

    #[test]
    fn test_extract_token_format() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Token sk-test-456".parse().unwrap(),
        );
        
        let token = extract_auth_token(&headers);
        assert_eq!(token, Some("sk-test-456".to_string()));
    }

    #[test]
    fn test_extract_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "sk-test-789".parse().unwrap());
        
        let token = extract_auth_token(&headers);
        assert_eq!(token, Some("sk-test-789".to_string()));
    }

    #[test]
    fn test_extract_x_api_key_uppercase() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", "sk-test-012".parse().unwrap());
        
        let token = extract_auth_token(&headers);
        assert_eq!(token, Some("sk-test-012".to_string()));
    }

    #[test]
    fn test_no_auth_header() {
        let headers = HeaderMap::new();
        let token = extract_auth_token(&headers);
        assert_eq!(token, None);
    }
}
