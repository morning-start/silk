//! 失败回退调度器（策略模式）
//!
//! 业务层把「一次完整尝试」封装成闭包交给调度器，调度器统一负责：
//! 循环调度、总超时、尝试次数上限、升级回退（换渠道）与终止判定。
//!
//! 用法：
//! ```text
//! let scheduler = FailoverScheduler::new(FailoverPolicy {
//!     max_attempts: 10,
//!     total_timeout: Duration::from_secs(15),
//! });
//! let result = scheduler
//!     .run(initial_ctx, attempt_closure, next_channel_closure)
//!     .await;
//! ```
//!
//! - 输入：初始上下文 + 尝试闭包（`FnMut(ctx, start) -> Result<AttemptOutcome<T>, E>`）
//!   + 升级闭包（`FnMut(ctx) -> Option<T>`）
//! - 输出：`Result<T, FailoverError<T, E>>`

use std::future::Future;
use std::time::{Duration, Instant};

/// 单次尝试的结论（由尝试闭包产生，调度器据此决策）
pub enum AttemptOutcome<T> {
    /// 成功，携带最终结果
    Success(T),
    /// 同级重试（如换 Key 后重试）
    Retry(T),
    /// 升级到下一级回退（如换渠道）
    Escalate(T),
}

/// 失败回退策略参数（实例化调度器时传入，可配置）
pub struct FailoverPolicy {
    /// 最大失败尝试次数（达到即终止）
    pub max_attempts: u32,
    /// 总超时（从 run 开始计时，超出即终止）
    pub total_timeout: Duration,
}

/// 调度终止原因
#[derive(Debug)]
pub enum FailoverError<T, E> {
    /// 总超时
    Timeout(T),
    /// 达到最大尝试次数
    TooManyAttempts(T),
    /// 所有回退级别耗尽（升级闭包返回 None）
    NoChannelLeft(T),
    /// 尝试闭包返回的终止错误（不可重试），原样上抛
    Aborted(E),
}

/// 失败回退调度器：策略参数化，业务只提供「尝试」与「升级」两个闭包
pub struct FailoverScheduler {
    policy: FailoverPolicy,
}

impl FailoverScheduler {
    pub fn new(policy: FailoverPolicy) -> Self {
        Self { policy }
    }

    /// 统一入口：执行失败回退调度
    ///
    /// - `initial`：初始上下文
    /// - `attempt`：执行一次完整尝试。返回 `Success` 结束；
    ///   `Retry` 继续同级重试；`Escalate` 转交 `next_channel` 升级；
    ///   `Err` 表示不可重试错误，立即以 `Aborted` 终止。
    /// - `next_channel`：收到 `Escalate` 后升一级回退，返回 `None` 表示无下一级。
    pub async fn run<T, E, F, Fut, G, GFut>(
        &self,
        initial: T,
        mut attempt: F,
        mut next_channel: G,
    ) -> Result<T, FailoverError<T, E>>
    where
        T: Clone,
        F: FnMut(T, Instant) -> Fut,
        Fut: Future<Output = Result<AttemptOutcome<T>, E>>,
        G: FnMut(T) -> GFut,
        GFut: Future<Output = Option<T>>,
    {
        let failover_start = Instant::now();
        let mut ctx = initial;
        let mut attempts: u32 = 0;

        loop {
            // 总超时检查
            if failover_start.elapsed() > self.policy.total_timeout {
                return Err(FailoverError::Timeout(ctx));
            }
            // 尝试次数上限检查
            if attempts >= self.policy.max_attempts {
                return Err(FailoverError::TooManyAttempts(ctx));
            }

            match attempt(ctx, failover_start).await {
                Ok(AttemptOutcome::Success(new_ctx)) => return Ok(new_ctx),
                Ok(AttemptOutcome::Retry(new_ctx)) => {
                    ctx = new_ctx;
                    attempts += 1;
                }
                Ok(AttemptOutcome::Escalate(new_ctx)) => {
                    attempts += 1;
                    match next_channel(new_ctx.clone()).await {
                        Some(next) => ctx = next,
                        None => return Err(FailoverError::NoChannelLeft(new_ctx)),
                    }
                }
                Err(error) => return Err(FailoverError::Aborted(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Debug, PartialEq)]
    struct Ctx {
        id: u32,
    }

    fn policy(max_attempts: u32, secs: u64) -> FailoverPolicy {
        FailoverPolicy {
            max_attempts,
            total_timeout: Duration::from_secs(secs),
        }
    }

    #[tokio::test]
    async fn success_on_first_try() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                move |ctx, _start| {
                    c.fetch_add(1, Ordering::Relaxed);
                    async move { Ok(AttemptOutcome::Success(ctx)) }
                },
                |_ctx| async move { unreachable!("不应触发升级") },
            )
            .await;
        assert_eq!(result.unwrap(), Ctx { id: 0 });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn retry_then_success() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                |ctx, _start| async move {
                    if ctx.id == 0 {
                        Ok(AttemptOutcome::Retry(Ctx { id: 1 }))
                    } else {
                        Ok(AttemptOutcome::Success(ctx))
                    }
                },
                |_ctx| async move { None },
            )
            .await;
        assert_eq!(result.unwrap(), Ctx { id: 1 });
    }

    #[tokio::test]
    async fn escalate_then_success() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let escalations = Arc::new(AtomicU32::new(0));
        let count = escalations.clone();
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                |ctx, _start| async move {
                    if ctx.id == 1 {
                        Ok(AttemptOutcome::Success(ctx))
                    } else {
                        Ok(AttemptOutcome::Escalate(ctx))
                    }
                },
                move |ctx| {
                    let counter = escalations.clone();
                    async move {
                        counter.fetch_add(1, Ordering::Relaxed);
                        Some(Ctx { id: ctx.id + 1 })
                    }
                },
            )
            .await;
        assert_eq!(result.unwrap(), Ctx { id: 1 });
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn too_many_attempts() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                |ctx, _start| async move { Ok(AttemptOutcome::Retry(ctx)) },
                |_ctx| async move { None },
            )
            .await;
        assert!(matches!(
            result,
            Err(FailoverError::TooManyAttempts(Ctx { id: 0 }))
        ));
    }

    #[tokio::test]
    async fn no_channel_left() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                |ctx, _start| async move { Ok(AttemptOutcome::Escalate(ctx)) },
                |_ctx| async move { None },
            )
            .await;
        assert!(matches!(result, Err(FailoverError::NoChannelLeft(_))));
    }

    #[tokio::test]
    async fn aborted_error_propagates() {
        let scheduler = FailoverScheduler::new(policy(3, 5));
        let result = scheduler
            .run(
                Ctx { id: 0 },
                |_ctx, _start| async move {
                    Err::<AttemptOutcome<Ctx>, &'static str>("boom")
                },
                |_ctx| async move { None },
            )
            .await;
        assert!(matches!(result, Err(FailoverError::Aborted("boom"))));
    }

    #[tokio::test]
    async fn timeout_terminates() {
        let scheduler = FailoverScheduler::new(FailoverPolicy {
            max_attempts: 100,
            total_timeout: Duration::from_millis(10),
        });
        let result: Result<Ctx, FailoverError<Ctx, &'static str>> = scheduler
            .run(
                Ctx { id: 0 },
                |ctx, _start| async move {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    Ok(AttemptOutcome::Retry(ctx))
                },
                |_ctx| async move { None },
            )
            .await;
        assert!(matches!(result, Err(FailoverError::Timeout(_))));
    }
}
