use std::sync::Arc;

use crate::protocol::adapters::claude::ClaudeMessagesAdapter;
use crate::protocol::adapters::openai_chat::OpenAIChatAdapter;
use crate::protocol::adapters::openai_response::OpenAIResponseAdapter;
use crate::protocol::AdapterRegistry;

/// 注册所有内置适配器到注册表
///
/// 新增适配器时：
/// 1. 在 `protocol/adapters/` 下创建模块
/// 2. 在 `adapters/mod.rs` 中添加 `pub mod my_adapter;`
/// 3. 在本文件添加一行 `registry.register(Arc::new(MyAdapter));`
///
/// 无需修改 `registry.rs` 中的注册逻辑。
pub fn register_all(registry: &mut AdapterRegistry) {
    registry.register(Arc::new(OpenAIChatAdapter));
    registry.register(Arc::new(ClaudeMessagesAdapter));
    registry.register(Arc::new(OpenAIResponseAdapter));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_all_registers_known_adapters() {
        let mut registry = AdapterRegistry::new_empty();
        register_all(&mut registry);
        assert!(registry.get("openai_chat").is_some());
        assert!(registry.get("claude_messages").is_some());
        assert!(registry.get("openai_response").is_some());
        assert_eq!(registry.supported_types().len(), 3);
    }
}
