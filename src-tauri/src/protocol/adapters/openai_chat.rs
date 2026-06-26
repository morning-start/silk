use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
use crate::protocol::canonical::*;
use crate::models::Provider;

pub struct OpenAIChatAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIChatAdapter {
    fn provider_type(&self) -> &'static str {
        "openai_chat"
    }

    async fn canonicalize_request(
        &self,
        req: &CanonicalRequest,
        provider: &Provider,
    ) -> Result<UpstreamRequest, ProtocolError> {
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

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": to_openai_messages(&req.messages)?,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = req.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(tools) = &req.tools {
            body["tools"] = serde_json::json!(to_openai_tools(tools));
        }
        if req.stream {
            body["stream"] = serde_json::json!(true);
        }

        Ok(UpstreamRequest {
            url: format!("{}/chat/completions", provider.api_base_url),
            headers,
            body,
        })
    }

    async fn parse_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<CanonicalResponse, ProtocolError> {
        if resp.status >= 400 {
            return Err(ProtocolError::InvalidValue {
                field: "status".to_string(),
                reason: format!("HTTP {}: {}", resp.status, json_err_msg(&resp.body)),
            });
        }

        let id = resp.body["id"]
            .as_str()
            .ok_or_else(|| ProtocolError::MissingField("id".to_string()))?
            .to_string();

        let model = resp.body["model"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let choices = resp.body["choices"]
            .as_array()
            .ok_or_else(|| ProtocolError::MissingField("choices".to_string()))?;

        let mut canonical_choices = Vec::new();
        for (i, choice) in choices.iter().enumerate() {
            let message = choice["message"]
                .as_object()
                .ok_or_else(|| ProtocolError::MissingField("message".to_string()))?;

            let role = message["role"]
                .as_str()
                .ok_or_else(|| ProtocolError::MissingField("role".to_string()))?;

            let role = match role {
                "user" => CanonicalRole::User,
                "assistant" => CanonicalRole::Assistant,
                "system" => CanonicalRole::System,
                "tool" => CanonicalRole::Tool,
                _ => {
                    return Err(ProtocolError::InvalidValue {
                        field: "role".to_string(),
                        reason: format!("未知角色: {role}"),
                    })
                }
            };

            let content = message["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let finish_reason = choice["finish_reason"]
                .as_str()
                .map(|s| s.to_string());

            canonical_choices.push(CanonicalChoice {
                index: i as i32,
                message: CanonicalMessage {
                    role,
                    content: CanonicalContent::Text { text: content },
                    name: None,
                    tool_call_id: None,
                },
                finish_reason,
            });
        }

        let usage = resp.body["usage"].as_object().map(|u| CanonicalUsage {
            prompt_tokens: u["prompt_tokens"].as_i64().unwrap_or(0) as i32,
            completion_tokens: u["completion_tokens"].as_i64().unwrap_or(0) as i32,
            total_tokens: u["total_tokens"].as_i64().unwrap_or(0) as i32,
        });

        Ok(CanonicalResponse {
            id,
            model,
            choices: canonical_choices,
            usage,
        })
    }
}

fn to_openai_messages(messages: &[CanonicalMessage]) -> Result<Vec<serde_json::Value>, ProtocolError> {
    let mut result = Vec::new();
    for msg in messages {
        let role = match msg.role {
            CanonicalRole::User => "user",
            CanonicalRole::Assistant => "assistant",
            CanonicalRole::System => "system",
            CanonicalRole::Tool => "tool",
        };

        let content = match &msg.content {
            CanonicalContent::Text { text } => serde_json::json!(text),
            CanonicalContent::ImageUrl { image_url } => {
                serde_json::json!({"type": "image_url", "image_url": {"url": image_url}})
            }
            CanonicalContent::ToolUse { name, arguments } => {
                serde_json::json!({"type": "tool_use", "name": name, "input": arguments})
            }
            CanonicalContent::ToolResult { tool_use_id, content } => {
                serde_json::json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content})
            }
        };

        let mut msg_obj = serde_json::json!({
            "role": role,
            "content": content,
        });

        if let Some(ref name) = msg.name {
            msg_obj["name"] = serde_json::json!(name);
        }
        if let Some(ref tool_call_id) = msg.tool_call_id {
            msg_obj["tool_call_id"] = serde_json::json!(tool_call_id);
        }

        result.push(msg_obj);
    }
    Ok(result)
}

fn to_openai_tools(tools: &[CanonicalTool]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect()
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
            api_base_url: "https://api.openai.com/v1".to_string(),
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
    async fn test_canonicalize_simple_request() {
        let adapter = OpenAIChatAdapter;
        let provider = test_provider();
        let req = CanonicalRequest {
            messages: vec![CanonicalMessage {
                role: CanonicalRole::User,
                content: CanonicalContent::Text {
                    text: "Hello".to_string(),
                },
                name: None,
                tool_call_id: None,
            }],
            model: "gpt-4".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: None,
            stream: false,
            metadata: None,
        };

        let result = adapter.canonicalize_request(&req, &provider).await.unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/chat/completions");
        assert!(result.body["model"].as_str().unwrap() == "gpt-4");
        assert!(result.body["temperature"].as_f64().unwrap() - 0.7 < 0.001);
    }

    #[tokio::test]
    async fn test_parse_response() {
        let adapter = OpenAIChatAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "chatcmpl-123",
                "model": "gpt-4",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": {"type": "text", "text": "Hi!" }
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

        let result = adapter.parse_response(&resp).await.unwrap();
        assert_eq!(result.id, "chatcmpl-123");
        assert_eq!(result.choices.len(), 1);
        assert_eq!(result.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[tokio::test]
    async fn test_parse_error_response() {
        let adapter = OpenAIChatAdapter;
        let resp = UpstreamResponse {
            status: 400,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "error": {"message": "Invalid API key"}
            }),
        };

        let result = adapter.parse_response(&resp).await;
        assert!(result.is_err());
    }
}
