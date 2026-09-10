//! 调度器与策略抽象层
//!
//! 本目录存放与业务解耦、可独立测试复用的调度器，均采用策略模式：
//! 实例化调度器时传入策略参数，调用统一入口，传入标准输入、返回标准输出。
//!
//! - [`failover`]：失败回退调度器
//!   `FailoverScheduler::new(FailoverPolicy{...})` → `run(ctx, attempt, next_channel)`
//! - [`load_balancer`]：负载均衡调度器
//!   `LoadBalancer::new(items, LoadBalanceStrategy::Weighted)` → `select()`
//!
//! 业务层（`application/`、`gateway/`）只负责组装输入参数与消费结果，
//! 不重复实现调度逻辑；新增调度器时在 `mod.rs` 声明并补齐文档即可。

pub mod failover;
pub mod load_balancer;
