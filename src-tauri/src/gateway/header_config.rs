use std::collections::HashSet;
use serde::{Deserialize, Serialize};

/// Header 转发配置
///
/// 定义哪些 header 应该被转发到上游 API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderConfig {
    /// 需要转发的 header 名称（小写）
    pub forward_headers: HashSet<String>,
    /// 不应该转发的 header 名称（小写）
    pub exclude_headers: HashSet<String>,
}

impl Default for HeaderConfig {
    fn default() -> Self {
        let mut forward_headers = HashSet::new();
        // 通用 header
        forward_headers.insert("user-agent".to_string());
        forward_headers.insert("accept".to_string());
        forward_headers.insert("accept-encoding".to_string());
        forward_headers.insert("accept-language".to_string());
        forward_headers.insert("content-type".to_string());
        
        // 请求追踪 header
        forward_headers.insert("x-request-id".to_string());
        forward_headers.insert("x-trace-id".to_string());
        forward_headers.insert("x-correlation-id".to_string());
        
        // AI 工具特定 header
        forward_headers.insert("x-cursor-client-id".to_string());
        forward_headers.insert("x-cursor-trace-id".to_string());
        forward_headers.insert("x-windsurf-client-id".to_string());
        forward_headers.insert("x-windsurf-version".to_string());
        
        let mut exclude_headers = HashSet::new();
        // 不应该转发的 header
        exclude_headers.insert("authorization".to_string());
        exclude_headers.insert("x-api-key".to_string());
        exclude_headers.insert("anthropic-version".to_string());
        exclude_headers.insert("host".to_string());
        exclude_headers.insert("connection".to_string());
        exclude_headers.insert("content-length".to_string());
        exclude_headers.insert("transfer-encoding".to_string());
        
        Self {
            forward_headers,
            exclude_headers,
        }
    }
}

impl HeaderConfig {
    /// 检查 header 是否应该被转发
    pub fn should_forward(&self, header_name: &str) -> bool {
        let name_lower = header_name.to_lowercase();
        self.forward_headers.contains(&name_lower) && !self.exclude_headers.contains(&name_lower)
    }
}
