use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, Method, Uri};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::gateway::group_manager::GroupManager;
use crate::models::{GatewaySettings, Provider, RoutingRule};
use crate::persistence::RoutingRuleRepo;
use crate::protocol::AdapterRegistry;

// ---------------------------------------------------------------------------
// GatewayContext
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GatewayContext {
    pub pool: SqlitePool,
    pub settings: Arc<RwLock<GatewaySettings>>,
    pub route_manager: Arc<RouteManager>,
    pub provider_cache: Arc<ProviderCache>,
    pub log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    pub adapter_registry: Arc<AdapterRegistry>,
    pub group_manager: Arc<GroupManager>,
    /// 共享的 HTTP 客户端（非流式，带超时）
    pub http_client: reqwest::Client,
    /// 共享的 HTTP 客户端（流式，无超时）
    pub http_client_streaming: reqwest::Client,
}

impl GatewayContext {
    pub fn new(
        pool: SqlitePool,
        settings: Arc<RwLock<GatewaySettings>>,
        route_manager: Arc<RouteManager>,
        provider_cache: Arc<ProviderCache>,
        log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
        adapter_registry: Arc<AdapterRegistry>,
        group_manager: Arc<GroupManager>,
    ) -> Self {
        // 创建共享 HTTP 客户端（连接池复用，避免每请求创建新 TLS 连接）
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let http_client_streaming = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create streaming HTTP client");

        Self {
            pool,
            settings,
            route_manager,
            provider_cache,
            log_sender,
            adapter_registry,
            group_manager,
            http_client,
            http_client_streaming,
        }
    }
}

