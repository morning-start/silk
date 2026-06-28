use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};

use linguafranca::anthropic::request::AnthropicRequest;
use linguafranca::anthropic::response::AnthropicResponse;
use linguafranca::traits::IntoOpenResponses;

use crate::models::Provider;
use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};

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
        let _anthropic_req: AnthropicRequest = serde_json::from_slice(req_body)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(selected_api_key).map_err(|e| ProtocolError::InvalidValue {
                field: "x-api-key".to_string(),
                reason: e.to_string(),
            })?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let body: serde_json::Value = serde_json::from_slice(req_body)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        Ok(UpstreamRequest {
            url: format!("{}/v1/messages", provider.api_base_url),
            method: "POST".to_string(),
            headers,
            body,
        })
    }

    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError> {
        if resp.status >= 400 {
            return Err(ProtocolError::UpstreamError {
                status: resp.status,
                message: json_err_msg(&resp.body),
            });
        }

        let anthropic_resp: AnthropicResponse = serde_json::from_value(resp.body.clone())
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        let openai_resp = anthropic_resp
            .into_open_responses(None)
            .map_err(|e| ProtocolError::Transform(e.to_string()))?
            .value;

        serde_json::to_value(openai_resp)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))
    }
}

fn json_err_msg(body: &serde_json::Value) -> String {
    body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| body["error"]["type"].as_str().unwrap_or("unknown error"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> Provider {
        Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            provider_type: "anthropic".to_string(),
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
