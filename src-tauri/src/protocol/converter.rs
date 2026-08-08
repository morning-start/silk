//! 协议转换抽象层
//!
//! 定义协议转换器的核心接口，支持流式和非流式转换。
//! 所有转换器必须是无状态的，仅处理字节流，不承担网络通信职责。

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// 协议转换错误
#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("解析失败: {0}")]
    ParseError(String),

    #[error("转换失败: {0}")]
    TransformError(String),

    #[error("序列化失败: {0}")]
    SerializationError(String),

    #[error("不支持的协议: {0}")]
    UnsupportedProtocol(String),

    #[error("内部错误: {0}")]
    InternalError(String),
}

/// 协议转换器trait
///
/// 所有协议转换器必须实现此接口。
/// 转换器是无状态的，每次转换都是独立的。
#[async_trait]
pub trait ProtocolConverter: Send + Sync {
    /// 转换器名称
    fn name(&self) -> &str;

    /// 支持的源协议
    fn source_protocols(&self) -> Vec<&str>;

    /// 支持的目标协议
    fn target_protocols(&self) -> Vec<&str>;

    /// 检查是否支持指定的协议转换
    fn supports(&self, from: &str, to: &str) -> bool {
        self.source_protocols().contains(&from) && self.target_protocols().contains(&to)
    }

    /// 转换请求体
    ///
    /// # Arguments
    /// * `body` - 原始请求体（JSON字节流）
    /// * `from_protocol` - 源协议名称
    /// * `to_protocol` - 目标协议名称
    ///
    /// # Returns
    /// 转换后的请求体（JSON字节流）
    async fn convert_request(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError>;

    /// 转换响应体
    ///
    /// # Arguments
    /// * `body` - 原始响应体（JSON字节流）
    /// * `from_protocol` - 源协议名称
    /// * `to_protocol` - 目标协议名称
    ///
    /// # Returns
    /// 转换后的响应体（JSON字节流）
    async fn convert_response(
        &self,
        body: &[u8],
        from_protocol: &str,
        to_protocol: &str,
    ) -> Result<Bytes, ConversionError>;
}

/// 协议转换器注册表
///
/// 管理所有已注册的协议转换器，支持按名称查找和按协议匹配。
pub struct ConverterRegistry {
    converters: HashMap<String, Arc<dyn ProtocolConverter>>,
}

impl ConverterRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            converters: HashMap::new(),
        }
    }

    /// 注册转换器
    pub fn register(&mut self, converter: Arc<dyn ProtocolConverter>) {
        let name = converter.name().to_string();
        self.converters.insert(name, converter);
    }

    /// 根据名称获取转换器
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ProtocolConverter>> {
        self.converters.get(name)
    }

    /// 查找支持指定协议转换的转换器
    pub fn find_converter(&self, from: &str, to: &str) -> Option<&Arc<dyn ProtocolConverter>> {
        self.converters.values().find(|c| c.supports(from, to))
    }

    /// 获取所有转换器名称
    pub fn list_converters(&self) -> Vec<&str> {
        self.converters.keys().map(|s| s.as_str()).collect()
    }

    /// 获取转换器数量
    pub fn len(&self) -> usize {
        self.converters.len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.converters.is_empty()
    }
}

impl Default for ConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConverter {
        name: String,
    }

    impl MockConverter {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl ProtocolConverter for MockConverter {
        fn name(&self) -> &str {
            &self.name
        }

        fn source_protocols(&self) -> Vec<&str> {
            vec!["openai_chat"]
        }

        fn target_protocols(&self) -> Vec<&str> {
            vec!["claude_messages"]
        }

        async fn convert_request(
            &self,
            body: &[u8],
            _from: &str,
            _to: &str,
        ) -> Result<Bytes, ConversionError> {
            Ok(Bytes::from(body.to_vec()))
        }

        async fn convert_response(
            &self,
            body: &[u8],
            _from: &str,
            _to: &str,
        ) -> Result<Bytes, ConversionError> {
            Ok(Bytes::from(body.to_vec()))
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = ConverterRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ConverterRegistry::new();
        let converter = Arc::new(MockConverter::new("test"));
        registry.register(converter);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_get() {
        let mut registry = ConverterRegistry::new();
        let converter = Arc::new(MockConverter::new("test"));
        registry.register(converter);

        let found = registry.get("test");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "test");

        let not_found = registry.get("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_registry_find_converter() {
        let mut registry = ConverterRegistry::new();
        let converter = Arc::new(MockConverter::new("test"));
        registry.register(converter);

        let found = registry.find_converter("openai_chat", "claude_messages");
        assert!(found.is_some());

        let not_found = registry.find_converter("unsupported", "openai_chat");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_registry_list_converters() {
        let mut registry = ConverterRegistry::new();
        registry.register(Arc::new(MockConverter::new("converter1")));
        registry.register(Arc::new(MockConverter::new("converter2")));

        let list = registry.list_converters();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"converter1"));
        assert!(list.contains(&"converter2"));
    }

    #[test]
    fn test_converter_supports() {
        let converter = MockConverter::new("test");
        assert!(converter.supports("openai_chat", "claude_messages"));
        assert!(!converter.supports("unsupported", "openai_chat"));
        assert!(!converter.supports("openai_chat", "unsupported"));
    }
}
