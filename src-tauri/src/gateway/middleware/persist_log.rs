use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::persistence::ModelMappingRepo;

use super::{headers_to_json, maybe_body_text};

/// 日志异步写入中间件
///
/// 只记录 /v1/* 路径的模型请求，记录协议、流式状态、模型名、token 用量等
pub async fn run(
    log_sender: &tokio::sync::mpsc::Sender<crate::models::NewRequestLog>,
    ctx: &mut RequestContext,
) -> Result<(), GatewayError> {
    // 只记录 /v1/* 路径的请求（模型相关）
    if !ctx.path.starts_with("/v1/") {
        return Ok(());
    }

    let route = ctx.route.as_ref();
    let provider = ctx.provider.as_ref();
    let status = ctx
        .final_status
        .or(ctx.upstream_status)
        .map(|value| value.as_u16() as i64);

    // 从请求体 JSON 提取 model 和 stream 字段
    let request_body = maybe_body_text(&ctx.request_body);
    let (model_from_body, stream_from_body) = request_body
        .as_deref()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .map(|json| {
            let model = json
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let stream = json
                .get("stream")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (model, stream)
        })
        .unwrap_or((None, false));

    // 流式判定：请求体 stream=true 优先，兜底看 upstream_body 是否为空
    let is_streaming = stream_from_body || ctx.upstream_body.is_none();

    // 模型名：remote_model_override > 请求体 model > 路由或 Provider
    let model_used = ctx
        .remote_model_override
        .clone()
        .or(model_from_body)
        .or_else(|| {
            route
                .and_then(|r| r.model_name_override.clone())
                .or_else(|| provider.and_then(|p| p.models_vec().first().cloned()))
        });

    // 从响应体提取 token 用量（非流式场景）
    let response_body = ctx
        .upstream_body
        .as_ref()
        .and_then(|body| maybe_body_text(body));
    let (tokens_input, tokens_output) = response_body
        .as_deref()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .and_then(|json| {
            json.get("usage").and_then(|usage| {
                let inp = usage.get("prompt_tokens").and_then(|v| v.as_i64());
                let out = usage.get("completion_tokens").and_then(|v| v.as_i64());
                if inp.is_some() || out.is_some() {
                    Some((inp, out))
                } else {
                    None
                }
            })
        })
        .unwrap_or((None, None));

    // 计费：通过模型映射价格计算本次费用（仅非流式响应有完整 tokens 信息）
    let cost = calculate_cost(
        &model_used,
        tokens_input,
        tokens_output,
    )
    .await;

    let response_headers = ctx.upstream_headers.as_ref().and_then(headers_to_json);
    let request_headers = headers_to_json(&ctx.headers);

    let log = crate::models::NewRequestLog {
        request_id: ctx.request_id.clone(),
        method: ctx.method.to_string(),
        path: ctx.path.clone(),
        // 无 routing rule 时有认证 key 则记录 key 名
        route_id: route.map(|route| route.id.clone()),
        inbound_protocol: ctx.inbound_protocol.clone(),
        outbound_protocol: ctx.outbound_protocol.clone(),
        request_headers,
        request_body,
        response_status: status,
        status_code: status,
        response_headers,
        response_body,
        duration_ms: Some(ctx.elapsed_ms()),
        provider_id: provider.filter(|p| !p.id.is_empty()).map(|provider| provider.id.clone()),
        error_message: ctx.error_message.clone(),
        error_code: ctx.error_code.clone(),
        model_used,
        retry_count: Some(0),
        stream_enabled: Some(is_streaming),
        cache_hit: Some(false),
        request_size_bytes: Some(ctx.request_size_bytes()),
        response_size_bytes: ctx.response_size_bytes(),
        tokens_input,
        tokens_output,
        cost,
        auth_key_name: ctx.auth_key_name.clone(),
    };

    if let Err(err) = log_sender.send(log).await {
        tracing::warn!(%err, "发送日志到 channel 失败");
    }

    Ok(())
}

/// 通过模型映射价格计算本次请求费用
///
/// 公式：(tokens_input / 1_000_000) × input_price_per_1m + (tokens_output / 1_000_000) × output_price_per_1m
async fn calculate_cost(
    model_used: &Option<String>,
    tokens_input: Option<i64>,
    tokens_output: Option<i64>,
) -> Option<f64> {
    let model_name = model_used.as_ref()?;

    let pool = crate::get_db_pool()?;
    let mapping = ModelMappingRepo::find_by_model_name(pool, model_name)
        .await
        .ok()
        .flatten()?;

    let input_price = mapping.input_price_per_1m?;
    let output_price = mapping.output_price_per_1m?;

    let inp = tokens_input.unwrap_or(0) as f64 / 1_000_000.0 * input_price;
    let out = tokens_output.unwrap_or(0) as f64 / 1_000_000.0 * output_price;

    Some(inp + out)
}
