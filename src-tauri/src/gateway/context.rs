use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, Method, Uri};
use sqlx::SqlitePool;
use tokio::sync::{oneshot, RwLock};

use crate::gateway::header_config::HeaderConfig;
use crate::gateway::middleware::rate_limit::RateLimitState;
use crate::models::{GatewaySettings, Provider};

// ---------------------------------------------------------------------------
// GatewayContext
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GatewayContext {
    pub pool: SqlitePool,
    pub settings: Arc<RwLock<GatewaySettings>>,
    pub provider_cache: Arc<ProviderCache>,
    pub log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    /// 共享的 HTTP 客户端（非流式，带超时）
    pub http_client: reqwest::Client,
    /// 共享的 HTTP 客户端（流式，无超时）
    pub http_client_streaming: reqwest::Client,
    /// Header 转发配置
    pub header_config: HeaderConfig,
    /// 网关插件列表
    pub plugins: Vec<Arc<dyn crate::gateway::plugin::GatewayPlugin>>,
    /// 限流状态（全局共享，配置可热更新）
    pub rate_limit_state: RateLimitState,
}

impl GatewayContext {
    pub async fn new(
        pool: SqlitePool,
        settings: Arc<RwLock<GatewaySettings>>,
        provider_cache: Arc<ProviderCache>,
        log_sender: tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
        plugins: Vec<Arc<dyn crate::gateway::plugin::GatewayPlugin>>,
    ) -> Result<Self, String> {
        // 创建共享的 HTTP 客户端（连接池复用，避免每请求创建新 TLS 连接）
        // 流式客户端基于非流式客户端 clone 后修改超时，避免重复构建连接配置
        let http_client = build_http_client()?;
        let http_client_streaming = build_http_client()?;

        let rate_limit_state = {
            let settings_guard = settings.read().await;
            RateLimitState::new(
                settings_guard.rate_limit_enabled,
                settings_guard.rate_limit_max_requests_per_minute as u64,
                settings_guard.rate_limit_max_tokens_per_minute as u64,
            )
        };

        Ok(Self {
            pool,
            settings,
            provider_cache,
            log_sender,
            http_client,
            http_client_streaming,
            header_config: HeaderConfig::default(),
            plugins,
            rate_limit_state,
        })
    }
}

// ---------------------------------------------------------------------------
// HTTP 客户端构建（含系统代理支持）
// ---------------------------------------------------------------------------

/// 构建共享 HTTP 客户端：自动继承 Windows 系统代理设置。
///
/// 背景：用户环境常通过 Clash 等工具配置系统代理（注册表
/// `Internet Settings` → ProxyEnable/ProxyServer，如 `socks=127.0.0.1:9000`），
/// 且 DNS 返回 fake-ip 段（如 198.18.x.x）。若 reqwest 不走代理直连，
/// 会因 DNS 解析到 fake-ip 而连接失败（"dns error 11001 不知道这样的主机"）。
/// 因此构建客户端时读取注册表系统代理并注入。
fn build_http_client() -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().connect_timeout(Duration::from_secs(10));
    #[cfg(target_os = "windows")]
    {
        if let Some(proxy) = windows_system_proxy() {
            builder = builder.proxy(proxy);
            tracing::info!(proxy = %windows_system_proxy_str().unwrap_or_default(), "HTTP 客户端已启用系统代理");
        } else {
            tracing::debug!("未检测到系统代理，HTTP 客户端直连");
        }
    }
    builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// 读取 Windows 注册表系统代理设置并构建 reqwest Proxy
#[cfg(target_os = "windows")]
fn windows_system_proxy() -> Option<reqwest::Proxy> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;
    let proxy_enable: u32 = settings.get_value("ProxyEnable").ok()?;
    if proxy_enable == 0 {
        return None;
    }
    let proxy_server: String = settings.get_value("ProxyServer").ok()?;
    if proxy_server.trim().is_empty() {
        return None;
    }
    // 常见格式：`socks=127.0.0.1:9000`、`http=127.0.0.1:7890;https=127.0.0.1:7890`
    // 或纯 `127.0.0.1:7890`。优先取 socks，其次 http/https。
    let normalized = if let Some(addr) = proxy_server.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix("socks=").map(|a| format!("socks5://{a}"))
    }) {
        addr
    } else if let Some(addr) = proxy_server.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix("http=").map(|a| format!("http://{a}"))
    }) {
        addr
    } else if proxy_server.contains("://") {
        proxy_server.clone()
    } else {
        format!("http://{proxy_server}")
    };
    reqwest::Proxy::all(&normalized).ok()
}

