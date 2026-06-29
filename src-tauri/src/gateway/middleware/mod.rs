pub mod authenticate;
pub mod dispatch_upstream;
pub mod extract;
pub mod finalize;

pub mod persist_log;
pub mod resolve_route;
pub mod select_channel;
pub mod stream_response;
pub mod transform_request;
pub mod transform_response;

// 内部工具函数（不对外暴露）
pub(crate) mod internals {
    use axum::http::{HeaderMap, HeaderName, HeaderValue, Uri};
    use axum::response::IntoResponse;

    use crate::gateway::error::GatewayError;

    pub fn build_upstream_url(base_url: &str, uri: &Uri) -> Result<reqwest::Url, GatewayError> {
        let mut url = reqwest::Url::parse(base_url)
            .map_err(|err| GatewayError::BadRequest(format!("无效的上游地址: {err}")))?;
        url.set_path(uri.path());
        url.set_query(uri.query());
        Ok(url)
    }

    pub fn should_forward_header(name: &HeaderName) -> bool {
        !matches!(
            name,
            &axum::http::header::HOST
                | &axum::http::header::CONTENT_LENGTH
                | &axum::http::header::TRANSFER_ENCODING
                | &axum::http::header::CONNECTION
                | &axum::http::header::UPGRADE
        )
    }

    pub fn headers_to_json(headers: &HeaderMap) -> Option<String> {
        let pairs = headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|text| (name.as_str().to_string(), text.to_string()))
            })
            .collect::<Vec<_>>();

        if pairs.is_empty() {
            None
        } else {
            serde_json::to_string(&pairs).ok()
        }
    }

    pub fn maybe_body_text(body: &[u8]) -> Option<String> {
        if body.is_empty() {
            return None;
        }
        String::from_utf8(body.to_vec()).ok()
    }

    pub fn build_response(
        status: axum::http::StatusCode,
        headers: HeaderMap,
        body: bytes::Bytes,
    ) -> axum::response::Response {
        let mut response = axum::response::Response::builder().status(status);
        {
            let response_headers = response.headers_mut().expect("response headers available");
            for (name, value) in headers.iter() {
                if should_forward_header(name) {
                    response_headers.insert(name, value.clone());
                }
            }
        }

        response
            .body(axum::body::Body::from(body))
            .unwrap_or_else(|err| GatewayError::Internal(err.to_string()).into_response())
    }

    pub fn request_path(uri: &Uri) -> String {
        uri.path().to_string()
    }

    pub fn header_value_as_str(value: Option<&HeaderValue>) -> Option<&str> {
        value.and_then(|v| v.to_str().ok())
    }
}

// 公开重导出（供 pipeline 和中间件使用）
pub use internals::*;
