use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("加密/解密错误: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),

    #[error("网关错误: {0}")]
    Gateway(String),

    #[error("协议转换错误: {0}")]
    Protocol(String),

    #[error("Provider 不可用: {0}")]
    ProviderUnavailable(String),

    #[error("请求超时")]
    Timeout,

    #[error("配置错误: {0}")]
    Config(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("验证错误: {0}")]
    Validation(String),
}

impl From<AppError> for String {
    fn from(err: AppError) -> String {
        err.to_string()
    }
}
