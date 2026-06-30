use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::http::request::Parts;
use axum::http::{Method, Uri};
use uuid::Uuid;

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;

use super::{header_value_as_str, request_path};

const REQUEST_BODY_LIMIT: usize = 2 * 1024 * 1024;

pub fn initialize(parts: Parts) -> RequestContext {
    let request_id = Uuid::new_v4().to_string();
    let started_at = Instant::now();
    let method: Method = parts.method;
    let uri: Uri = parts.uri;
    let headers = parts.headers;

    RequestContext::new(request_id, started_at, method, uri, headers)
}

pub async fn read_body(ctx: RequestContext, body: Body) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();
    let bytes = to_bytes(body, REQUEST_BODY_LIMIT).await.map_err(|err| {
        StageError::new(
            error_ctx,
            GatewayError::BadRequest(format!("读取请求体失败: {err}")),
        )
    })?;

    let mut ctx = ctx;
    ctx.client_body = bytes.clone();
    ctx.request_body = bytes;
    if !ctx.request_body.is_empty() {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&ctx.request_body) {
            ctx.parsed_body = Some(json);
        }
    }
    let host = header_value_as_str(ctx.headers.get(axum::http::header::HOST))
        .map(|value| value.to_string());
    let content_type = header_value_as_str(ctx.headers.get(axum::http::header::CONTENT_TYPE))
        .map(|value| value.to_string());
    ctx.path = request_path(&ctx.uri);
    ctx.host = host;
    ctx.content_type = content_type;
    Ok(ctx)
}
