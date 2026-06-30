use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
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

/// 获取或创建主密钥文件，存储在用户主目录下
pub fn get_or_create_master_key() -> Result<[u8; 32], CryptoError> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| CryptoError::KeyDerivation("无法获取用户主目录".to_string()))?;

    let key_path = PathBuf::from(home_dir).join(".silk_master.key");
    if key_path.exists() {
        let bytes = fs::read(&key_path)
            .map_err(|e| CryptoError::KeyDerivation(format!("读取主密钥失败: {}", e)))?;
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }

    // 生成新密钥
    let mut key = [0u8; 32];
    let mut rng = rand::thread_rng();
    rand::Rng::fill(&mut rng, &mut key);

    fs::write(&key_path, key)
        .map_err(|e| CryptoError::KeyDerivation(format!("保存主密钥失败: {}", e)))?;

    Ok(key)
}

/// 使用 AES-256-GCM 加密明文
pub fn encrypt(plaintext: &str) -> Result<String, CryptoError> {
    let master_key = get_or_create_master_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&master_key);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::thread_rng();
    rand::Rng::fill(&mut rng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CryptoError::Encrypt(e.to_string()))?;

    let data = EncryptedData {
        nonce: base64::prelude::BASE64_STANDARD.encode(nonce_bytes),
        ciphertext: base64::prelude::BASE64_STANDARD.encode(ciphertext),
    };

    Ok(data.to_storage_string())
}

/// 解密密文，如果是明文直接返回
pub fn decrypt(storage_string: &str) -> Result<String, CryptoError> {
    let data = match EncryptedData::from_storage_string(storage_string) {
        Ok(d) => d,
        Err(_) => return Ok(storage_string.to_string()),
    };

    let master_key = get_or_create_master_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&master_key);
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

/// 哈希 API Key（用于存储和查找，不可逆）
pub fn hash_api_key(api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let result = hasher.finalize();
    base64::prelude::BASE64_STANDARD.encode(result)
}
