//! 分层级日志系统
//!
//! 特性：
//! - 模块级日志过滤（不同模块不同级别）
//! - 请求链路追踪（request_id 贯穿）
//! - 分级输出格式（文件详细，控制台精简）

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// 日志系统配置
pub struct LoggingConfig {
    /// 全局日志级别
    pub level: String,
    /// 文件日志级别
    pub file_level: String,
    /// 模块级日志覆盖
    pub module_levels: std::collections::HashMap<String, String>,
    /// 日志文件目录
    pub log_dir: PathBuf,
    /// 日志保留天数
    pub retention_days: i64,
}

impl LoggingConfig {
    /// 从 GatewaySettings 构建日志配置
    pub fn from_settings(
        settings: &crate::models::gateway_settings::GatewaySettings,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            level: settings.log_level.clone(),
            file_level: settings.file_level.clone(),
            module_levels: settings.log_modules.clone(),
            log_dir,
            retention_days: settings.log_retention_days,
        }
    }
}

/// 初始化分层级日志系统
///
/// 返回 _guard 必须保持存活到进程结束（泄漏到全局）
pub fn init_logging(config: LoggingConfig) -> WorkerGuard {
    // 构建 EnvFilter：模块级别 > 全局级别
    let filter = build_env_filter(&config.level, &config.file_level, &config.module_levels);

    // 文件 appender（按天轮转）
    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "silk.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 文件层：详细格式，含源码位置
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true);

    // 控制台层：精简格式，带颜色
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .compact();

    // 组合所有层
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!(
        level = %config.level,
        file_level = %config.file_level,
        modules = ?config.module_levels,
        "分层级日志系统已初始化"
    );

    guard
}

/// 构建 EnvFilter：支持模块级别覆盖
fn build_env_filter(
    global_level: &str,
    _file_level: &str,
    module_levels: &std::collections::HashMap<String, String>,
) -> EnvFilter {
    // 默认使用全局级别
    let mut filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(global_level));

    // 添加模块级覆盖（用户配置优先级最高）
    for (module, level) in module_levels {
        let directive = format!("{module}={level}");
        if let Ok(d) = directive.parse() {
            filter = filter.add_directive(d);
        }
    }

    // 添加默认模块级别（gateway 中间件默认 debug）
    let default_modules = [
        ("silk::gateway::middleware", "debug"),
        ("silk::protocol::prism_wasm", "debug"),
        ("silk::gateway::trace_manager", "debug"),
    ];

    for (module, level) in default_modules {
        // 仅当用户未覆盖时添加
        if !module_levels.contains_key(module) {
            let directive = format!("{module}={level}");
            if let Ok(d) = directive.parse() {
                filter = filter.add_directive(d);
            }
        }
    }

    filter
}

/// 请求链路追踪：为每个请求生成唯一 ID
///
/// 用法：
/// ```rust
/// let request_id = generate_request_id();
/// let span = tracing::info_span!("request", request_id = %request_id);
/// let _guard = span.enter();
/// // 后续所有日志自动携带 request_id
/// ```
pub fn generate_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 创建请求追踪 span
pub fn request_span(request_id: &str, method: &str, path: &str) -> tracing::Span {
    tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %method,
        path = %path,
    )
}

/// 创建流式响应追踪 span
pub fn stream_span(request_id: &str, stream_id: &str) -> tracing::Span {
    tracing::info_span!(
        "stream",
        request_id = %request_id,
        stream_id = %stream_id,
    )
}

/// 创建协议转换追踪 span
pub fn protocol_span(request_id: &str, source: &str, target: &str) -> tracing::Span {
    tracing::debug_span!(
        "protocol",
        request_id = %request_id,
        source = %source,
        target = %target,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_env_filter_default() {
        let modules = std::collections::HashMap::new();
        let filter = build_env_filter("info", "debug", &modules);
        // 不 panic 即为成功
        let _ = filter;
    }

    #[test]
    fn test_build_env_filter_with_modules() {
        let mut modules = std::collections::HashMap::new();
        modules.insert("silk::gateway".to_string(), "warn".to_string());
        let filter = build_env_filter("info", "debug", &modules);
        let _ = filter;
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID 格式
    }
}
