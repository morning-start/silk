use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};

use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
use crate::protocol::canonical::*;
use crate::models::Provider;

pub struct OpenAIResponseAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIResponseAdapter {
    fn provider_type(&self) -> &'static str {
        "openai_response"
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

        let input = to_response_input(req);

        let mut body = serde_json::json!({
            "model": req.model,
            "input": input,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if req.stream {
            body["stream"] = serde_json::json!(true);
        }

        Ok(UpstreamRequest {
            url: format!("{}/responses", provider.api_base_url),
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

        // 从 output 中提取消息
        let output = resp.body["output"]
            .as_array()
            .ok_or_else(|| ProtocolError::MissingField("output".to_string()))?;

        let mut messages = Vec::new();
        for item in output {
            let item_type = item["type"].as_str().unwrap_or("");
            match item_type {
                "message" => {
                    let role = item["role"].as_str().unwrap_or("assistant");
                    let role = match role {
                        "user" => CanonicalRole::User,
                        "assistant" => CanonicalRole::Assistant,
                        "system" => CanonicalRole::System,
                        _ => CanonicalRole::Assistant,
                    };

                    let empty_vec: Vec<serde_json::Value> = vec![];
                    let content_blocks = item["content"].as_array().unwrap_or(&empty_vec);
                    for block in content_blocks {
                        let block_type = block["type"].as_str().unwrap_or("");
                        match block_type {
                            "text" => {
                                let text = block["text"].as_str().unwrap_or("");
                                messages.push(CanonicalMessage {
                                    role: role.clone(),
                                    content: CanonicalContent::Text {
                                        text: text.to_string(),
                                    },
                                    name: None,
                                    tool_call_id: None,
                                });
                            }
                            "function_call" => {
                                let name = block["name"].as_str().unwrap_or("").to_string();
                                let arguments = block["arguments"].as_str().unwrap_or("{}");
                                let arguments: serde_json::Value = serde_json::from_str(arguments)
                                    .unwrap_or(serde_json::Value::Null);
                                messages.push(CanonicalMessage {
                                    role: role.clone(),
                                    content: CanonicalContent::ToolUse {
                                        name,
                                        arguments,
                                    },
                                    name: None,
                                    tool_call_id: None,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let usage = resp.body["usage"].as_object().map(|u| CanonicalUsage {
            prompt_tokens: u["input_tokens"].as_i64().unwrap_or(0) as i32,
            completion_tokens: u["output_tokens"].as_i64().unwrap_or(0) as i32,
            total_tokens: u["total_tokens"].as_i64().unwrap_or(0) as i32,
        });

        let message = messages.into_iter().next().ok_or_else(|| {
            ProtocolError::MissingField("output".to_string())
        })?;

        Ok(CanonicalResponse {
            id,
            model,
            choices: vec![CanonicalChoice {
                index: 0,
                message,
                finish_reason: Some("stop".to_string()),
            }],
            usage,
        })
    }
}

fn to_response_input(req: &CanonicalRequest) -> serde_json::Value {
    // Response API 使用 input 数组，包含消息对象
    let mut input = Vec::new();
    for msg in &req.messages {
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
                serde_json::json!({"type": "function_call", "name": name, "arguments": arguments})
            }
            CanonicalContent::ToolResult { tool_use_id, content } => {
                serde_json::json!({"type": "function_call_output", "tool_call_id": tool_use_id, "output": content})
            }
        };

        input.push(serde_json::json!({
            "type": "message",
            "role": role,
            "content": content,
        }));
    }
    serde_json::json!(input)
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
            api_base_url: "https://api.openai.com/v1".to_string(),
            api_key: "encrypted".to_string(),
            model_name: Some("gpt-4o".to_string()),
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
    async fn test_canonicalize_request() {
        let adapter = OpenAIResponseAdapter;
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
            model: "gpt-4o".to_string(),
            temperature: None,
            max_tokens: None,
            tools: None,
            stream: false,
            metadata: None,
        };

        let result = adapter.canonicalize_request(&req, &provider).await.unwrap();
        assert_eq!(result.url, "https://api.openai.com/v1/responses");
        assert!(result.body["input"].is_array());
    }

    #[tokio::test]
    async fn test_parse_response() {
        let adapter = OpenAIResponseAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "resp_123",
                "model": "gpt-4o",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "text", "text": "Hello!"}
                    ]
                }],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5,
                    "total_tokens": 15
                }
            }),
        };

        let result = adapter.parse_response(&resp).await.unwrap();
        assert_eq!(result.id, "resp_123");
        assert_eq!(result.choices.len(), 1);
    }
}
