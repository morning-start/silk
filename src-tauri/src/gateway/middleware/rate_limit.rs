use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;

/// 限流计数器（按客户端 IP）
#[derive(Debug)]
struct RateCounter {
    /// 当前分钟的请求数
    request_count: AtomicU64,
    /// 当前分钟的 token 数
    token_count: AtomicU64,
    /// 当前分钟的起始时间
    window_start: RwLock<Instant>,
}

impl RateCounter {
    fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            token_count: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
        }
    }

    /// 检查并增加计数，返回是否允许
    async fn check_and_increment(
        &self,
        max_requests_per_minute: u64,
        max_tokens_per_minute: u64,
        tokens: u64,
    ) -> bool {
        let mut window = self.window_start.write().await;
        let now = Instant::now();

        // 如果当前窗口已过期，重置
        if now.duration_since(*window) >= Duration::from_secs(60) {
            *window = now;
            self.request_count.store(0, Ordering::Relaxed);
            self.token_count.store(0, Ordering::Relaxed);
        }
        drop(window);

        // 检查请求数限制
        let current_requests = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
        if current_requests > max_requests_per_minute {
            return false;
        }

        // 检查 token 数限制
        if max_tokens_per_minute > 0 {
            let current_tokens = self.token_count.fetch_add(tokens, Ordering::Relaxed) + tokens;
            if current_tokens > max_tokens_per_minute {
                return false;
            }
        }

        true
    }
}

/// 限流状态（全局共享）
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// 是否启用限流
    enabled: bool,
    /// 每分钟请求上限
    max_requests_per_minute: u64,
    /// 每分钟 token 上限
    max_tokens_per_minute: u64,
    /// 按客户端 IP 的计数器
    counters: Arc<DashMap<String, Arc<RateCounter>>>,
}

impl RateLimitState {
    pub fn new(enabled: bool, max_requests_per_minute: u64, max_tokens_per_minute: u64) -> Self {
        Self {
            enabled,
            max_requests_per_minute,
            max_tokens_per_minute,
            counters: Arc::new(DashMap::new()),
        }
    }

    /// 检查请求是否允许
    pub async fn check(&self, client_ip: String, tokens: u64) -> bool {
        if !self.enabled {
            return true;
        }

        let counter = self
            .counters
            .entry(client_ip)
            .or_insert_with(|| Arc::new(RateCounter::new()))
            .clone();

        counter
            .check_and_increment(
                self.max_requests_per_minute,
                self.max_tokens_per_minute,
                tokens,
            )
            .await
    }

    /// 清理过期的计数器（定期调用）
    pub fn cleanup(&self) {
        self.counters.retain(|_, counter| {
            // 保留最近活跃的计数器
            let window = counter.window_start.try_read();
            match window {
                Ok(w) => Instant::now().duration_since(*w) < Duration::from_secs(120),
                Err(_) => true, // 如果无法读取，保留
            }
        });
    }
}

fn client_ip_from_headers(ctx: &RequestContext) -> String {
    ctx.headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// 管道级限流检查（在认证之后、路由之前调用）
///
/// 使用 `ip:provider_id` 作为隔离键，实现 per-provider 独立限流。
/// 若 provider 未知则降级为 IP 级别。
pub async fn run(mut ctx: RequestContext, runtime: &GatewayContext) -> Result<RequestContext, StageError> {
    let state = &runtime.rate_limit_state;
    if !state.enabled {
        return Ok(ctx);
    }

    let client_ip = client_ip_from_headers(&ctx);
    let key = match ctx.provider.as_ref() {
        Some(p) => format!("{}:{}", client_ip, p.id),
        None => client_ip,
    };

    if !state.check(key, 0).await {
        let err = GatewayError::TooManyRequests;
        ctx.mark_error(err.to_string(), err.error_code().to_string(), err.status_code());
        return Err(StageError::new(ctx, err));
    }
    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_counter_basic() {
        let counter = RateCounter::new();

        // 前 10 个请求应该通过
        for _ in 0..10 {
            assert!(counter.check_and_increment(10, 0, 0).await);
        }

        // 第 11 个请求应该被拒绝
        assert!(!counter.check_and_increment(10, 0, 0).await);
    }

    #[tokio::test]
    async fn test_rate_counter_tokens() {
        let counter = RateCounter::new();

        // 前 5 个请求，每个 100 tokens，应该通过
        for _ in 0..5 {
            assert!(counter.check_and_increment(100, 500, 100).await);
        }

        // 第 6 个请求会超过 token 限制 (600 > 500)
        assert!(!counter.check_and_increment(100, 500, 100).await);
    }

    #[tokio::test]
    async fn test_rate_limit_state_disabled() {
        let state = RateLimitState::new(false, 1, 1);

        // 即使限制为 1，也应该通过（因为未启用）
        for _ in 0..100 {
            assert!(state.check("127.0.0.1".to_string(), 0).await);
        }
    }
}