// ---------------------------------------------------------------------------
// RouteManager
// ---------------------------------------------------------------------------

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

    /// 根据路由规则解析目标 Provider ID
    ///
    /// 如果规则指向 group，通过 GroupManager 选择一个 Provider；
    /// 如果规则直接指向 provider，返回该 provider_id。
    pub async fn resolve_provider_id(
        &self,
        route: &RoutingRule,
        group_manager: &GroupManager,
    ) -> Option<String> {
        if let Some(ref group_id) = route.target_group_id {
            // 从分组中选择一个 Provider
            group_manager
                .select_provider(group_id)
                .await
                .map(|member| member.provider_id)
        } else {
            Some(route.target_provider_id.clone())
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderCache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CachedProvider {
    provider: Provider,
    expires_at: Instant,
}

impl CachedProvider {
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

#[derive(Clone)]
pub struct ProviderCache {
    inner: Arc<RwLock<HashMap<String, CachedProvider>>>,
    ttl: Duration,
}

impl ProviderCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// 从缓存获取 Provider，miss 或过期返回 None
    pub async fn get(&self, id: &str) -> Option<Provider> {
        let map = self.inner.read().await;
        let cached = map.get(id)?;
        if cached.is_expired() {
            None
        } else {
            Some(cached.provider.clone())
        }
    }

    /// 将 Provider 写入缓存
    pub async fn put(&self, provider: Provider) {
        let mut map = self.inner.write().await;
        map.insert(
            provider.id.clone(),
            CachedProvider {
                provider,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// 清除指定 Provider 的缓存（配置更新时调用）
    pub async fn invalidate(&self, id: &str) {
        let mut map = self.inner.write().await;
        map.remove(id);
    }

    /// 清除所有缓存
    pub async fn clear(&self) {
        let mut map = self.inner.write().await;
        map.clear();
    }
}

// ---------------------------------------------------------------------------
// RequestContext
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RequestContext {
    pub request_id: String,
    pub started_at: Instant,
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub host: Option<String>,
    pub content_type: Option<String>,
    pub path: String,
    /// 客户端原始请求体（初始化时设置，永不修改）
    pub client_body: bytes::Bytes,
    /// 管道中流转的请求体（可被 resolve_route / transform_request 修改）
    pub request_body: bytes::Bytes,
    pub route: Option<RoutingRule>,
    pub provider: Option<Provider>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub final_status: Option<axum::http::StatusCode>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub upstream_status: Option<axum::http::StatusCode>,
    pub upstream_headers: Option<HeaderMap>,
    pub upstream_body: Option<bytes::Bytes>,
    pub response: Option<axum::response::Response>,
    /// 已发送给客户端的响应字节数（流式场景）
    pub response_bytes_sent: u64,
    /// 最后收到的 SSE 事件 ID（用于断线重连）
    pub last_event_id: Option<String>,
    /// 适配器注册表（用于协议转换）
    pub adapter_registry: Arc<crate::protocol::AdapterRegistry>,
    /// 远程模型名覆盖（来自 model_mapping_channels 的 selected_models）
    pub remote_model_override: Option<String>,
    /// 认证通过的 Gateway Key 名称（用于日志记录）
    pub auth_key_name: Option<String>,
    /// 渠道映射选中的上游 API Key
    pub selected_api_key: Option<String>,
    /// 适配器指定的上游 URL（覆盖原始请求 URI）
    pub upstream_url: Option<String>,
    /// 适配器指定的上游 HTTP 方法（覆盖原始请求方法）
    pub upstream_method: Option<String>,
    /// 失败回退：已尝试失败的上游 Key 列表
    pub failed_keys: Vec<String>,
    /// 失败回退：已尝试失败的 Provider ID 列表
    pub failed_providers: Vec<String>,
    /// 模型池映射中可用的渠道列表（provider_id），用于失败回退
    pub channels_available: Vec<String>,
}

impl Clone for RequestContext {
    fn clone(&self) -> Self {
        Self {
            request_id: self.request_id.clone(),
            started_at: self.started_at,
            method: self.method.clone(),
            uri: self.uri.clone(),
            headers: self.headers.clone(),
            host: self.host.clone(),
            content_type: self.content_type.clone(),
            path: self.path.clone(),
            client_body: self.client_body.clone(),
            request_body: self.request_body.clone(),
            route: self.route.clone(),
            provider: self.provider.clone(),
            inbound_protocol: self.inbound_protocol.clone(),
            outbound_protocol: self.outbound_protocol.clone(),
            final_status: self.final_status,
            error_message: self.error_message.clone(),
            error_code: self.error_code.clone(),
            upstream_status: self.upstream_status,
            upstream_headers: self.upstream_headers.clone(),
            upstream_body: self.upstream_body.clone(),
            response: None,
            response_bytes_sent: self.response_bytes_sent,
            last_event_id: self.last_event_id.clone(),
            adapter_registry: self.adapter_registry.clone(),
            remote_model_override: self.remote_model_override.clone(),
            auth_key_name: self.auth_key_name.clone(),
            selected_api_key: self.selected_api_key.clone(),
            upstream_url: self.upstream_url.clone(),
            upstream_method: self.upstream_method.clone(),
            failed_keys: self.failed_keys.clone(),
            failed_providers: self.failed_providers.clone(),
            channels_available: self.channels_available.clone(),
        }
    }
}

/// 流处理共享状态（读取任务 → 主线程）
#[derive(Debug, Default)]
pub struct StreamSharedState {
    /// 已发送给客户端的响应字节数
    pub bytes_sent: u64,
    /// 最后收到的 SSE 事件 ID
    pub last_event_id: Option<String>,
    /// 已接收的上游字节数
    pub bytes_received: u64,
}

impl RequestContext {
    pub fn new(
        request_id: String,
        started_at: Instant,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
    ) -> Self {
        let host = headers
            .get(axum::http::header::HOST)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let content_type = headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        Self {
            request_id,
            started_at,
            path: uri.path().to_string(),
            method,
            uri,
            headers,
            host,
            content_type,
            client_body: bytes::Bytes::new(),
            request_body: bytes::Bytes::new(),
            route: None,
            provider: None,
            inbound_protocol: None,
            outbound_protocol: None,
            final_status: None,
            error_message: None,
            error_code: None,
            upstream_status: None,
            upstream_headers: None,
            upstream_body: None,
            response: None,
            response_bytes_sent: 0,
            last_event_id: None,
            adapter_registry: Arc::new(crate::protocol::AdapterRegistry::new()),
            remote_model_override: None,
            auth_key_name: None,
            selected_api_key: None,
            upstream_url: None,
            upstream_method: None,
            failed_keys: Vec::new(),
            failed_providers: Vec::new(),
            channels_available: Vec::new(),
        }
    }

    pub fn mark_error(
        &mut self,
        error_message: String,
        error_code: String,
        status: axum::http::StatusCode,
    ) {
        self.error_message = Some(error_message);
        self.error_code = Some(error_code);
        self.final_status = Some(status);
    }

    pub fn request_size_bytes(&self) -> i64 {
        self.client_body.len() as i64
    }

    pub fn response_size_bytes(&self) -> Option<i64> {
        self.upstream_body.as_ref().map(|body| body.len() as i64)
    }

    pub fn elapsed_ms(&self) -> i64 {
        self.started_at.elapsed().as_millis() as i64
    }
}

