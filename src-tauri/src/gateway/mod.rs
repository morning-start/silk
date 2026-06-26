use std::sync::Arc;
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use reqwest::Client;
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::models::{GatewaySettings, NewRequestLog, Provider, RoutingRule};
use crate::persistence::{GatewaySettingsRepo, LogRepo, ProviderRepo, RoutingRuleRepo};

const REQUEST_BODY_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct GatewayContext {
    pub pool: SqlitePool,
    pub settings: Arc<RwLock<GatewaySettings>>,
    pub route_manager: Arc<RouteManager>,
}

impl GatewayContext {
    pub fn new(
        pool: SqlitePool,
        settings: Arc<RwLock<GatewaySettings>>,
        route_manager: Arc<RouteManager>,
    ) -> Self {
        Self {
            pool,
            settings,
            route_manager,
        }
    }
}

#[derive(Clone)]
pub struct RouteManager {
    routes: Arc<RwLock<Vec<RoutingRule>>>,
}

impl RouteManager {
    pub async fn load(pool: &SqlitePool) -> Result<Self, sqlx::Error> {
        let routes = RoutingRuleRepo::find_enabled_ordered(pool).await?;
        Ok(Self {
            routes: Arc::new(RwLock::new(routes)),
        })
    }

    pub async fn reload(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let routes = RoutingRuleRepo::find_enabled_ordered(pool).await?;
        *self.routes.write().await = routes;
        Ok(())
    }

    pub async fn resolve(
        &self,
        host: Option<&str>,
        method: &str,
        path: &str,
        content_type: Option<&str>,
    ) -> Option<RoutingRule> {
        let routes = self.routes.read().await;
        routes
            .iter()
            .find(|route| route.matches(host, method, path, content_type))
            .cloned()
    }
}

pub struct GatewayServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    join_handle: JoinHandle<()>,
}

impl GatewayServerHandle {
    pub async fn stop(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.join_handle.await;
    }
}

pub async fn load_gateway_context(pool: &SqlitePool) -> Result<GatewayContext, sqlx::Error> {
    let settings = GatewaySettingsRepo::load_effective(pool).await?;
    let route_manager = RouteManager::load(pool).await?;

    Ok(GatewayContext::new(
        pool.clone(),
        Arc::new(RwLock::new(settings)),
        Arc::new(route_manager),
    ))
}

pub async fn spawn_gateway_server(
    context: GatewayContext,
) -> Result<GatewayServerHandle, std::io::Error> {
    let settings = context.settings.read().await.clone();
    let addr = format!("{}:{}", settings.bind_host, settings.bind_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let app = build_router(context);

    let join_handle = tokio::spawn(async move {
        let server = axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

        if let Err(err) = server.await {
            tracing::error!(%err, "gateway server stopped with error");
        }
    });

    Ok(GatewayServerHandle {
        shutdown: Some(shutdown_tx),
        join_handle,
    })
}

fn build_router(context: GatewayContext) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .fallback(proxy_handler)
        .with_state(context)
}

async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "silk-gateway"
    }))
}

async fn proxy_handler(
    State(context): State<GatewayContext>,
    req: Request<Body>,
) -> Result<Response, GatewayError> {
    let started_at = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let host = header_value_as_str(headers.get(axum::http::header::HOST));
    let content_type = header_value_as_str(headers.get(axum::http::header::CONTENT_TYPE));
    let path = request_path(&uri);
    let request_body = to_bytes(req.into_body(), REQUEST_BODY_LIMIT)
        .await
        .map_err(|err| GatewayError::BadRequest(format!("读取请求体失败: {err}")))?;

    let route = context
        .route_manager
        .resolve(host, method.as_str(), &path, content_type)
        .await
        .ok_or_else(|| GatewayError::NotFound(format!("未找到匹配路由: {method} {path}")))?;

    let provider = ProviderRepo::find_by_id(&context.pool, &route.target_provider_id)
        .await
        .map_err(GatewayError::Database)?
        .ok_or_else(|| GatewayError::NotFound(format!("未找到目标 Provider: {}", route.target_provider_id)))?;

    let response = forward_request(
        &context,
        &route,
        &provider,
        method,
        &uri,
        headers,
        request_body,
        request_id,
        started_at,
    )
    .await?;

    Ok(response)
}

