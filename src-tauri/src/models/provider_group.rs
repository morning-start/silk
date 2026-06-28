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
