use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use rand::Rng;

use crate::crypto::hash_api_key;
use crate::error::ServiceError;

/// api-key 文件路径（启动时初始化）
static API_KEY_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 内存缓存的 key 明文，支持刷新
static API_KEY_CACHE: OnceLock<RwLock<String>> = OnceLock::new();

/// 获取 api-key 文件路径
fn get_api_key_path(data_dir: &Path) -> PathBuf {
    data_dir.join("api-key")
}

/// 生成 `sk-silk-{32位随机hex}` 格式的 key
fn generate_key() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    let hex_str: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sk-silk-{}", hex_str)
}

/// 确保 api-key 文件存在（启动时调用）
pub fn ensure_api_key(data_dir: &Path) -> Result<(), ServiceError> {
    let path = get_api_key_path(data_dir);
    API_KEY_PATH.set(path.clone()).map_err(|_| ServiceError::Internal {
        message: "API Key 路径已初始化".to_string(),
        detail: None,
    })?;

    if !path.exists() {
        let key = generate_key();
        std::fs::write(&path, &key).map_err(|e| ServiceError::Internal {
            message: "写入 api-key 文件失败".to_string(),
            detail: Some(e.to_string()),
        })?;
        tracing::info!("已生成 API Key 文件: {}", &path.display());
    }

    // 加载到内存缓存
    let key = std::fs::read_to_string(&path)
        .map_err(|e| ServiceError::Internal {
            message: "读取 api-key 文件失败".to_string(),
            detail: Some(e.to_string()),
        })?
        .trim()
        .to_string();

    let _ = API_KEY_CACHE.set(RwLock::new(key));

    Ok(())
}

/// 获取缓存的 API Key 明文
pub fn get_api_key() -> String {
    API_KEY_CACHE
        .get()
        .map(|lock| lock.read().unwrap().clone())
        .unwrap_or_default()
}

/// 强制刷新（重新生成）API Key
pub fn reset_api_key() -> Result<String, ServiceError> {
    let path = API_KEY_PATH.get().ok_or_else(|| ServiceError::Internal {
        message: "API Key 路径未初始化".to_string(),
        detail: None,
    })?;

    let new_key = generate_key();
    std::fs::write(path, &new_key).map_err(|e| ServiceError::Internal {
        message: "写入 api-key 文件失败".to_string(),
        detail: Some(e.to_string()),
    })?;

    // 更新内存缓存
    if let Some(cache) = API_KEY_CACHE.get() {
        let mut guard = cache.write().unwrap();
        *guard = new_key.clone();
    }

    tracing::info!("API Key 已刷新: {}", path.display());
    Ok(new_key)
}

/// 获取已缓存的 API Key 的 SHA-256 哈希（供网关认证比对）
pub fn get_api_key_hash() -> String {
    let key = get_api_key();
    hash_api_key(&key)
}