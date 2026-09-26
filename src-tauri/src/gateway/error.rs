use axum::http::StatusCode;
use axum::response::IntoResponse;
use bytes::Bytes;
use thiserror::Error;

/// 上游返回的错误响应（原样保留）
///
/// `body` 是**原始字节**而非解析后的 JSON：上游的错误体不一定是 JSON
/// （nginx 的 HTML 502、纯文本 401 都常见），解析失败或截断都会破坏「诚实返回」。
/// `content_type` 原样转发，保证客户端看到的 Content-Type 与上游一致。
#[derive(Debug, Clone)]
pub struct UpstreamFailure {
    pub status: u16,
    pub body: Bytes,
    pub content_type: Option<String>,
}

impl UpstreamFailure {
    pub fn new(status: u16, body: Bytes, content_type: Option<String>) -> Self {
        Self {
            status,
            body,
            content_type,
        }
    }

    /// 尽力从错误体中提取可读消息（**仅用于日志**，不影响透传内容）
    pub fn message(&self) -> String {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&self.body) {
            // OpenAI/Anthropic 系：{"error":{"message":...}}
            if let Some(msg) = json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                return msg.to_string();
            }
            // Gemini 系：{"error":{"status":...,"message":...}} 已覆盖
            // 其他扁平形状：{"message":...} / {"detail":...} / {"msg":...}
            for key in ["message", "detail", "msg", "error_description"] {
                if let Some(msg) = json.get(key).and_then(|m| m.as_str()) {
                    return msg.to_string();
                }
            }
        }
        String::from_utf8_lossy(&self.body).chars().take(500).collect()
    }
}

/// 错误来源：区分「上游返回的错误」与「silk 自身产生的错误」
///
/// 这是错误诚实性的基石：
/// - `Upstream`：错误体来自上游，**逐字节原样透传**，不加字段、不改写、不加工；
/// - `Silk`：错误由 silk 自己产生（网关鉴权、路由、协议转换、超时、连不上上游等），
///   客户端可见消息必须带 `【silk】` 标记，避免把 silk 的问题误判成上游的问题。
///
/// 历史教训：403「模型不在当前套餐内」曾被统一文案成「认证失败/API 密钥错误」，
/// 把用户引向改 Key 的错误方向 —— 实际上密钥有效，改 Key 无用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorOrigin {
    /// 上游（Provider）返回的错误 —— 原样透传
    Upstream,
    /// silk 自身产生的错误 —— 加 【silk】 标记
    Silk,
}

impl ErrorOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorOrigin::Upstream => "upstream",
            ErrorOrigin::Silk => "silk",
        }
    }
}

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("请求错误: {0}")]
    BadRequest(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("协议转换错误: {0}")]
    Transform(String),

    /// silk → 上游这一跳未能完成（DNS / 连接 / TLS / 读超时等）。
    ///
    /// 上游**没有**返回过任何错误响应，所以这是 silk 侧的报告，而非上游的原话。
    #[error("无法连接上游: {0}")]
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

    /// 上游返回 HTTP 错误（4xx/5xx），原样携带上游响应
    #[error("上游返回错误: HTTP {}", .0.status)]
    UpstreamError(UpstreamFailure),
}

impl GatewayError {
    /// silk 自身错误的统一标记（客户端可见）
    pub const SILK_MARKER: &'static str = "【silk】";

    /// 该错误的来源
    ///
    /// 只有 `UpstreamError` 是上游的原话，其余（含 `Upstream` 传输失败、`Timeout`
    /// 流超时、`TooManyRequests` 本地限流、`Unauthorized` 网关鉴权）都由 silk 产生。
    pub fn origin(&self) -> ErrorOrigin {
        match self {
            GatewayError::UpstreamError(_) => ErrorOrigin::Upstream,
            _ => ErrorOrigin::Silk,
        }
    }

