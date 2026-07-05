use linguafranca::anthropic::request::AnthropicRequest;
use linguafranca::chat_completions_openai::request::ChatCompletionsOpenAiRequest;
use linguafranca::open_responses::request::OpenResponsesRequest;
use linguafranca::traits::{FromOpenResponses, IntoOpenResponses};

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;

/// 请求转换中间件
///
/// 选择出站协议对应的适配器，处理跨协议请求体格式转换，
/// 将原始请求体（JSON）转为上游请求格式，更新 ctx.request_body。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    let inbound = ctx
        .inbound_protocol
        .clone()
        .unwrap_or_else(|| "openai_chat".to_string());
    let outbound = ctx
        .outbound_protocol
        .clone()
        .unwrap_or_else(|| inbound.clone());

    let target_protocol = &outbound;

    // 注入 stream:true，使所有请求走流式 SSE 路径
    // 本项目网关以流式为核心处理方式，上游不支持非流式响应时（返回空 body），依赖此机制规避
    if let Some(body) = ctx.get_parsed_body() {
        let mut json = body.clone();
        if !json.get("stream").and_then(|v| v.as_bool()).unwrap_or(false) {
            json["stream"] = serde_json::Value::Bool(true);
            // 请求上游在流式最终 chunk 返回精确 token 用量
            json["stream_options"] = serde_json::json!({"include_usage": true});
            ctx.update_body(json).map_err(|e| {
                StageError::new(
                    ctx.clone(),
                    GatewayError::BadRequest(format!("注入 stream 字段失败: {e}")),
                )
            })?;
        }
    }

    // 跨协议时转换请求体格式：inbound → hub → outbound
    let request_bytes = if inbound != outbound {
        tracing::debug!(
            "跨协议格式转换: inbound={}, outbound={}",
            inbound,
            outbound
        );
        convert_request_body(&ctx.request_body, &inbound, &outbound)
            .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e)))?
    } else {
        ctx.request_body.to_vec()
    };

    // 获取 outbound 适配器（生成正确的 URL、认证头、Content-Type）
    let adapter = ctx
        .adapter_registry
        .as_ref()
        .and_then(|reg| reg.get(target_protocol))
        .or_else(|| {
            ctx.adapter_registry
                .as_ref()
                .and_then(|reg| reg.get("openai_chat"))
        })
        .ok_or_else(|| {
            StageError::new(
                ctx.clone(),
                GatewayError::Transform(format!("不支持的协议: {target_protocol}")),
            )
        })?;

    let provider = ctx.provider.as_ref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    let selected_api_key = ctx.selected_api_key.as_deref().ok_or_else(|| {
        StageError::new(
            ctx.clone(),
            GatewayError::Internal("缺少已选中的上游 Key".to_string()),
        )
    })?;

    // 调用 outbound 适配器转换（验证并生成请求头/URL/序列化 body）
    let upstream_req = adapter
        .transform_request(&request_bytes, provider, selected_api_key)
        .await
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e.to_string())))?;

    let new_body = serde_json::to_vec(&upstream_req.body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    ctx.request_body = bytes::Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);
    ctx.upstream_url = Some(upstream_req.url);
    ctx.upstream_method = Some(upstream_req.method);

    Ok(ctx)
}

