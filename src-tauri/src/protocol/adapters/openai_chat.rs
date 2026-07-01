use async_trait::async_trait;

use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse;
use linguafranca::traits::IntoOpenResponses;

use crate::models::Provider;
use crate::protocol::adapter::{
    build_bearer_headers, build_upstream, ProtocolError, ProviderAdapter, UpstreamRequest,
    UpstreamResponse,
};

pub struct OpenAIChatAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIChatAdapter {
    fn provider_type(&self) -> &'static str {
        "openai_chat"
    }

    async fn transform_request(
        &self,
        req_body: &[u8],
        provider: &Provider,
        selected_api_key: &str,
    ) -> Result<UpstreamRequest, ProtocolError> {
        build_upstream::<ChatCompletionsOpenAiRequest>(
            req_body,
            provider,
            selected_api_key,
            "v1/chat/completions",
            build_bearer_headers,
        )
    }

    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError> {
        let chat_resp: ChatCompletionsOpenAiResponse = serde_json::from_value(resp.body.clone())
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        let openai_resp = chat_resp
            .into_open_responses(None)
            .map_err(|e| ProtocolError::Transform(e.to_string()))?
            .value;

        serde_json::to_value(openai_resp)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))
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
            protocols: r#"["chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: r#"[{"name":"主密钥","value":"encrypted","enabled":true,"weight":1}]"#
                .to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://api.openai.com".to_string(),
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
        let adapter = OpenAIChatAdapter;
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 0.7
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = adapter
            .transform_request(&req_bytes, &provider, "sk-test")
            .await
            .unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/chat/completions");
        assert!(result.body["model"].as_str().unwrap() == "gpt-4");
    }

    #[tokio::test]
    async fn test_transform_response() {
        let adapter = OpenAIChatAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1234567890,
                "model": "gpt-4",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hi!"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15
                }
            }),
        };

        let result = adapter.transform_response(&resp).await.unwrap();
        assert!(result.is_object());
    }
}
