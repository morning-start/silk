use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("加密失败: {0}")]
    Encrypt(String),
    #[error("解密失败: {0}")]
    Decrypt(String),
    #[error("密钥派生失败: {0}")]
    KeyDerivation(String),
    #[error("无效的密文格式")]
    InvalidFormat,
}

/// 加密后的数据结构（存储在数据库中）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Base64 编码的 nonce
    pub nonce: String,
    /// Base64 编码的密文
    pub ciphertext: String,
}

impl EncryptedData {
    /// 编码为单个字符串存储（JSON 格式）
    fn to_storage_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// 从存储字符串解码
    fn from_storage_string(s: &str) -> Result<Self, CryptoError> {
        serde_json::from_str(s).map_err(|_| CryptoError::InvalidFormat)
    }
}

/// 使用 AES-256-GCM 加密明文
pub fn encrypt(plaintext: &str, master_key: &[u8; 32]) -> Result<String, CryptoError> {
    let key = Key::<Aes256Gcm>::from_slice(master_key);
    let cipher = Aes256Gcm::new(key);

    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CryptoError::Encrypt(e.to_string()))?;

    let data = EncryptedData {
        nonce: base64::prelude::BASE64_STANDARD.encode(nonce),
        ciphertext: base64::prelude::BASE64_STANDARD.encode(ciphertext),
    };

    Ok(data.to_storage_string())
}

/// 解密密文
pub fn decrypt(storage_string: &str, master_key: &[u8; 32]) -> Result<String, CryptoError> {
    let data = EncryptedData::from_storage_string(storage_string)?;

    let key = Key::<Aes256Gcm>::from_slice(master_key);
    let cipher = Aes256Gcm::new(key);

    let nonce_bytes = base64::prelude::BASE64_STANDARD
        .decode(&data.nonce)
        .map_err(|e| CryptoError::Decrypt(format!("nonce decode error: {}", e)))?;
    let ciphertext_bytes = base64::prelude::BASE64_STANDARD
        .decode(&data.ciphertext)
        .map_err(|e| CryptoError::Decrypt(format!("ciphertext decode error: {}", e)))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext_bytes.as_ref())
        .map_err(|e| CryptoError::Decrypt(e.to_string()))?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::Decrypt("无效的 UTF-8".to_string()))
}

/// 专门用于加密 API Key 的便捷函数
pub fn encrypt_api_key(api_key: &str, master_key: &[u8; 32]) -> Result<String, CryptoError> {
    encrypt(api_key, master_key)
}

/// 专门用于解密 API Key 的便捷函数
pub fn decrypt_api_key(encrypted: &str, master_key: &[u8; 32]) -> Result<String, CryptoError> {
    decrypt(encrypted, master_key)
}

/// 生成随机 nonce（12 字节，AES-GCM 标准）
fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    let mut rng = rand::thread_rng();
    rand::Rng::fill(&mut rng, &mut nonce);
    nonce
}

/// 从密码派生 256 位密钥（使用 PBKDF2）
pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 100_000, &mut key);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key: [u8; 32] = rand::random();
        let plaintext = "sk-test-key-12345";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_keys_fail() {
        let key1: [u8; 32] = rand::random();
        let key2: [u8; 32] = rand::random();
        let plaintext = "sk-test-key-12345";

        let encrypted = encrypt(plaintext, &key1).unwrap();
        assert!(decrypt(&encrypted, &key2).is_err());
    }
}
