use async_trait::async_trait;

use crate::models::Provider;
use crate::protocol::adapter::{
    build_anthropic_headers, build_upstream, ProtocolError, ProviderAdapter, UpstreamRequest,
    UpstreamResponse,
};

pub struct ClaudeMessagesAdapter;

#[async_trait]
impl ProviderAdapter for ClaudeMessagesAdapter {
    fn provider_type(&self) -> &'static str {
        "claude_messages"
    }

    async fn transform_request(
        &self,
        req_body: &[u8],
        provider: &Provider,
        selected_api_key: &str,
    ) -> Result<UpstreamRequest, ProtocolError> {
        build_upstream(
            req_body,
            provider,
            selected_api_key,
            "v1/messages",
            build_anthropic_headers,
        )
    }

    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError> {
        // 协议转换已由 prism.wasm 完成，此处透传上游响应
        Ok(resp.body.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    fn test_provider() -> Provider {
        Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            protocols: r#"["message"]"#.to_string(),
            models: r#"["claude-3-opus"]"#.to_string(),
            keys: r#"[{"name":"主密钥","value":"encrypted","enabled":true,"weight":1}]"#
                .to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://api.anthropic.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 3,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            custom_headers: "[]".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }

    #[tokio::test]
    async fn test_transform_request() {
        let adapter = ClaudeMessagesAdapter;
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 1024
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = adapter
            .transform_request(&req_bytes, &provider, "sk-test")
            .await
            .unwrap();
        assert_eq!(result.url, "https://api.anthropic.com/v1/messages");
        assert!(result.body["model"].as_str().unwrap() == "claude-3-opus");
    }

    #[tokio::test]
    async fn test_transform_response() {
        let adapter = ClaudeMessagesAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "msg_123",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "Hello!"}],
                "model": "claude-3-opus",
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 10, "output_tokens": 5}
            }),
        };

        let result = adapter.transform_response(&resp).await.unwrap();
        assert!(result.is_object());
    }
}
