use crate::crypto::hash_api_key;
use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::persistence::GatewayKeyRepo;

/// 认证中间件：对所有 /v1/* 请求校验 Authorization: Bearer <sk-gw-xxx>
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();

    // 仅校验 /v1/* 路径
    if !ctx.path.starts_with("/v1/") {
        return Ok(ctx);
    }

    // 从 Authorization header 提取 Bearer token
    let auth_header = ctx
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    let bearer_token = match auth_header {
        Some(ref val) if val.starts_with("Bearer ") => val[7..].trim().to_string(),
        _ => {
            return Err(StageError::new(
                error_ctx,
                GatewayError::Unauthorized("缺少 Key".to_string()),
            ));
        }
    };

    // 哈希 token 并在数据库中查找
    let pool = crate::get_db_pool().ok_or_else(|| {
        StageError::new(
            error_ctx.clone(),
            GatewayError::Internal("数据库未初始化".to_string()),
        )
    })?;
    let key_hash = hash_api_key(&bearer_token);

    match GatewayKeyRepo::find_by_hash(pool, &key_hash).await {
        Ok(Some(key)) => {
            if !key.is_active() {
                return Err(StageError::new(
                    error_ctx,
                    GatewayError::Unauthorized("Key 错误".to_string()),
                ));
            }
            // 认证通过，注入 key 名称到上下文中（日志可用）
            ctx.auth_key_name = Some(key.name);
            ctx.auth_key_value = Some(bearer_token);
            Ok(ctx)
        }
        Ok(None) => Err(StageError::new(
            error_ctx,
            GatewayError::Unauthorized("Key 错误".to_string()),
        )),
        Err(e) => Err(StageError::new(error_ctx, GatewayError::Database(e))),
    }
}
