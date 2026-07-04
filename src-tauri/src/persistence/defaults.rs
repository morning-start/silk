//! 数据库层面的默认常量，集中管理 magic numbers

/// 网关默认端口
pub const DEFAULT_BIND_PORT: i64 = 9876;

/// 默认日志保留天数
pub const DEFAULT_LOG_RETENTION_DAYS: i64 = 30;

/// 默认速率限制：每分钟最大请求数
pub const DEFAULT_RATE_LIMIT_MAX_REQUESTS: i64 = 1000;

/// 默认速率限制：每分钟最大 Token 数
pub const DEFAULT_RATE_LIMIT_MAX_TOKENS: i64 = 500000;

/// 分页查询最大返回条数
pub const PAGINATION_MAX_LIMIT: i64 = 1000;

/// Provider 默认超时秒数
pub const DEFAULT_PROVIDER_TIMEOUT_SECONDS: i64 = 30;

/// Provider 默认最大重试次数
pub const DEFAULT_PROVIDER_MAX_RETRIES: i64 = 3;

/// Gateway Key 默认最大并发连接数
pub const DEFAULT_KEY_MAX_CONCURRENT: i64 = 10;

/// 路由规则默认优先级
pub const DEFAULT_ROUTING_PRIORITY: i64 = 100;
