use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::load_balancer::{LoadBalanceStrategy, LoadBalancer};
use crate::models::{GroupMember, ProviderGroup};
use crate::persistence::GroupRepo;
use sqlx::SqlitePool;

/// 分组管理器：维护所有 ProviderGroup，按策略选择 Provider
pub struct GroupManager {
    inner: Arc<RwLock<GroupManagerInner>>,
}

struct GroupManagerInner {
    groups: HashMap<String, GroupState>,
}

struct GroupState {
    group: ProviderGroup,
    balancer: LoadBalancer<GroupMember>,
}

impl GroupManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(GroupManagerInner {
                groups: HashMap::new(),
            })),
        }
    }

    /// 从数据库加载所有启用的分组
    pub async fn load(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let groups = GroupRepo::find_all_groups(pool).await?;
        let mut inner = self.inner.write().await;

        inner.groups.clear();

        for group in groups {
            if group.enabled == 0 {
                continue;
            }

            let members = GroupRepo::find_members_by_group(pool, &group.id).await?;
            let enabled_members: Vec<_> = members.into_iter().filter(|m| m.enabled != 0).collect();

            if enabled_members.is_empty() {
                continue;
            }

            let strategy = LoadBalanceStrategy::from_str(&group.strategy);

            let state = GroupState {
                group,
                balancer: LoadBalancer::new(enabled_members, strategy),
            };

            inner.groups.insert(state.group.id.clone(), state);
        }

        Ok(())
    }

    /// 根据分组 ID 选择一个 Provider
    pub async fn select_provider(&self, group_id: &str) -> Option<GroupMember> {
        let inner = self.inner.read().await;
        let state = inner.groups.get(group_id)?;
        state.balancer.select().cloned()
    }

    /// 获取分组信息
    pub async fn get_group(&self, group_id: &str) -> Option<ProviderGroup> {
        let inner = self.inner.read().await;
        inner.groups.get(group_id).map(|s| s.group.clone())
    }

    /// 获取所有分组
    pub async fn get_all_groups(&self) -> Vec<ProviderGroup> {
        let inner = self.inner.read().await;
        inner.groups.values().map(|s| s.group.clone()).collect()
    }

    /// 获取分组的成员列表
    pub async fn get_members(&self, group_id: &str) -> Vec<GroupMember> {
        let inner = self.inner.read().await;
        inner
            .groups
            .get(group_id)
            .map(|s| s.balancer.items().to_vec())
            .unwrap_or_default()
    }

    /// 重新加载指定分组
    pub async fn reload_group(&self, pool: &SqlitePool, group_id: &str) -> Result<(), sqlx::Error> {
        let group = match GroupRepo::find_group_by_id(pool, group_id).await? {
            Some(g) => g,
            None => {
                // 分组已删除，移除
                let mut inner = self.inner.write().await;
                inner.groups.remove(group_id);
                return Ok(());
            }
        };

        if group.enabled == 0 {
            let mut inner = self.inner.write().await;
            inner.groups.remove(group_id);
            return Ok(());
        }

        let members = GroupRepo::find_members_by_group(pool, &group.id).await?;
        let enabled_members: Vec<_> = members.into_iter().filter(|m| m.enabled != 0).collect();

        let mut inner = self.inner.write().await;

        if enabled_members.is_empty() {
            inner.groups.remove(group_id);
            return Ok(());
        }

        let strategy = LoadBalanceStrategy::from_str(&group.strategy);

        let state = GroupState {
            group,
            balancer: LoadBalancer::new(enabled_members, strategy),
        };

        inner.groups.insert(group_id.to_string(), state);
        Ok(())
    }

    /// 重新加载所有分组（委托给 load）
    pub async fn reload_all(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        self.load(pool).await
    }
}

impl Default for GroupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_balance_strategy_roundtrip() {
        let strategies = vec![
            LoadBalanceStrategy::RoundRobin,
            LoadBalanceStrategy::Weighted,
            LoadBalanceStrategy::LeastConn,
        ];

        for strategy in &strategies {
            let s = strategy.as_str();
            let parsed = LoadBalanceStrategy::from_str(s);
            assert_eq!(parsed, *strategy);
        }
    }

    #[test]
    fn test_load_balance_strategy_default() {
        assert_eq!(
            LoadBalanceStrategy::from_str("unknown"),
            LoadBalanceStrategy::RoundRobin
        );
    }
}