    /// 是否为上游原样透传的错误
    pub fn is_upstream_origin(&self) -> bool {
        matches!(self.origin(), ErrorOrigin::Upstream)
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            GatewayError::BadRequest(_) | GatewayError::Transform(_) => StatusCode::BAD_REQUEST,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            GatewayError::Upstream(_) => StatusCode::BAD_GATEWAY,
            GatewayError::UpstreamError(failure) => {
                StatusCode::from_u16(failure.status).unwrap_or(StatusCode::BAD_GATEWAY)
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
            // 上游根本没答话（连不通/超时），与「上游返回了错误」是两回事
            GatewayError::Upstream(_) => "upstream_unreachable",
            GatewayError::UpstreamError(_) => "upstream_error",
            GatewayError::Database(_) => "database_error",
            GatewayError::Internal(_) => "internal_error",
            GatewayError::Timeout => "timeout",
            GatewayError::TooManyRequests => "too_many_requests",
            GatewayError::Serialization(_) => "serialization_error",
        }
    }

    /// silk 自身错误的客户端响应体；上游错误返回 `None`（走原样透传）
    ///
    /// 给出两种形状，兼顾不同客户端：
    /// - `message`：顶层字段，简单客户端/CLI 直接读它；
    /// - `error`：标准对象（`message`/`type`/`origin`），OpenAI 系 SDK、
    ///   Claude Code、Codex 等按标准形状取错时也能拿到标记。
    ///
    /// 客户端据此可判断：**没有 `【silk】` 就是上游的原话，有则是 silk 的问题**。
    pub fn silk_body(&self) -> Option<serde_json::Value> {
        if self.is_upstream_origin() {
            return None;
        }
        let message = self.marked_message();
        Some(serde_json::json!({
            "message": message,
            "error": {
                "message": message,
                "type": self.error_code(),
                "origin": ErrorOrigin::Silk.as_str(),
            }
        }))
    }

