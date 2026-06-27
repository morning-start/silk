use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::load_balancer::{LoadBalanceStrategy, LoadBalancedItem, LoadBalancer};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    /// 支持的接口协议列表（JSON 数组），如 ["chat","response","message"]
    pub protocols: String,
    /// 模型列表（JSON 数组），如 ["gpt-4o","gpt-3.5-turbo"]
    pub models: String,
    /// API Key 列表（JSON 数组），格式 [{"name":"主密钥","value":"<encrypted>","enabled":true}]
    pub keys: String,
    /// 密钥选择策略: round_robin / weighted / failover
    pub key_strategy: String,
    pub api_base_url: String,
    pub proxy_url: Option<String>,
    pub timeout_seconds: i64,
    pub max_retries: i64,
    pub status: String,
    pub health_status: Option<String>,
    pub last_health_check_at: Option<chrono::NaiveDateTime>,
    pub metadata_json: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// 用于创建 Provider 的输入结构（不含 id 和 timestamps）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProvider {
    pub name: String,
    pub protocols: Vec<String>,
    pub api_base_url: String,
    /// 模型列表
    pub models: Vec<String>,
    /// API Key 列表（创建时明文，存储时加密）
    pub keys: Vec<ProviderKeyEntry>,
    /// 密钥选择策略
    pub key_strategy: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub health_status: Option<String>,
    pub last_health_check_at: Option<chrono::NaiveDateTime>,
    pub metadata_json: Option<String>,
}

/// 用于更新 Provider 的输入结构（所有字段可选）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProvider {
    pub name: Option<String>,
    pub protocols: Option<Vec<String>>,
    pub api_base_url: Option<String>,
    pub models: Option<Vec<String>>,
    pub keys: Option<Vec<ProviderKeyEntry>>,
    pub key_strategy: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub health_status: Option<String>,
    pub last_health_check_at: Option<chrono::NaiveDateTime>,
    pub metadata_json: Option<String>,
}

/// 渠道的 API Key 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyEntry {
    pub name: String,
    /// 明文 Key（加密前），存储时会被加密替换
    pub value: String,
    pub enabled: bool,
    #[serde(default = "default_weight")]
    pub weight: i64,
}

impl LoadBalancedItem for ProviderKeyEntry {
    fn weight(&self) -> i64 { self.weight.max(1) }
    fn enabled(&self) -> bool { self.enabled }
}

fn default_weight() -> i64 { 1 }

impl Provider {
    /// 按负载均衡策略选择一个 API Key 并解密
    pub fn decrypted_api_key(
        &self,
        master_key: &[u8; 32],
    ) -> Result<String, crate::crypto::CryptoError> {
        let entries: Vec<ProviderKeyEntry> = serde_json::from_str(&self.keys).unwrap_or_default();
        let strategy = LoadBalanceStrategy::from_str(&self.key_strategy);
        let balancer = LoadBalancer::new(entries, strategy);
        let selected = balancer.select().ok_or(crate::crypto::CryptoError::InvalidFormat)?;

        if selected.enabled && !selected.value.is_empty() {
            crate::crypto::decrypt_api_key(&selected.value, master_key)
        } else {
            Err(crate::crypto::CryptoError::InvalidFormat)
        }
    }

    /// 获取超时时间（秒）
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds as u64)
    }

    /// 是否启用
    pub fn is_enabled(&self) -> bool {
        self.status == "enabled"
    }

    /// 是否处于健康状态
    pub fn is_healthy(&self) -> bool {
        self.health_status.as_deref() == Some("healthy")
    }

    /// 归一化的健康状态标签
    pub fn health_status_label(&self) -> &str {
        self.health_status.as_deref().unwrap_or("unknown")
    }

    /// 解析 protocols JSON 字段为 Vec<String>
    pub fn protocols_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.protocols).unwrap_or_default()
    }

    /// 解析 models JSON 字段为 Vec<String>
    pub fn models_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.models).unwrap_or_default()
    }

    /// 规范化 API Base URL：去除尾部 /v1 或 /v1/
    pub fn normalize_api_base_url(url: &str) -> String {
        let trimmed = url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            trimmed[..trimmed.len() - 3].trim_end_matches('/').to_string()
        } else {
            trimmed.to_string()
        }
    }
}
