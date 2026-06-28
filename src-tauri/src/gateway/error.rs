use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Serialize)]
struct GatewayErrorPayload {
    message: String,
}

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("请求错误: {0}")]
    BadRequest(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("协议转换错误: {0}")]
    Transform(String),

    #[error("上游请求失败: {0}")]
    Upstream(#[from] reqwest::Error),

    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("内部错误: {0}")]
    Internal(String),

    #[error("请求超时")]
    Timeout,

    #[error("序列化错误: {0}")]
    Serialization(String),

    #[error("未授权: {0}")]
    Unauthorized(String),

    #[error("请求过多")]
    TooManyRequests,

    /// 上游返回 HTTP 错误（4xx/5xx），携带原始错误体
    #[error("上游返回错误: HTTP {status}")]
    UpstreamError {
        status: u16,
        body: serde_json::Value,
    },
}

impl From<crate::protocol::ProtocolError> for GatewayError {
    fn from(err: crate::protocol::ProtocolError) -> Self {
        match &err {
            crate::protocol::ProtocolError::UpstreamError { status, message } => {
                GatewayError::UpstreamError {
                    status: *status,
                    body: json!({"error": {"message": message}}),
                }
            }
            _ => GatewayError::Transform(err.to_string()),
        }
    }
}

impl GatewayError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            GatewayError::BadRequest(_) | GatewayError::Transform(_) => StatusCode::BAD_REQUEST,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            GatewayError::Upstream(_) => StatusCode::BAD_GATEWAY,
            GatewayError::UpstreamError { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            GatewayError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            GatewayError::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            GatewayError::Database(_)
            | GatewayError::Internal(_)
            | GatewayError::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            GatewayError::BadRequest(_) => "bad_request",
            GatewayError::NotFound(_) => "not_found",
            GatewayError::Unauthorized(_) => "unauthorized",
            GatewayError::Transform(_) => "transform_error",
            GatewayError::Upstream(_) => "upstream_error",
            GatewayError::UpstreamError { .. } => "upstream_error",
            GatewayError::Database(_) => "database_error",
            GatewayError::Internal(_) => "internal_error",
            GatewayError::Timeout => "timeout",
            GatewayError::TooManyRequests => "too_many_requests",
            GatewayError::Serialization(_) => "serialization_error",
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response {
        match self {
            // 上游错误：透传原始状态码和错误体
            GatewayError::UpstreamError { status, body } => {
                let code =
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
                (code, axum::Json(body)).into_response()
            }
            // 其他错误：统一包装为 {"message": "..."}
            _ => {
                let body = axum::Json(GatewayErrorPayload {
                    message: self.to_string(),
                });
                (self.status_code(), body).into_response()
            }
        }
    }
}
