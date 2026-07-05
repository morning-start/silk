use std::sync::atomic::{AtomicU64, Ordering};

/// 可负载均衡的条目 trait
pub trait LoadBalancedItem {
    fn weight(&self) -> i64;
    fn enabled(&self) -> bool;
}

/// 负载均衡策略
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Weighted,
    LeastConn,
    Failover,
}

impl LoadBalanceStrategy {
    pub fn parse(s: &str) -> Self {
        match s {
            "weighted" => Self::Weighted,
            "least_conn" => Self::LeastConn,
            "failover" => Self::Failover,
            "round_robin" => Self::RoundRobin,
            other => {
                tracing::warn!("未知负载均衡策略 '{other}'，默认使用 RoundRobin");
                Self::RoundRobin
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Weighted => "weighted",
            Self::LeastConn => "least_conn",
            Self::Failover => "failover",
        }
    }
}

/// 通用的负载均衡选择器
#[derive(Debug)]
pub struct LoadBalancer<T> {
    items: Vec<T>,
    strategy: LoadBalanceStrategy,
    counter: AtomicU64,
    /// 每个条目的活跃连接数（仅 LeastConn 策略使用）
    active_conns: Vec<AtomicU64>,
}

impl<T: LoadBalancedItem + Clone> LoadBalancer<T> {
    pub fn new(items: Vec<T>, strategy: LoadBalanceStrategy) -> Self {
        let n = items.len();
        let active_conns = (0..n).map(|_| AtomicU64::new(0)).collect();
        Self {
            items,
            strategy,
            counter: AtomicU64::new(0),
            active_conns,
        }
    }

    pub fn reload(&mut self, items: Vec<T>, strategy: LoadBalanceStrategy) {
        let n = items.len();
        self.items = items;
        self.strategy = strategy;
        self.counter.store(0, Ordering::Relaxed);
        self.active_conns = (0..n).map(|_| AtomicU64::new(0)).collect();
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// 按策略选择一个条目（只考虑启用的条目）
    pub fn select(&self) -> Option<&T> {
        let enabled: Vec<(usize, &T)> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.enabled())
            .collect();
        if enabled.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                Some(enabled[(idx as usize) % enabled.len()].1)
            }
            LoadBalanceStrategy::Weighted => {
                let total_weight: i64 = enabled.iter().map(|(_, i)| i.weight().max(1)).sum();
                let rng = rand::random::<u64>();
                let mut cumulative = 0i64;
                let target = (rng as i64).abs() % total_weight;
                for (_, item) in &enabled {
                    cumulative += item.weight().max(1);
                    if cumulative > target {
                        return Some(item);
                    }
                }
                enabled.last().map(|(_, item)| *item)
            }
            LoadBalanceStrategy::LeastConn => {
                // 选择活跃连接数最少的条目
                let mut best_idx = 0;
                let mut best_conns = u64::MAX;
                for (orig_idx, _) in &enabled {
                    let conns = self.active_conns[*orig_idx].load(Ordering::Relaxed);
                    if conns < best_conns {
                        best_conns = conns;
                        best_idx = *orig_idx;
                    }
                }
                Some(&self.items[best_idx])
            }
            LoadBalanceStrategy::Failover => enabled.first().map(|(_, item)| *item),
        }
    }

    /// 记录连接开始（LeastConn 策略使用）
    pub fn connection_started(&self, item: &T) {
        if self.strategy == LoadBalanceStrategy::LeastConn {
            if let Some(idx) = self.items.iter().position(|i| std::ptr::eq(i, item)) {
                self.active_conns[idx].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 记录连接结束（LeastConn 策略使用）
    pub fn connection_finished(&self, item: &T) {
        if self.strategy == LoadBalanceStrategy::LeastConn {
            if let Some(idx) = self.items.iter().position(|i| std::ptr::eq(i, item)) {
                self.active_conns[idx].fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}
