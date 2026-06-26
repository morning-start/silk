use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, Method, Uri};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::gateway::group_manager::GroupManager;
use crate::models::{GatewaySettings, Provider, RoutingRule};
use crate::persistence::{GatewaySettingsRepo, GroupRepo, RoutingRuleRepo};
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
        Self {
            pool,
            settings,
            route_manager,
            provider_cache,
            log_sender,
            adapter_registry,
            group_manager,
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
    pub body: bytes::Bytes,
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
            body: self.body.clone(),
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
            body: bytes::Bytes::new(),
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
        self.body.len() as i64
    }

    pub fn response_size_bytes(&self) -> Option<i64> {
        self.upstream_body.as_ref().map(|body| body.len() as i64)
    }

    pub fn elapsed_ms(&self) -> i64 {
        self.started_at.elapsed().as_millis() as i64
    }
}

// ---------------------------------------------------------------------------
// Helper: load_gateway_context
// ---------------------------------------------------------------------------

pub async fn load_gateway_context(
    pool: SqlitePool,
    log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
) -> Result<GatewayContext, sqlx::Error> {
    let settings = GatewaySettingsRepo::load_effective(&pool).await?;
    let route_manager = RouteManager::load(&pool).await?;
    let provider_cache = Arc::new(ProviderCache::new(Duration::from_secs(300)));
    let adapter_registry = Arc::new(AdapterRegistry::new());
    let group_manager = Arc::new(GroupManager::new());
    group_manager.load(&pool).await?;

    Ok(GatewayContext::new(
        pool,
        Arc::new(RwLock::new(settings)),
        Arc::new(route_manager),
        provider_cache,
        log_sender,
        adapter_registry,
        group_manager,
    ))
}
