use async_trait::async_trait;

use linguafranca::open_responses::request::OpenResponsesRequest;

use crate::models::Provider;
use crate::protocol::adapter::{
    build_bearer_headers, ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse,
};

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
        selected_api_key: &str,
    ) -> Result<UpstreamRequest, ProtocolError> {
        // 反序列化一次，用于验证 + 生成 body
        let openai_req: OpenResponsesRequest = serde_json::from_slice(req_body)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
        let body = serde_json::to_value(&openai_req)
            .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;

        Ok(UpstreamRequest {
            url: format!("{}/v1/responses", provider.api_base_url),
            method: "POST".to_string(),
            headers: build_bearer_headers(selected_api_key)?,
            body,
        })
    }

    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError> {
        // OpenAI Response 格式直通，无需转换
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
            protocols: r#"["response"]"#.to_string(),
            models: r#"["gpt-4o"]"#.to_string(),
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
        let adapter = OpenAIResponseAdapter;
        let provider = test_provider();
        let req_body = serde_json::json!({
            "model": "gpt-4o",
            "input": [{"role": "user", "content": "Hello"}]
        });
        let req_bytes = serde_json::to_vec(&req_body).unwrap();

        let result = adapter
            .transform_request(&req_bytes, &provider, "sk-test")
            .await
            .unwrap();
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