async fn forward_request(
    context: &GatewayContext,
    route: &RoutingRule,
    provider: &Provider,
    method: axum::http::Method,
    uri: &Uri,
    headers: HeaderMap,
    request_body: bytes::Bytes,
    request_id: String,
    started_at: Instant,
) -> Result<Response, GatewayError> {
    let upstream_url = build_upstream_url(&provider.api_base_url, uri)?;
    let client = Client::builder()
        .timeout(provider.timeout())
        .build()
        .map_err(GatewayError::Upstream)?;

    let mut upstream_request = client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes())
            .map_err(|err| GatewayError::BadRequest(format!("不支持的方法: {err}")))?,
        upstream_url,
    );

    for (name, value) in headers.iter() {
        if should_forward_header(name) {
            upstream_request = upstream_request.header(name, value);
        }
    }

    let upstream_response = upstream_request
        .body(request_body.clone())
        .send()
        .await
        .map_err(GatewayError::Upstream)?;

    let status = upstream_response.status();
    let response_headers = upstream_response.headers().clone();
    let response_body = upstream_response
        .bytes()
        .await
        .map_err(GatewayError::Upstream)?;

    let elapsed_ms = started_at.elapsed().as_millis() as i64;
    let request_headers_json = headers_to_json(&headers);
    let response_headers_json = headers_to_json(&response_headers);
    let request_body_text = maybe_body_text(&request_body);
    let response_body_text = maybe_body_text(&response_body);

    let log = NewRequestLog {
        request_id,
        method: method.to_string(),
        path: request_path(uri),
        route_id: Some(route.id.clone()),
        inbound_protocol: route.inbound_protocol.clone(),
        outbound_protocol: route.outbound_protocol.clone(),
        request_headers: request_headers_json,
        request_body: request_body_text,
        response_status: Some(status.as_u16() as i64),
        status_code: Some(status.as_u16() as i64),
        response_headers: response_headers_json,
        response_body: response_body_text,
        duration_ms: Some(elapsed_ms),
        provider_id: Some(provider.id.clone()),
        error_message: None,
        error_code: None,
        model_used: route
            .model_name_override
            .clone()
            .or_else(|| provider.model_name.clone()),
        retry_count: Some(0),
        stream_enabled: Some(false),
        cache_hit: Some(false),
        request_size_bytes: Some(request_body.len() as i64),
        response_size_bytes: Some(response_body.len() as i64),
        tokens_input: None,
        tokens_output: None,
    };

    let _ = LogRepo::insert(&context.pool, &log).await;

    Ok(build_response(status, response_headers, response_body))
}

fn build_response(
    status: reqwest::StatusCode,
    headers: HeaderMap,
    body: bytes::Bytes,
) -> Response {
    let mut response = Response::builder().status(status);
    {
        let response_headers = response.headers_mut().expect("response headers available");
        for (name, value) in headers.iter() {
            if should_forward_header(name) {
                response_headers.insert(name, value.clone());
            }
        }
    }

    response
        .body(Body::from(body))
        .unwrap_or_else(|err| GatewayError::Internal(err.to_string()).into_response())
}

fn build_upstream_url(base_url: &str, uri: &Uri) -> Result<reqwest::Url, GatewayError> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|err| GatewayError::BadRequest(format!("无效的上游地址: {err}")))?;
    url.set_path(uri.path());
    url.set_query(uri.query());
    Ok(url)
}

fn request_path(uri: &Uri) -> String {
    uri.path().to_string()
}

fn header_value_as_str(value: Option<&HeaderValue>) -> Option<&str> {
    value.and_then(|v| v.to_str().ok())
}

fn should_forward_header(name: &HeaderName) -> bool {
    !matches!(
        name,
        &axum::http::header::HOST
            | &axum::http::header::CONTENT_LENGTH
            | &axum::http::header::TRANSFER_ENCODING
            | &axum::http::header::CONNECTION
            | &axum::http::header::UPGRADE
    )
}

fn headers_to_json(headers: &HeaderMap) -> Option<String> {
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

fn maybe_body_text(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    String::from_utf8(body.to_vec()).ok()
}

#[derive(Debug, Serialize)]
struct GatewayErrorPayload {
    message: String,
}

#[derive(thiserror::Error, Debug)]
enum GatewayError {
    #[error("请求错误: {0}")]
    BadRequest(String),
    #[error("未找到: {0}")]
    NotFound(String),
    #[error("上游请求失败: {0}")]
    Upstream(#[from] reqwest::Error),
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
    #[error("内部错误: {0}")]
    Internal(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let status = match self {
            GatewayError::BadRequest(_) => StatusCode::BAD_REQUEST,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::Upstream(_) => StatusCode::BAD_GATEWAY,
            GatewayError::Database(_) | GatewayError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = axum::Json(GatewayErrorPayload {
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}
