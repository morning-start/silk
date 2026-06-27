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
    pub fn from_str(s: &str) -> Self {
        match s {
            "weighted" => Self::Weighted,
            "least_conn" => Self::LeastConn,
            "failover" => Self::Failover,
            _ => Self::RoundRobin,
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
}

impl<T: LoadBalancedItem + Clone> LoadBalancer<T> {
    pub fn new(items: Vec<T>, strategy: LoadBalanceStrategy) -> Self {
        Self {
            items,
            strategy,
            counter: AtomicU64::new(0),
        }
    }

    pub fn reload(&mut self, items: Vec<T>, strategy: LoadBalanceStrategy) {
        self.items = items;
        self.strategy = strategy;
        self.counter.store(0, Ordering::Relaxed);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> Vec<T> {
        self.items.clone()
    }

    /// 按策略选择一个条目（只考虑启用的条目）
    pub fn select(&self) -> Option<&T> {
        let enabled: Vec<&T> = self.items.iter().filter(|i| i.enabled()).collect();
        if enabled.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                Some(enabled[(idx as usize) % enabled.len()])
            }
            LoadBalanceStrategy::Weighted => {
                let total_weight: i64 = enabled.iter().map(|i| i.weight().max(1)).sum();
                if total_weight == 0 {
                    return Some(enabled[0]);
                }
                let rng = rand::random::<u64>();
                let mut cumulative = 0i64;
                let target = (rng as i64).abs() % total_weight;
                for item in &enabled {
                    cumulative += item.weight().max(1);
                    if cumulative > target {
                        return Some(item);
                    }
                }
                enabled.last().copied()
            }
            LoadBalanceStrategy::LeastConn => {
                // 最少连接退化到 round_robin（需要连接追踪）
                let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                Some(enabled[(idx as usize) % enabled.len()])
            }
            LoadBalanceStrategy::Failover => enabled.first().copied(),
        }
    }
}
