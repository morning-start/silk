use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::load_balancer::LoadBalancedItem;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProviderGroup {
    pub id: String,
    pub name: String,
    pub model_name: String,
    pub strategy: String,
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: String,
    pub group_id: String,
    pub provider_id: String,
    pub weight: i64,
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
}

impl LoadBalancedItem for GroupMember {
    fn weight(&self) -> i64 {
        self.weight
    }
    fn enabled(&self) -> bool {
        self.enabled != 0
    }
}

/// 负载均衡策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Weighted,
    LeastConn,
}

impl LoadBalanceStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoadBalanceStrategy::RoundRobin => "round_robin",
            LoadBalanceStrategy::Weighted => "weighted",
            LoadBalanceStrategy::LeastConn => "least_conn",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "round_robin" => Some(LoadBalanceStrategy::RoundRobin),
            "weighted" => Some(LoadBalanceStrategy::Weighted),
            "least_conn" => Some(LoadBalanceStrategy::LeastConn),
            _ => None,
        }
    }
}

impl std::fmt::Display for LoadBalanceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 创建 ProviderGroup 的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewProviderGroup {
    pub name: String,
    pub model_name: String,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

/// 更新 ProviderGroup 的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProviderGroup {
    pub name: Option<String>,
    pub model_name: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

/// 创建 GroupMember 的输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGroupMember {
    pub group_id: String,
    pub provider_id: String,
    pub weight: Option<i64>,
}

/// 更新 GroupMember 的输入
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateGroupMember {
    pub weight: Option<i64>,
    pub enabled: Option<bool>,
}

/// 分组及其成员（用于 API 返回）
#[derive(Debug, Clone, Serialize)]
pub struct ProviderGroupWithMembers {
    pub id: String,
    pub name: String,
    pub model_name: String,
    pub strategy: String,
    pub enabled: bool,
    pub members: Vec<GroupMemberInfo>,
    pub created_at: String,
    pub updated_at: String,
}

/// 分组成员信息（含 Provider 详情）
#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberInfo {
    pub id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub weight: i64,
    pub enabled: bool,
}
