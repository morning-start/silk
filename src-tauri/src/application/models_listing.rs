use serde::Serialize;

use crate::error::ServiceError;
use crate::persistence::{ModelMappingRepo, ProviderRepo};

/// 全量模型条目（与 /v1/models 的 data[] 结构一致，额外带 model_mapping_id）
#[derive(Debug, Clone, Serialize)]
pub struct ModelListingItem {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    /// 模型池映射 ID，前端 dropdown 用此作 value；Provider 模型为 None
    pub model_mapping_id: Option<String>,
}

/// 返回全量模型列表：模型池（top） + 各 Provider 模型（按 owned_by 排序）
/// 与网关 /v1/models 完全一致，去重。
pub async fn list_all_models() -> Result<Vec<ModelListingItem>, ServiceError> {
    let pool = crate::error::require_db()?;

    // ① 模型池（silk）
    let pool_models = match ModelMappingRepo::find_enabled(pool).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(%e, "查询模型池列表失败");
            Vec::new()
        }
    };

    // ② 渠道模型
    let providers = match ProviderRepo::find_enabled(pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(%e, "查询渠道列表失败");
            Vec::new()
        }
    };

    let mut items: Vec<ModelListingItem> = Vec::with_capacity(
        pool_models.len() + providers.iter().map(|p| p.models_vec().len()).sum::<usize>(),
    );

    // 模型池条目（top）
    for m in &pool_models {
        items.push(ModelListingItem {
            id: m.model_name.clone(),
            object: "model".to_string(),
            created: m.created_at.and_utc().timestamp(),
            owned_by: "silk".to_string(),
            model_mapping_id: Some(m.id.clone()),
        });
    }

    // 渠道条目（每个 Provider 内去重，按 owned_by 排序）
    let mut provider_entries: Vec<ModelListingItem> = Vec::new();
    for provider in &providers {
        let models = provider.models_vec();
        let mut seen = std::collections::HashSet::new();
        for model_id in models {
            if seen.insert(model_id.clone()) {
                provider_entries.push(ModelListingItem {
                    id: model_id,
                    object: "model".to_string(),
                    created: provider.created_at.and_utc().timestamp(),
                    owned_by: provider.name.clone(),
                    model_mapping_id: None,
                });
            }
        }
    }

    // 按 owned_by 排序
    provider_entries.sort_by(|a, b| a.owned_by.cmp(&b.owned_by));

    items.extend(provider_entries);
    Ok(items)
}