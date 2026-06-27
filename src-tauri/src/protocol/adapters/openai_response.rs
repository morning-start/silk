use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};

use linguafranca::open_responses::request::OpenResponsesRequest;

use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
use crate::models::Provider;

pub struct OpenAIResponseAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIResponseAdapter {
    fn provider_type(&self) -> &'static str {
        "openai_response"
    }

    async fn transform_request(
        &self,
        req_body: &[u8],
        provider: &Provider,
    ) -> Result<UpstreamRequest, ProtocolError> {
        let _openai_req: OpenResponsesRequest = serde_json::from_slice(req_body)
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
            url: format!("{}/v1/responses", provider.api_base_url),
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

        Ok(resp.body.clone())
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
            provider_type: "openai".to_string(),
            protocols: r#"["response"]"#.to_string(),
            models: r#"["gpt-4o"]"#.to_string(),
            keys: r#"[{"name":"主密钥","value":"encrypted","enabled":true,"weight":1}]"#.to_string(),
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
        let adapter = OpenAIResponseAdapter;
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "gpt-4o",
            "input": [{"role": "user", "content": "Hello"}]
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = adapter.transform_request(&req_bytes, &provider).await.unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/responses");
        assert!(result.body["model"].as_str().unwrap() == "gpt-4o");
    }

    #[tokio::test]
    async fn test_transform_response() {
        let adapter = OpenAIResponseAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "resp_123",
                "object": "response",
                "created_at": 1234567890,
                "status": "completed",
                "model": "gpt-4o",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "Hello!"}]
                }],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5,
                    "total_tokens": 15
                }
            }),
        };

        let result = adapter.transform_response(&resp).await.unwrap();
        assert!(result["id"].as_str().unwrap() == "resp_123");
    }
}
