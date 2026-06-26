use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};

use crate::protocol::adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
use crate::protocol::canonical::*;
use crate::models::Provider;

pub struct ClaudeMessagesAdapter;

#[async_trait]
impl ProviderAdapter for ClaudeMessagesAdapter {
    fn provider_type(&self) -> &'static str {
        "claude_messages"
    }

    async fn canonicalize_request(
        &self,
        req: &CanonicalRequest,
        provider: &Provider,
    ) -> Result<UpstreamRequest, ProtocolError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(
                &provider.decrypted_api_key(&[0u8; 32]).unwrap_or_default(),
            )
            .map_err(|e| ProtocolError::InvalidValue {
                field: "x-api-key".to_string(),
                reason: e.to_string(),
            })?,
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        // 提取 system 消息
        let system_messages: Vec<_> = req
            .messages
            .iter()
            .filter(|m| m.role == CanonicalRole::System)
            .collect();

        let messages: Vec<_> = req
            .messages
            .iter()
            .filter(|m| m.role != CanonicalRole::System)
            .collect();

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": to_claude_messages(&messages)?,
        });

        if !system_messages.is_empty() {
            let system_text: Vec<String> = system_messages
                .iter()
                .filter_map(|m| match &m.content {
                    CanonicalContent::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            if !system_text.is_empty() {
                body["system"] = serde_json::json!(system_text.join("\n"));
            }
        }

        if let Some(max_tokens) = req.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        } else {
            body["max_tokens"] = serde_json::json!(4096);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if req.stream {
            body["stream"] = serde_json::json!(true);
        }

        Ok(UpstreamRequest {
            url: format!("{}/v1/messages", provider.api_base_url),
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

        let content_blocks = resp.body["content"]
            .as_array()
            .ok_or_else(|| ProtocolError::MissingField("content".to_string()))?;

        let mut messages = Vec::new();
        for block in content_blocks {
            let block_type = block["type"].as_str().ok_or_else(|| {
                ProtocolError::MissingField("type".to_string())
            })?;

            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("");
                    messages.push(CanonicalMessage {
                        role: CanonicalRole::Assistant,
                        content: CanonicalContent::Text {
                            text: text.to_string(),
                        },
                        name: None,
                        tool_call_id: None,
                    });
                }
                "tool_use" => {
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let input = block["input"].clone();
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    messages.push(CanonicalMessage {
                        role: CanonicalRole::Assistant,
                        content: CanonicalContent::ToolUse {
                            name,
                            arguments: input,
                        },
                        name: None,
                        tool_call_id: Some(id),
                    });
                }
                _ => {}
            }
        }

        let stop_reason = resp.body["stop_reason"]
            .as_str()
            .map(|s| s.to_string());

        let usage = resp.body["usage"].as_object().map(|u| CanonicalUsage {
            prompt_tokens: u["input_tokens"].as_i64().unwrap_or(0) as i32,
            completion_tokens: u["output_tokens"].as_i64().unwrap_or(0) as i32,
            total_tokens: u["input_tokens"].as_i64().unwrap_or(0) as i32
                + u["output_tokens"].as_i64().unwrap_or(0) as i32,
        });

        Ok(CanonicalResponse {
            id,
            model,
            choices: vec![CanonicalChoice {
                index: 0,
                message: messages.into_iter().next().ok_or_else(|| {
                    ProtocolError::MissingField("content".to_string())
                })?,
                finish_reason: stop_reason,
            }],
            usage,
        })
    }
}

fn to_claude_messages(
    messages: &[&CanonicalMessage],
) -> Result<Vec<serde_json::Value>, ProtocolError> {
    let mut result = Vec::new();
    for msg in messages {
        let role = match msg.role {
            CanonicalRole::User => "user",
            CanonicalRole::Assistant => "assistant",
            _ => {
                return Err(ProtocolError::InvalidValue {
                    field: "role".to_string(),
                    reason: format!("Claude 不支持 {:?} 角色在 messages 中", msg.role),
                })
            }
        };

        let content = match &msg.content {
            CanonicalContent::Text { text } => serde_json::json!({"type": "text", "text": text}),
            CanonicalContent::ImageUrl { image_url } => {
                serde_json::json!({"type": "image", "source": {"type": "url", "url": image_url}})
            }
            CanonicalContent::ToolUse { name, arguments } => {
                serde_json::json!({"type": "tool_use", "name": name, "input": arguments, "id": msg.tool_call_id})
            }
            CanonicalContent::ToolResult { tool_use_id, content } => {
                serde_json::json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content})
            }
        };

        result.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }
    Ok(result)
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
            api_base_url: "https://api.anthropic.com".to_string(),
            api_key: "encrypted".to_string(),
            model_name: Some("claude-3-opus".to_string()),
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
    async fn test_canonicalize_with_system() {
        let adapter = ClaudeMessagesAdapter;
        let provider = test_provider();
        let req = CanonicalRequest {
            messages: vec![
                CanonicalMessage {
                    role: CanonicalRole::System,
                    content: CanonicalContent::Text {
                        text: "You are helpful".to_string(),
                    },
                    name: None,
                    tool_call_id: None,
                },
                CanonicalMessage {
                    role: CanonicalRole::User,
                    content: CanonicalContent::Text {
                        text: "Hello".to_string(),
                    },
                    name: None,
                    tool_call_id: None,
                },
            ],
            model: "claude-3-opus".to_string(),
            temperature: None,
            max_tokens: None,
            tools: None,
            stream: false,
            metadata: None,
        };

        let result = adapter.canonicalize_request(&req, &provider).await.unwrap();
        assert_eq!(result.url, "https://api.anthropic.com/v1/messages");
        assert!(result.body["system"].as_str().unwrap().contains("You are helpful"));
        assert_eq!(result.body["max_tokens"], serde_json::json!(4096));
    }

    #[tokio::test]
    async fn test_parse_response() {
        let adapter = ClaudeMessagesAdapter;
        let resp = UpstreamResponse {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::json!({
                "id": "msg_123",
                "model": "claude-3-opus",
                "content": [
                    {"type": "text", "text": "Hello there!"},
                    {"type": "tool_use", "name": "search", "input": {"q": "weather"}, "id": "tool_1"}
                ],
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 10, "output_tokens": 5}
            }),
        };

        let result = adapter.parse_response(&resp).await.unwrap();
        assert_eq!(result.id, "msg_123");
        assert_eq!(result.choices.len(), 1);
    }
}
