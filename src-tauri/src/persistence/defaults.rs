//! 数据库层面的默认常量，集中管理 magic numbers

/// 生成新记录的标准 ID（UUID）和创建时间戳
pub fn new_id_and_now() -> (String, chrono::NaiveDateTime) {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().naive_utc();
    (id, now)
}

/// 将 Option<bool> 转为 SQLite 的 i64（0/1）
pub fn bool_to_i64(val: Option<bool>, default: bool) -> i64 {
    if val.unwrap_or(default) { 1 } else { 0 }
}

/// 将可序列化值转为 JSON 字符串，失败时返回默认空值
pub fn to_json<T: serde::Serialize>(val: &T) -> String {
    serde_json::to_string(val).unwrap_or_default()
}

/// 将 Option<可序列化值> 转为 Option<JSON 字符串>，失败时返回 "[]"
pub fn to_json_opt<T: serde::Serialize>(val: Option<&T>) -> Option<String> {
    val.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()))
}

/// 网关默认端口
pub const DEFAULT_BIND_PORT: i64 = 1877;

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
