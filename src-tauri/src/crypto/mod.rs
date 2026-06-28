use base64::Engine;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("密钥派生失败: {0}")]
    KeyDerivation(String),
    #[error("无效的密文格式")]
    InvalidFormat,
}

/// 哈希 API Key（用于存储和查找，不可逆）
pub fn hash_api_key(api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let result = hasher.finalize();
    base64::prelude::BASE64_STANDARD.encode(result)
}
