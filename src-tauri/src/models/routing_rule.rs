use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: String,
    pub name: String,
    pub match_host: Option<String>,
    pub match_path: String,
    pub match_method: String,
    pub match_content_type: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub target_provider_id: String,
    pub target_group_id: Option<String>,
    pub failover_provider_id: Option<String>,
    pub protocol_conversion: i64,
    pub model_name_override: Option<String>,
    pub metadata_json: Option<String>,
    pub priority: i64,
    pub enabled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 用于创建路由规则的输入结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewRoutingRule {
    pub name: String,
    pub match_host: Option<String>,
    pub match_path: String,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub target_provider_id: String,
    pub target_group_id: Option<String>,
    pub failover_provider_id: Option<String>,
    pub protocol_conversion: Option<bool>,
    pub model_name_override: Option<String>,
    pub metadata_json: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

/// 用于更新路由规则的输入结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRoutingRule {
    pub name: Option<String>,
    pub match_host: Option<String>,
    pub match_path: Option<String>,
    pub match_method: Option<String>,
    pub match_content_type: Option<String>,
    pub inbound_protocol: Option<String>,
    pub outbound_protocol: Option<String>,
    pub target_provider_id: Option<String>,
    pub target_group_id: Option<String>,
    pub failover_provider_id: Option<String>,
    pub protocol_conversion: Option<bool>,
    pub model_name_override: Option<String>,
    pub metadata_json: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

impl RoutingRule {
    /// 判断主机名是否匹配此规则
    pub fn matches_host(&self, host: Option<&str>) -> bool {
        match self.match_host.as_deref() {
            Some(expected) if !expected.is_empty() => {
                host.map(|actual| actual == expected).unwrap_or(false)
            }
            _ => true,
        }
    }

    /// 判断请求是否匹配此规则
    pub fn matches(
        &self,
        host: Option<&str>,
        method: &str,
        path: &str,
        content_type: Option<&str>,
    ) -> bool {
        if self.enabled == 0 {
            return false;
        }

        if !self.matches_host(host) {
            return false;
        }

        // 方法匹配
        if self.match_method != "*" && self.match_method != method {
            return false;
        }

        // 路径匹配（支持 * 通配符）
        if self.match_path.ends_with('*') {
            let prefix = &self.match_path[..self.match_path.len() - 1];
            if !path.starts_with(prefix) {
                return false;
            }
        } else if self.match_path != path {
            return false;
        }

        // Content-Type 匹配（如果规则指定了的话）
        if let Some(ref ct) = self.match_content_type {
            if let Some(req_ct) = content_type {
                if !req_ct.contains(ct) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}