    /// 带来源标记的日志/展示用消息
    ///
    /// 上游错误保持原样（不含标记），silk 错误加 `【silk】`。
    pub fn marked_message(&self) -> String {
        match self.origin() {
            ErrorOrigin::Upstream => self.to_string(),
            ErrorOrigin::Silk => format!("{}{}", Self::SILK_MARKER, self),
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response {
        match self {
            // 上游错误：状态码 + Content-Type + 原始字节，全部原样透传
            GatewayError::UpstreamError(failure) => {
                let status = StatusCode::from_u16(failure.status)
                    .unwrap_or(StatusCode::BAD_GATEWAY);
                let mut builder = axum::response::Response::builder().status(status);
                if let Some(ref content_type) = failure.content_type {
                    builder = builder.header(axum::http::header::CONTENT_TYPE, content_type);
                }
                builder
                    .body(axum::body::Body::from(failure.body))
                    .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
            }
            // silk 自身错误：统一加 【silk】 标记
            other => {
                let status = other.status_code();
                let body = other.silk_body().expect("非上游错误必有 silk body");
                (status, axum::Json(body)).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream_failure() -> UpstreamFailure {
        UpstreamFailure::new(
            403,
            Bytes::from_static(
                br#"{"error":{"message":"model is not available in the current token plan","type":"permission_denied_error","code":"7"}}"#,
            ),
            Some("application/json".to_string()),
        )
    }

    #[test]
    fn upstream_error_body_is_passed_through_verbatim() {
        let failure = upstream_failure();
        let raw = failure.body.clone();
        let err = GatewayError::UpstreamError(failure);

        // 不是 silk 错误 → 不生成 silk body
        assert!(err.silk_body().is_none());
        assert_eq!(err.origin(), ErrorOrigin::Upstream);

        // 透传的字节与上游返回的完全一致（含全部字段，无新增、无改写）
        match err {
            GatewayError::UpstreamError(f) => {
                assert_eq!(f.body, raw);
                let json: serde_json::Value = serde_json::from_slice(&f.body).unwrap();
                assert_eq!(json["error"]["code"], "7");
                assert!(json.get("origin").is_none(), "不应给上游错误添加 origin");
                assert!(json.get("message").is_none(), "不应注入顶层 message");
                assert!(!String::from_utf8_lossy(&f.body).contains(GatewayError::SILK_MARKER));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn upstream_error_keeps_status_and_content_type() {
        let err = GatewayError::UpstreamError(upstream_failure());
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(err.error_code(), "upstream_error");
    }

    #[test]
    fn non_json_upstream_body_is_not_truncated_or_rewrapped() {
        // nginx 的 HTML 502：既不是 JSON，也不该被截断或包装成别的形状
        let html = format!("<html><body>{}</body></html>", "x".repeat(2000));
        let failure = UpstreamFailure::new(
            502,
            Bytes::from(html.clone()),
            Some("text/html".to_string()),
        );
        let err = GatewayError::UpstreamError(failure);
        match err {
            GatewayError::UpstreamError(f) => {
                assert_eq!(f.body.len(), html.len(), "原始字节不应被截断");
                assert_eq!(String::from_utf8_lossy(&f.body), html);
                assert_eq!(f.content_type.as_deref(), Some("text/html"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn upstream_message_extraction_handles_shapes() {
        let openai = UpstreamFailure::new(
            401,
            Bytes::from_static(br#"{"error":{"message":"Invalid token."}}"#),
            None,
        );
        assert_eq!(openai.message(), "Invalid token.");

        let flat = UpstreamFailure::new(
            400,
            Bytes::from_static(br#"{"detail":"bad model"}"#),
            None,
        );
        assert_eq!(flat.message(), "bad model");

        let plain = UpstreamFailure::new(503, Bytes::from_static(b"Service Unavailable"), None);
        assert_eq!(plain.message(), "Service Unavailable");
    }

    #[test]
    fn silk_errors_carry_marker_and_origin() {
        let cases: Vec<GatewayError> = vec![
            GatewayError::BadRequest("bad".into()),
            GatewayError::NotFound("nope".into()),
            GatewayError::Transform("x".into()),
            GatewayError::Internal("boom".into()),
            GatewayError::Timeout,
            GatewayError::Serialization("s".into()),
            GatewayError::Unauthorized("Key 错误".into()),
            GatewayError::TooManyRequests,
        ];

        for err in cases {
            assert_eq!(err.origin(), ErrorOrigin::Silk, "{err:?}");
            let body = err.silk_body().expect("silk 错误必须有 body");
            let message = body["message"].as_str().unwrap();
            assert!(
                message.starts_with(GatewayError::SILK_MARKER),
                "message 缺少 【silk】 标记: {message}"
            );
            // 标准 error 对象同样带标记，供 SDK 取错
            assert_eq!(body["error"]["message"].as_str().unwrap(), message, "{err:?}");
            assert_eq!(body["error"]["origin"].as_str().unwrap(), "silk");
            assert_eq!(body["error"]["type"].as_str().unwrap(), err.error_code());
        }
    }

    #[test]
    fn gateway_auth_failure_is_marked_silk_not_upstream() {
        // 网关本地 Key 校验失败：必须标成 silk，否则用户会去改上游 Key
        let err = GatewayError::Unauthorized("Key 错误".to_string());
        let body = err.silk_body().unwrap();
        assert!(body["message"].as_str().unwrap().contains("Key 错误"));
        assert!(body["message"].as_str().unwrap().contains("【silk】"));
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn transport_failure_is_silk_origin_not_upstream() {
        // 上游从未返回过错误响应 —— 不能伪装成上游原话
        let err = GatewayError::Internal("上游请求失败（无详细错误）".to_string());
        assert_eq!(err.origin(), ErrorOrigin::Silk);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn upstream_unreachable_code_differs_from_upstream_error() {
        // 两个 code 必须可区分：前者是 silk 连不上，后者是上游真的返回了错误
        assert_eq!(
            GatewayError::Internal("x".into()).error_code(),
            "internal_error"
        );
        assert_eq!(
            GatewayError::UpstreamError(upstream_failure()).error_code(),
            "upstream_error"
        );
    }

    #[test]
    fn marked_message_matches_origin() {
        let up = GatewayError::UpstreamError(upstream_failure());
        assert!(!up.marked_message().contains("【silk】"));

        let silk = GatewayError::Timeout;
        assert!(silk.marked_message().starts_with("【silk】"));
    }

    #[test]
    fn invalid_upstream_status_falls_back_to_bad_gateway() {
        let err = GatewayError::UpstreamError(UpstreamFailure::new(
            0,
            Bytes::new(),
            None,
        ));
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);
    }
}
