use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    /// 旧类型字段，保留用于向后兼容（不再在前端展示）
    pub provider_type: String,
    /// 支持的接口协议列表（JSON 数组），如 ["chat","response","message"]
    pub protocols: String,
    /// 模型列表（JSON 数组），如 ["gpt-4o","gpt-3.5-turbo"]
    pub models: String,
    /// 额外 API Key 列表（JSON 数组），格式 [{"name":"主密钥","enabled":true}]
    /// value 字段在存储时加密，返回时不暴露
    pub keys: String,
    pub api_base_url: String,
    /// 加密的 API Key（AES-GCM 密文，hex 编码存储）
    pub api_key: String,
    /// 已废弃：默认模型名，由 models 替代
    pub model_name: Option<String>,
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
    /// 支持的接口协议，如 ["chat", "response", "message"]
    pub protocols: Vec<String>,
    pub api_base_url: String,
    /// 明文 API Key，存储时由调用方加密
    pub api_key: String,
    /// 模型列表
    pub models: Vec<String>,
    /// 额外 Key 列表（创建时明文，存储时加密）
    pub keys: Vec<ProviderKeyEntry>,
    pub model_name: Option<String>,
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
    /// 明文 API Key，存储时由调用方加密
    pub api_key: Option<String>,
    pub models: Option<Vec<String>>,
    pub keys: Option<Vec<ProviderKeyEntry>>,
    pub model_name: Option<String>,
    pub proxy_url: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub max_retries: Option<i64>,
    pub status: Option<String>,
    pub health_status: Option<String>,
    pub last_health_check_at: Option<chrono::NaiveDateTime>,
    pub metadata_json: Option<String>,
}

/// 渠道的额外 API Key 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyEntry {
    pub name: String,
    /// 明文 Key（加密前），存储时会被加密替换
    pub value: String,
    pub enabled: bool,
}

impl Provider {
    /// 获取解密后的 API Key（需要传入 master_key）
    pub fn decrypted_api_key(
        &self,
        master_key: &[u8; 32],
    ) -> Result<String, crate::crypto::CryptoError> {
        crate::crypto::decrypt_api_key(&self.api_key, master_key)
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
