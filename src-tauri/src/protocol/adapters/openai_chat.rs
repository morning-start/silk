use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};

use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::chat_completions_openai::response::ChatCompletionsOpenAiResponse;
use linguafranca::traits::IntoOpenResponses;

use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
use crate::models::Provider;

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
    ) -> Result<UpstreamRequest, ProtocolError> {
        let _chat_req: ChatCompletionsOpenAiRequest = serde_json::from_slice(req_body)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!(
                "Bearer {}",
                provider.decrypted_api_key(&[0u8; 32]).unwrap_or_default()
            ))
            .map_err(|e| ProtocolError::InvalidValue {
                field: "Authorization".to_string(),
                reason: e.to_string(),
            })?,
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let body: serde_json::Value = serde_json::from_slice(req_body)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        Ok(UpstreamRequest {
            url: format!("{}/v1/chat/completions", provider.api_base_url),
            headers,
            body,
        })
    }

    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError> {
        if resp.status >= 400 {
            return Err(ProtocolError::InvalidValue {
                field: "status".to_string(),
                reason: format!("HTTP {}: {}", resp.status, json_err_msg(&resp.body)),
            });
        }

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

fn json_err_msg(body: &serde_json::Value) -> String {
    body["error"]["message"]
        .as_str()
        .unwrap_or("unknown error")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> Provider {
        Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            provider_type: "openai".to_string(),
            protocols: r#"["chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            api_base_url: "https://api.openai.com".to_string(),
            api_key: "encrypted".to_string(),
            model_name: Some("gpt-4".to_string()),
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

        let result = adapter.transform_request(&req_bytes, &provider).await.unwrap();
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
