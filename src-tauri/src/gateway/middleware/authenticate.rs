use crate::crypto::hash_api_key;
use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::persistence::GatewayKeyRepo;

/// 认证中间件：对所有 /v1/* 请求校验网关 Key
///
/// 支持的认证方式：
/// - Authorization: Bearer <sk-gw-xxx>  (OpenAI 风格)
/// - x-api-key: <sk-gw-xxx>              (Anthropic 风格)
pub async fn run(
    mut ctx: RequestContext,
    runtime: &GatewayContext,
) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 仅校验 /v1/* 路径
    if !ctx.path.starts_with("/v1/") {
        return Ok(ctx);
    }

    // 提取 token：优先 Authorization: Bearer，其次 x-api-key
    let token = ctx
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .and_then(|val| {
            if val.starts_with("Bearer ") {
                Some(val[7..].trim().to_string())
            } else {
                None
            }
        })
        .or_else(|| {
            ctx.headers
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });

    let bearer_token = match token {
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
