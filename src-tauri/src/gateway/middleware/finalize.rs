use axum::response::{IntoResponse, Response};

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;

use super::build_response;

/// 成功响应：如果 ctx 中已有构建好的响应（如 SSE 流式），直接返回；
/// 否则从上下文中构建非流式响应。
pub fn success(mut ctx: RequestContext) -> Response {
    // 如果响应已构建（如 SSE 流式），直接返回
    if let Some(response) = ctx.response.take() {
        return response;
    }

    // 非流式：从上下文构建响应
    let status = ctx
        .final_status
        .or(ctx.upstream_status)
        .unwrap_or(axum::http::StatusCode::OK);
    let headers = ctx.upstream_headers.clone().unwrap_or_default();
    let body = ctx.upstream_body.clone().unwrap_or_default();

    build_response(status, headers, body)
}

pub fn failure(error: GatewayError) -> Response {
    error.into_response()
}
