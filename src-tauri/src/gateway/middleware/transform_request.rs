use bytes::Bytes;

use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::protocol::{adapters, prism_wasm};

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

    inject_stream_options(&mut ctx)?;
    inject_claude_default_max_tokens(&mut ctx, &outbound)?;

    let request_bytes = convert_to_outbound_protocol(&ctx, &inbound, &outbound)?;
    apply_upstream_request(&mut ctx, &request_bytes, &outbound)?;

    Ok(ctx)
}

/// 注入 stream:true，使所有请求走流式 SSE 路径
///
/// 本项目网关以流式为核心处理方式，上游不支持非流式响应时（返回空 body），依赖此机制规避。
/// 客户端原始的 stream 取值记录在 `client_requested_stream`，
/// 供 dispatch 阶段决定是下发 SSE 流还是聚合为单个 JSON。
fn inject_stream_options(ctx: &mut RequestContext) -> Result<(), StageError> {
    let original_stream = ctx
        .get_parsed_body()
        .and_then(|body| body.get("stream"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    ctx.client_requested_stream = original_stream;

    if original_stream {
        return Ok(());
    }

    edit_body_checked(ctx, "stream", |json| {
        json["stream"] = serde_json::Value::Bool(true);
        // 请求上游在流式最终 chunk 返回精确 token 用量
        json["stream_options"] = serde_json::json!({"include_usage": true});
    })?;

    tracing::debug!("注入 stream:true + stream_options");
    Ok(())
}

/// 注入默认 max_tokens（Claude 协议必需字段）
///
/// Anthropic API 要求请求中必须包含 max_tokens，当客户端未传入时设置默认值。
fn inject_claude_default_max_tokens(
    ctx: &mut RequestContext,
    outbound: &str,
) -> Result<(), StageError> {
    if outbound != "claude_messages" {
        return Ok(());
    }
    if ctx.get_parsed_body().is_some_and(|b| b.get("max_tokens").is_some()) {
        return Ok(());
    }

    edit_body_checked(ctx, "max_tokens", |json| {
        json["max_tokens"] = serde_json::Value::Number(1024.into());
    })?;

    tracing::debug!("注入默认 max_tokens:1024 (Claude 协议必需)");
    Ok(())
}

/// 跨协议转换请求体（同协议时原样返回）
fn convert_to_outbound_protocol(
    ctx: &RequestContext,
    inbound: &str,
    outbound: &str,
) -> Result<Bytes, StageError> {
    if inbound == outbound {
        return Ok(Bytes::from(ctx.request_body.to_vec()));
    }

    let body_str = String::from_utf8_lossy(&ctx.request_body).to_string();
    let start = std::time::Instant::now();
    let converted = prism_wasm::convert_request(inbound, &body_str, outbound)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Transform(e)))?;

    tracing::info!(
        inbound = %inbound,
        outbound = %outbound,
        input_bytes = body_str.len(),
        output_bytes = converted.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "请求体跨协议转换完成"
    );
    Ok(Bytes::from(converted))
}

/// 调用适配器构建上游请求，并把 URL / 方法 / 头 / 请求体写回 ctx
fn apply_upstream_request(
    ctx: &mut RequestContext,
    request_bytes: &Bytes,
    target_protocol: &str,
) -> Result<(), StageError> {
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

    let upstream_req = adapters::build_upstream_request(
        request_bytes,
        provider,
        selected_api_key,
        target_protocol,
    )
    .map_err(|e| StageError::new(ctx.clone(), e))?;

    let new_body = serde_json::to_vec(&upstream_req.body)
        .map_err(|e| StageError::new(ctx.clone(), GatewayError::Serialization(e.to_string())))?;

    ctx.request_body = Bytes::from(new_body);
    ctx.upstream_headers = Some(upstream_req.headers);
    ctx.upstream_url = Some(upstream_req.url);
    ctx.upstream_method = Some(upstream_req.method);

    Ok(())
}

/// 修改请求体，序列化失败时统一包装为 BadRequest 错误
fn edit_body_checked(
    ctx: &mut RequestContext,
    field: &'static str,
    edit: impl FnOnce(&mut serde_json::Value),
) -> Result<(), StageError> {
    ctx.edit_body(edit).map_err(|e| {
        StageError::new(
            ctx.clone(),
            GatewayError::BadRequest(format!("注入 {field} 字段失败: {e}")),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use axum::http::{HeaderMap, Method};

    use crate::gateway::context::RequestContext;
    use crate::models::Provider;

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
            custom_headers: "[]".to_string(),
            models_passthrough: 0,
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
        ctx.inbound_protocol = Some("openai".to_string());
        ctx.outbound_protocol = Some("openai".to_string());
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
        assert_eq!(
            transformed.get("stream").and_then(|v| v.as_bool()),
            Some(true)
        );
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

        assert_eq!(
            transformed.get("stream").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}
