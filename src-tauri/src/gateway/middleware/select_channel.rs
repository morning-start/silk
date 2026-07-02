use crate::crypto::decrypt;
use crate::gateway::context::RequestContext;
use crate::gateway::error::GatewayError;
use crate::gateway::pipeline::StageError;
use crate::load_balancer::{LoadBalanceStrategy, LoadBalancer};
use crate::models::ProviderKeyEntry;

/// 渠道映射中间件
///
/// 在模型池/Provider 已确定后，按 Provider 自身的 key 负载均衡策略选择一个上游 API Key。
/// 已失败的 Key（在 ctx.failed_keys 中）会被跳过。
pub async fn run(mut ctx: RequestContext) -> Result<RequestContext, StageError> {
    let error_ctx = ctx.clone();
    let provider = ctx.provider.as_ref().ok_or_else(|| {
        StageError::new(
            error_ctx.clone(),
            GatewayError::Internal("缺少 provider".to_string()),
        )
    })?;

    // 解析所有 Key 条目，排除已失败的 Key
    let all_entries: Vec<ProviderKeyEntry> =
        serde_json::from_str(&provider.keys).map_err(|e| {
            StageError::new(
                error_ctx.clone(),
                GatewayError::Internal(format!("Provider keys JSON 格式错误: {e}")),
            )
        })?;

    let available: Vec<&ProviderKeyEntry> = all_entries
        .iter()
        .filter(|e| e.enabled && !e.value.is_empty() && !ctx.failed_keys.contains(&e.value))
        .collect();

    if available.is_empty() {
        return Err(StageError::new(
            error_ctx,
            GatewayError::Transform("所有上游 Key 均已失败，无可用 Key".to_string()),
        ));
    }

    // 按 Provider 的策略从可用 Key 中选一个
    let strategy = LoadBalanceStrategy::from_str(&provider.key_strategy);
    let items: Vec<_> = available.iter().map(|e| (*e).clone()).collect();
    let balancer = LoadBalancer::new(items, strategy);
    let selected = balancer.select().ok_or_else(|| {
        StageError::new(
            error_ctx.clone(),
            GatewayError::Transform("选择上游 Key 失败".to_string()),
        )
    })?;

    // 解密选中的 Key（数据库存储的是加密密文）
    let decrypted = decrypt(&selected.value).map_err(|e| {
        StageError::new(
            error_ctx.clone(),
            GatewayError::Internal(format!("解密 API Key 失败: {e}")),
        )
    })?;
    ctx.selected_api_key = Some(decrypted);
    Ok(ctx)
}
