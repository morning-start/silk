use serde::Serialize;
use sqlx::SqlitePool;

/// 应用服务层的统一错误类型
///
/// 所有 application/ 层函数都返回此类型，commands 层通过 `.map_err(|e| e.to_string())` 转换为 Tauri 兼容的 String。
#[derive(Debug, thiserror::Error, Serialize)]
pub enum ServiceError {
    /// 数据库连接池未初始化
    #[error("数据库未初始化")]
    DbNotInitialized,

    /// 请求的资源不存在
    #[error("{message} 不存在")]
    NotFound {
        message: String,
    },

    /// 参数校验失败等客户端错误
    #[error("{message}")]
    BadRequest {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },

    /// 数据库操作失败
    #[error("数据库错误: {message}")]
    Database {
        message: String,
    },

    /// 内部错误（第三方服务、IO 等）
    #[error("{message}")]
    Internal {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

/// 便捷函数：从 Option 获取数据库连接池，或返回 DbNotInitialized 错误
pub fn require_db() -> Result<&'static SqlitePool, ServiceError> {
    crate::get_db_pool().ok_or(ServiceError::DbNotInitialized)
}

/// 从 Option 提取值，若为 None 返回 NotFound 错误
pub fn require_found<T>(value: Option<T>, name: &str) -> Result<T, ServiceError> {
    value.ok_or_else(|| ServiceError::NotFound {
        message: name.to_string(),
    })
}

impl From<sqlx::Error> for ServiceError {
    fn from(e: sqlx::Error) -> Self {
        ServiceError::Database {
            message: e.to_string(),
        }
    }
}