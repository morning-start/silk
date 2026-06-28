use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
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
}

impl From<crate::protocol::ProtocolError> for GatewayError {
    fn from(err: crate::protocol::ProtocolError) -> Self {
        GatewayError::Transform(err.to_string())
    }
}

impl GatewayError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            GatewayError::BadRequest(_) | GatewayError::Transform(_) => StatusCode::BAD_REQUEST,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            GatewayError::Upstream(_) => StatusCode::BAD_GATEWAY,
            GatewayError::Timeout => StatusCode::GATEWAY_TIMEOUT,
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
            GatewayError::Database(_) => "database_error",
            GatewayError::Internal(_) => "internal_error",
            GatewayError::Timeout => "timeout",
            GatewayError::Serialization(_) => "serialization_error",
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response {
        let body = axum::Json(GatewayErrorPayload {
            message: self.to_string(),
        });
        (self.status_code(), body).into_response()
    }
}
