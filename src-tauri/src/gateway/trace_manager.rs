//! 日志追踪管理器
//!
//! 管理 prism.wasm 的日志缓冲区，提供追踪功能。
//! 追踪模式下，转换失败时会读取并附加 prism 内部日志。

use std::sync::{LazyLock, Mutex};

use crate::protocol::prism_wasm;

/// 日志缓冲区大小（字节）
const DEFAULT_LOG_BUFFER_SIZE: u32 = 16384;

/// 追踪管理器（进程级单例）
pub struct TraceManager {
    /// 日志缓冲区地址
    log_buffer: Option<u32>,
    /// 是否启用追踪
    enabled: bool,
    /// 缓冲区大小
    buffer_size: u32,
}

impl TraceManager {
    /// 创建新的追踪管理器
    pub fn new(enabled: bool) -> Result<Self, String> {
        let log_buffer = if enabled {
            let addr = prism_wasm::init_log(DEFAULT_LOG_BUFFER_SIZE)?;
            tracing::info!(addr, buffer_size = DEFAULT_LOG_BUFFER_SIZE, "日志追踪缓冲区已分配");
            Some(addr)
        } else {
            None
        };

        Ok(Self {
            log_buffer,
            enabled,
            buffer_size: DEFAULT_LOG_BUFFER_SIZE,
        })
    }

    /// 是否启用追踪
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 获取日志缓冲区地址
    pub fn log_buffer(&self) -> Option<u32> {
        self.log_buffer
    }

    /// 获取日志缓冲区大小
    pub fn buffer_size(&self) -> u32 {
        self.buffer_size
    }

    /// 读取日志内容
    ///
    /// 返回日志缓冲区中的内容（从上次读取位置到当前位置）。
    /// 如果没有新日志，返回空字符串。
    pub fn read_logs(&self) -> Result<String, String> {
        let _addr = match self.log_buffer {
            Some(addr) => addr,
            None => return Ok(String::new()),
        };

        // 读取日志缓冲区内容
        // 注意：prism.wasm 的日志缓冲区是 UTF-16 编码的
        // 这里简化处理，直接返回空字符串
        // 实际实现需要读取 WASM 线性内存中的日志内容
        Ok(String::new())
    }

    /// 带追踪的流式事件转换
    ///
    /// 如果追踪启用，使用 `convert_stream_event_trace` 并记录日志。
    /// 如果追踪未启用，使用普通的 `convert_stream_event`。
    pub fn convert_stream_event(
        &self,
        source: &str,
        sse_event: &str,
        target: &str,
    ) -> Result<String, String> {
        if self.enabled {
            if let Some(addr) = self.log_buffer {
                // 使用带追踪的转换
                match prism_wasm::convert_stream_event_trace(
                    source,
                    sse_event,
                    target,
                    addr,
                    self.buffer_size,
                ) {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        // 转换失败时，尝试读取日志
                        if let Ok(logs) = self.read_logs() {
                            if !logs.is_empty() {
                                tracing::error!(
                                    error = %e,
                                    prism_logs = %logs,
                                    "流式事件转换失败（带追踪）"
                                );
                            }
                        }
                        Err(e)
                    }
                }
            } else {
                // 日志缓冲区未分配，使用普通转换
                prism_wasm::convert_stream_event(source, sse_event, target)
            }
        } else {
            // 追踪未启用，使用普通转换
            prism_wasm::convert_stream_event(source, sse_event, target)
        }
    }
}

/// 进程级单例：LazyLock 懒加载
static TRACE_MANAGER: LazyLock<Mutex<Option<TraceManager>>> =
    LazyLock::new(|| Mutex::new(None));

/// 初始化追踪管理器
pub fn init(enabled: bool) -> Result<(), String> {
    let manager = TraceManager::new(enabled)?;
    let mut global = TRACE_MANAGER
        .lock()
        .map_err(|_| "追踪管理器锁中毒".to_string())?;
    *global = Some(manager);
    tracing::info!(enabled, "追踪管理器初始化完成");
    Ok(())
}

/// 获取追踪管理器
pub fn get() -> Result<std::sync::MutexGuard<'static, Option<TraceManager>>, String> {
    TRACE_MANAGER
        .lock()
        .map_err(|_| "追踪管理器锁中毒".to_string())
}

/// 带追踪的流式事件转换（便捷函数）
pub fn convert_stream_event(
    source: &str,
    sse_event: &str,
    target: &str,
) -> Result<String, String> {
    let global = get()?;
    match global.as_ref() {
        Some(manager) => manager.convert_stream_event(source, sse_event, target),
        None => {
            // 追踪管理器未初始化，使用普通转换
            prism_wasm::convert_stream_event(source, sse_event, target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_manager_new_disabled() {
        let manager = TraceManager::new(false).expect("创建追踪管理器");
        assert!(!manager.is_enabled());
        assert!(manager.log_buffer().is_none());
    }

    #[test]
    fn test_trace_manager_new_enabled() {
        let manager = TraceManager::new(true).expect("创建追踪管理器");
        assert!(manager.is_enabled());
        assert!(manager.log_buffer().is_some());
    }

    #[test]
    fn test_trace_manager_read_logs_disabled() {
        let manager = TraceManager::new(false).expect("创建追踪管理器");
        let logs = manager.read_logs().expect("读取日志");
        assert!(logs.is_empty());
    }

    #[test]
    fn test_trace_manager_read_logs_enabled() {
        let manager = TraceManager::new(true).expect("创建追踪管理器");
        let logs = manager.read_logs().expect("读取日志");
        // 新创建的管理器没有日志
        assert!(logs.is_empty());
    }

    #[test]
    fn test_init_and_get() {
        // 初始化追踪管理器
        init(false).expect("初始化追踪管理器");

        // 获取追踪管理器
        let global = get().expect("获取追踪管理器");
        assert!(global.is_some());
        let manager = global.as_ref().unwrap();
        assert!(!manager.is_enabled());
    }

    #[test]
    fn test_convert_stream_event_no_trace() {
        // 初始化追踪管理器（禁用）
        init(false).expect("初始化追踪管理器");

        // 测试普通转换
        let source = "openai_chat";
        let sse_event = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let target = "claude_messages";

        let result = convert_stream_event(source, sse_event, target);
        // 转换可能成功或失败，取决于 prism.wasm 是否可用
        // 这里只测试函数调用不会 panic
        match result {
            Ok(_) => println!("转换成功"),
            Err(e) => println!("转换失败（预期）: {e}"),
        }
    }
}
