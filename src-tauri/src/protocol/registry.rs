use std::collections::HashMap;
use std::sync::Arc;

use crate::protocol::adapter::ProviderAdapter;
use crate::protocol::ProtocolError;

/// 适配器注册表：根据协议类型返回对应适配器
pub struct AdapterRegistry {
    adapters: HashMap<&'static str, Arc<dyn ProviderAdapter>>,
}

impl std::fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("adapters", &self.adapters.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl AdapterRegistry {
    /// 创建空注册表（不注册任何适配器）
    pub fn new_empty() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// 创建注册表并注册所有内置适配器
    pub fn new() -> Self {
        let mut registry = Self::new_empty();
        crate::protocol::builtin_adapters::register_all(&mut registry);
        registry
    }

    /// 注册自定义适配器
    pub fn register(&mut self, adapter: Arc<dyn ProviderAdapter>) {
        self.adapters.insert(adapter.provider_type(), adapter);
    }

    /// 根据协议类型获取适配器
    pub fn get(&self, protocol: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters.get(protocol).cloned()
    }

    /// 根据协议类型获取适配器，找不到时返回错误
    pub fn get_or_error(&self, protocol: &str) -> Result<Arc<dyn ProviderAdapter>, ProtocolError> {
        self.get(protocol).ok_or_else(|| {
            ProtocolError::UnsupportedFormat(format!(
                "不支持的协议类型: {}。支持的类型: {}",
                protocol,
                self.supported_types().join(", ")
            ))
        })
    }

    /// 返回所有支持的协议类型
    pub fn supported_types(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_all_adapters() {
        let registry = AdapterRegistry::new();
        assert!(registry.get("openai_chat").is_some());
        assert!(registry.get("claude_messages").is_some());
        assert!(registry.get("openai_response").is_some());
    }

    #[test]
    fn test_get_unknown_protocol_returns_none() {
        let registry = AdapterRegistry::new();
        assert!(registry.get("unknown_protocol").is_none());
    }

    #[test]
    fn test_get_or_error_returns_error_for_unknown() {
        let registry = AdapterRegistry::new();
        let result = registry.get_or_error("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_types() {
        let registry = AdapterRegistry::new();
        let types = registry.supported_types();
        assert!(types.contains(&"openai_chat"));
        assert!(types.contains(&"claude_messages"));
        assert!(types.contains(&"openai_response"));
    }
}
