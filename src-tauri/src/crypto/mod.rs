use base64::Engine;
use serde::Deserialize;
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

/// 从密码派生 256 位密钥（保留用于 Gateway Key 哈希等场景）
pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 100_000, &mut key);
    Ok(key)
}

// 保留 EncryptedData 类型用于反序列化旧的加密数据（迁移用）
#[derive(Debug, Clone, Deserialize)]
pub struct EncryptedData {
    pub nonce: String,
    pub ciphertext: String,
}

/// 尝试解密旧的 AES-GCM 加密数据（仅用于一次性迁移）
/// 若数据不是加密格式（明文），直接返回原文
pub fn decrypt_legacy(storage_string: &str, _master_key: &[u8; 32]) -> Result<String, CryptoError> {
    // 明文直接返回
    if serde_json::from_str::<EncryptedData>(storage_string).is_err() {
        return Ok(storage_string.to_string());
    }
    // 旧的加密数据：无法解密（主密钥已废弃），返回错误让调用方处理
    Err(CryptoError::InvalidFormat)
}
