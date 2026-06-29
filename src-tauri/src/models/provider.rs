use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::load_balancer::{LoadBalanceStrategy, LoadBalancedItem, LoadBalancer};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
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
    fn weight(&self) -> i64 {
        self.weight.max(1)
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

fn default_weight() -> i64 {
    1
}

impl Provider {
    /// 解析 keys JSON 字段为 ProviderKeyEntry 列表
    pub fn keys_vec(&self) -> Vec<ProviderKeyEntry> {
        serde_json::from_str(&self.keys).unwrap_or_default()
    }

    /// 按负载均衡策略选择一个 API Key。
    pub fn select_api_key(&self) -> Result<String, crate::crypto::CryptoError> {
        let entries = self.keys_vec();
        let strategy = LoadBalanceStrategy::from_str(&self.key_strategy);
        let balancer = LoadBalancer::new(entries, strategy);
        let selected = balancer
            .select()
            .ok_or(crate::crypto::CryptoError::InvalidFormat)?;

        if selected.enabled && !selected.value.is_empty() {
            // 明文存储，直接返回（旧加密数据视为无效，需用户重新填写）
            if selected.value.starts_with('{') && selected.value.contains("\"ciphertext\"") {
                return Err(crate::crypto::CryptoError::InvalidFormat);
            }
            Ok(selected.value.clone())
        } else {
            Err(crate::crypto::CryptoError::InvalidFormat)
        }
    }

    /// 按负载均衡策略选择一个 API Key（明文存储，直接返回）
    pub fn decrypted_api_key(
        &self,
        _master_key: &[u8; 32],
    ) -> Result<String, crate::crypto::CryptoError> {
        self.select_api_key()
    }

    /// 获取超时时间（秒）
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds as u64)
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
            trimmed[..trimmed.len() - 3]
                .trim_end_matches('/')
                .to_string()
        } else {
            trimmed.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(keys: &str, strategy: &str) -> Provider {
        Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            protocols: r#"["chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: keys.to_string(),
            key_strategy: strategy.to_string(),
            api_base_url: "https://api.openai.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 3,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }

    #[test]
    fn select_api_key_fails_when_all_keys_disabled() {
        let provider = make_provider(
            r#"[{"name":"主密钥","value":"k1","enabled":false,"weight":1}]"#,
            "round_robin",
        );

        let err = provider
            .select_api_key()
            .expect_err("all disabled keys should fail selection");

        assert!(matches!(err, crate::crypto::CryptoError::InvalidFormat));
    }
}