/// 读取系统代理字符串（仅用于日志展示）
#[cfg(target_os = "windows")]
fn windows_system_proxy_str() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;
    let proxy_server: String = settings.get_value("ProxyServer").ok()?;
    Some(proxy_server)
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
/// RequestContext 中所有可 Clone 的字段（排除 response 字段）
///
/// 使用 #[derive(Clone)] 消除手动逐字段 clone 的样板代码。
/// 通过 Deref/DerefMut 使中间件代码透明地访问 inner 字段，无需修改 field access 模式。
#[derive(Clone)]
pub struct RequestContextInner {
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
    /// 缓存的解析后的 JSON 请求体
    pub parsed_body: Option<serde_json::Value>,
    pub provider: Option<Provider>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub final_status: Option<axum::http::StatusCode>,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub upstream_status: Option<axum::http::StatusCode>,
    pub upstream_headers: Option<HeaderMap>,
    pub upstream_body: Option<bytes::Bytes>,
    /// 已发送给客户端的响应字节数（流式场景）
    pub response_bytes_sent: u64,
    /// 最后收到的 SSE 事件 ID（用于断线重连）
    pub last_event_id: Option<String>,
    /// 远程模型名覆盖（来自 model_mapping_channels 的 selected_models）
    pub remote_model_override: Option<String>,
    /// 认证通过的 Gateway Key 名称（用于日志记录）
    pub auth_key_name: Option<String>,
    /// 渠道映射选中的上游 API Key
    pub selected_api_key: Option<String>,
    /// 渠道映射选中的 Key 名称
    pub channel_key_name: Option<String>,
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
    /// 累计回退尝试次数（换 Key + 换 Provider），用于日志统计
    pub total_retry_attempts: u32,
    /// 客户端原始请求是否指定了 stream: true
    pub client_requested_stream: bool,
    /// 请求体是否已执行过跨协议转换（failover 换 Key 重试时避免重复转换导致 400）
    pub request_transformed: bool,
}

/// 请求上下文 — 整个网关管道的核心数据结构
///
/// 设计说明：
/// - `response` 字段（axum::Response，非 Clone）单独存放在外层 struct，
///   其余所有字段通过 `RequestContextInner` 自动 derive Clone。
/// - 通过 `Deref` / `DerefMut` 透明代理到 inner，中间件代码无需感知拆分。
/// - `clone()` 时 `response` 置为 `None`（克隆上下文时不应携带已构建的响应）。
pub struct RequestContext {
    pub response: Option<axum::response::Response>,
    /// SSE 流完成后通知接收端（由 handle_sse_response 设置，pipeline 消费）
    pub stream_complete_rx: Option<oneshot::Receiver<()>>,
    /// SSE 流处理共享状态（读取任务 → 主线程，流结束后 pipeline 读取统计）
    pub stream_shared: Option<Arc<RwLock<StreamSharedState>>>,
    inner: RequestContextInner,
}

impl Clone for RequestContext {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            response: None,
            stream_complete_rx: None,
            stream_shared: self.stream_shared.clone(),
        }
    }
}

impl std::ops::Deref for RequestContext {
    type Target = RequestContextInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for RequestContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
    /// 上游返回的精确 prompt_tokens（从最终 SSE chunk 的 usage 字段解析）
    pub exact_prompt_tokens: Option<i64>,
    /// 上游返回的精确 completion_tokens（从最终 SSE chunk 的 usage 字段解析）
    pub exact_completion_tokens: Option<i64>,
}

impl StreamSharedState {
    /// 记录上游上报的 token 用量
    ///
    /// 用量通常出现在流的最后一个 chunk，后到的值覆盖先前的值（取最终值）。
    pub fn record_usage(&mut self, usage: &crate::gateway::middleware::stream_response::Usage) {
        if let Some(input) = usage.input_tokens {
            self.exact_prompt_tokens = Some(input);
        }
        if let Some(output) = usage.output_tokens {
            self.exact_completion_tokens = Some(output);
        }
    }
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
            inner: RequestContextInner {
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
                parsed_body: None,
                provider: None,
                inbound_protocol: None,
                outbound_protocol: None,
                final_status: None,
                error_message: None,
                error_code: None,
                upstream_status: None,
                upstream_headers: None,
                upstream_body: None,
                response_bytes_sent: 0,
                last_event_id: None,
                remote_model_override: None,
                auth_key_name: None,
                selected_api_key: None,
                channel_key_name: None,
                upstream_url: None,
                upstream_method: None,
                failed_keys: Vec::new(),
                failed_providers: Vec::new(),
                channels_available: Vec::new(),
                total_retry_attempts: 0,
                client_requested_stream: false,
                request_transformed: false,
            },
            response: None,
            stream_complete_rx: None,
            stream_shared: None,
        }
    }

    pub fn get_parsed_body(&mut self) -> Option<&serde_json::Value> {
        if self.parsed_body.is_none() && !self.request_body.is_empty() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&self.request_body) {
                self.parsed_body = Some(json);
            }
        }
        self.parsed_body.as_ref()
    }

    pub fn update_body(&mut self, json: serde_json::Value) -> Result<(), serde_json::Error> {
        let new_body = serde_json::to_vec(&json)?;
        self.request_body = bytes::Bytes::from(new_body);
        self.parsed_body = Some(json);
        Ok(())
    }

    /// 就地修改已解析的请求体，并同步序列化回 `request_body`
    ///
    /// 统一走这个方法可以保证 `parsed_body` 缓存始终与 `request_body` 一致
    /// （直接改 `request_body` 会让缓存变成脏数据）。
    /// 请求体为空或不是合法 JSON 时不执行任何修改。
    pub fn edit_body<F>(&mut self, edit: F) -> Result<(), serde_json::Error>
    where
        F: FnOnce(&mut serde_json::Value),
    {
        let Some(mut json) = self.get_parsed_body().cloned() else {
            return Ok(());
        };
        edit(&mut json);
        self.update_body(json)
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