/// 将请求体从入站格式转换为出站格式（inbound → hub → outbound）
fn convert_request_body(
    body: &[u8],
    from: &str,
    to: &str,
) -> Result<Vec<u8>, String> {
    let hub: OpenResponsesRequest = match from {
        "openai_chat" => {
            let req: ChatCompletionsOpenAiRequest = serde_json::from_slice(body)
                .map_err(|e| format!("解析 Chat 请求失败: {e}"))?;
            req.into_open_responses(None)
                .map_err(|e| format!("Chat → OpenResponses 转换失败: {e}"))?
                .value
        }
        "claude_messages" => {
            let req: AnthropicRequest = serde_json::from_slice(body)
                .map_err(|e| format!("解析 Claude 请求失败: {e}"))?;
            req.into_open_responses(None)
                .map_err(|e| format!("Claude → OpenResponses 转换失败: {e}"))?
                .value
        }
        "openai_response" => serde_json::from_slice(body)
            .map_err(|e| format!("解析 OpenResponses 请求失败: {e}"))?,
        other => return Err(format!("不支持的入站协议: {other}")),
    };

    let outbound: serde_json::Value = match to {
        "openai_chat" => {
            let req = ChatCompletionsOpenAiRequest::from_open_responses(hub, None)
                .map_err(|e| format!("OpenResponses → Chat 转换失败: {e}"))?
                .value;
            serde_json::to_value(req)
                .map_err(|e| format!("序列化 Chat 请求失败: {e}"))?
        }
        "claude_messages" => {
            let req = AnthropicRequest::from_open_responses(hub, None)
                .map_err(|e| format!("OpenResponses → Claude 转换失败: {e}"))?
                .value;
            serde_json::to_value(req)
                .map_err(|e| format!("序列化 Claude 请求失败: {e}"))?
        }
        "openai_response" => serde_json::to_value(hub)
            .map_err(|e| format!("序列化 OpenResponses 请求失败: {e}"))?,
        other => return Err(format!("不支持的出站协议: {other}")),
    };

    serde_json::to_vec(&outbound).map_err(|e| format!("序列化失败: {e}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use axum::http::{HeaderMap, Method};

    use crate::gateway::context::RequestContext;
    use crate::models::Provider;
    use crate::protocol::AdapterRegistry;

    use super::*;

    fn test_provider() -> Provider {
        let now = chrono::Utc::now().naive_utc();
        Provider {
            id: "provider-1".to_string(),
            name: "Test Provider".to_string(),
            protocols: r#"["openai_chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: r#"[]"#.to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://example.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 0,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn test_context(body: serde_json::Value) -> RequestContext {
        let mut ctx = RequestContext::new(
            "req-1".to_string(),
            Instant::now(),
            Method::POST,
            "/v1/chat/completions".parse().expect("valid uri"),
            HeaderMap::new(),
        );
        ctx.request_body = bytes::Bytes::from(serde_json::to_vec(&body).expect("json body"));
        ctx.parsed_body = Some(body);
        ctx.provider = Some(test_provider());
        ctx.selected_api_key = Some("sk-test".to_string());
        ctx.inbound_protocol = Some("openai_chat".to_string());
        ctx.outbound_protocol = Some("openai_chat".to_string());
        ctx.adapter_registry = Some(Arc::new(AdapterRegistry::new()));
        ctx
    }

    #[tokio::test]
    async fn forces_streaming_for_non_streaming_request() {
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "hi"}]
        });

        let ctx = match run(test_context(body)).await {
            Ok(ctx) => ctx,
            Err(err) => panic!("transform request failed: {}", err.error),
        };
        let transformed: serde_json::Value =
            serde_json::from_slice(&ctx.request_body).expect("transformed json");

        // 网关强制注入 stream:true + stream_options
        assert_eq!(transformed.get("stream").and_then(|v| v.as_bool()), Some(true));
        assert!(transformed.get("stream_options").is_some());
    }

    #[tokio::test]
    async fn preserves_explicit_streaming_request() {
        let body = serde_json::json!({
            "model": "gpt-4",
            "stream": true,
            "messages": [{"role": "user", "content": "hi"}]
        });

        let ctx = match run(test_context(body)).await {
            Ok(ctx) => ctx,
            Err(err) => panic!("transform request failed: {}", err.error),
        };
        let transformed: serde_json::Value =
            serde_json::from_slice(&ctx.request_body).expect("transformed json");

        assert_eq!(transformed.get("stream").and_then(|v| v.as_bool()), Some(true));
    }
}
