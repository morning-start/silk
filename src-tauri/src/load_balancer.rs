use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 可负载均衡的条目 trait
pub trait LoadBalancedItem {
    fn weight(&self) -> i64;
    fn enabled(&self) -> bool;
}

/// 跨请求共享的负载均衡状态
///
/// LoadBalancer 每次请求都会重建（items 从 DB 读取），但
/// current_weights 需要跨请求持久才能让轮询类策略真正生效。
/// 网关将每个 mapping / provider 对应的状态缓存为 Arc，供各请求复用。
#[derive(Debug, Default)]
pub struct LoadBalancerState {
    /// 平滑加权轮询的当前权重（与启用条目列表对齐，跨请求持久）
    current_weights: Mutex<Vec<i64>>,
}

/// 负载均衡策略
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Weighted,
    LeastConn,
}

impl LoadBalanceStrategy {
    pub fn parse(s: &str) -> Self {
        match s {
            "weighted" => Self::Weighted,
            "least_conn" => Self::LeastConn,
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
        }
    }
}

/// 通用的负载均衡选择器
#[derive(Debug)]
pub struct LoadBalancer<T> {
    items: Vec<T>,
    strategy: LoadBalanceStrategy,
    /// 跨请求共享状态（网关提供时轮询类策略真正生效）
    shared: Option<Arc<LoadBalancerState>>,
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
            shared: None,
            counter: AtomicU64::new(0),
            active_conns,
        }
    }

    /// 携带跨请求共享状态创建（网关请求路径使用，轮询状态持久生效）
    pub fn with_shared_state(
        items: Vec<T>,
        strategy: LoadBalanceStrategy,
        shared: Arc<LoadBalancerState>,
    ) -> Self {
        let n = items.len();
        let active_conns = (0..n).map(|_| AtomicU64::new(0)).collect();
        Self {
            items,
            strategy,
            shared: Some(shared),
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
        // 共享状态的 current_weights 由 select 按需对齐长度，此处不重置
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
            LoadBalanceStrategy::RoundRobin => match &self.shared {
                // 平滑加权轮询：每轮所有条目 current += weight，取最大者，选中后 -= total
                Some(state) => {
                    let mut weights = state.current_weights.lock().unwrap();
                    if weights.len() != enabled.len() {
                        weights.resize(enabled.len(), 0);
                    }
                    let total_weight: i64 = enabled.iter().map(|(_, i)| i.weight().max(1)).sum();
                    for (i, (_, item)) in enabled.iter().enumerate() {
                        weights[i] += item.weight().max(1);
                    }
                    let mut best = 0usize;
                    let mut best_weight = i64::MIN;
                    for (i, w) in weights.iter().enumerate() {
                        if *w > best_weight {
                            best_weight = *w;
                            best = i;
                        }
                    }
                    weights[best] -= total_weight;
                    Some(enabled[best].1)
                }
                // 无共享状态时退化为普通轮询（测试等一次性场景）
                None => {
                    let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                    Some(enabled[(idx as usize) % enabled.len()].1)
                }
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestItem {
        id: &'static str,
        weight: i64,
        enabled: bool,
    }

    impl LoadBalancedItem for TestItem {
        fn weight(&self) -> i64 {
            self.weight
        }
        fn enabled(&self) -> bool {
            self.enabled
        }
    }

    fn items(spec: &[(&'static str, i64)]) -> Vec<TestItem> {
        spec.iter()
            .map(|(id, weight)| TestItem {
                id: *id,
                weight: *weight,
                enabled: true,
            })
            .collect()
    }

    #[test]
    fn smooth_weighted_round_robin_respects_weights() {
        // 权重 3:1:1，平滑加权轮询每 5 次一个完整周期：A B A C A
        let balancer = LoadBalancer::with_shared_state(
            items(&[("A", 3), ("B", 1), ("C", 1)]),
            LoadBalanceStrategy::RoundRobin,
            Arc::new(LoadBalancerState::default()),
        );
        let mut counts = std::collections::HashMap::new();
        for _ in 0..10 {
            let item = balancer.select().unwrap();
            *counts.entry(item.id).or_insert(0) += 1;
        }
        assert_eq!(counts.get("A"), Some(&6));
        assert_eq!(counts.get("B"), Some(&2));
        assert_eq!(counts.get("C"), Some(&2));
    }

    #[test]
    fn weighted_random_prefers_high_weight() {
        // 权重 [1,1,1,100]，高权重 Key 应被绝大多数请求选中（期望 ~97%）
        let balancer = LoadBalancer::new(
            items(&[("A", 1), ("B", 1), ("C", 1), ("D", 100)]),
            LoadBalanceStrategy::Weighted,
        );
        let mut heavy = 0;
        for _ in 0..2000 {
            let item = balancer.select().unwrap();
            if item.id == "D" {
                heavy += 1;
            }
        }
        assert!(heavy > 1800, "高权重 Key 选中次数过低: {heavy}");
    }

    #[test]
    fn least_conn_ignores_weight() {
        // 权重不影响最少连接选择：A 已有活跃连接时选中 B
        let balancer = LoadBalancer::new(
            items(&[("A", 100), ("B", 1)]),
            LoadBalanceStrategy::LeastConn,
        );
        let a_ref = balancer.select().unwrap();
        assert_eq!(a_ref.id, "A");
        balancer.connection_started(a_ref);
        let picked = balancer.select().unwrap();
        assert_eq!(picked.id, "B");
    }

    #[test]
    fn round_robin_without_shared_state_alternates() {
        // 无共享状态退化为普通轮询（忽略权重）
        let balancer = LoadBalancer::new(
            items(&[("A", 5), ("B", 1)]),
            LoadBalanceStrategy::RoundRobin,
        );
        let first = balancer.select().unwrap().id;
        let second = balancer.select().unwrap().id;
        assert_ne!(first, second);
    }

    #[test]
    fn select_returns_none_when_all_disabled() {
        let items = vec![TestItem {
            id: "A",
            weight: 1,
            enabled: false,
        }];
        let balancer = LoadBalancer::new(items, LoadBalanceStrategy::Weighted);
        assert!(balancer.select().is_none());
    }
}
