use sqlx::SqlitePool;

use crate::models::NewRequestLog;
use crate::persistence::ModelMappingRepo;

/// 计算一批日志的 cost（费用）
///
/// 在消费侧（日志写入任务）调用，不阻塞请求热路径。
/// 收集需要计算 cost 的模型名，批量查询 model_mappings 表，
/// 按 input/output token 用量和单价计算费用。
pub async fn compute_batch_costs(logs: &mut [NewRequestLog], pool: &SqlitePool) {
    // 收集所有需要计算 cost 的模型名
    let uncosted_models: Vec<String> = logs
        .iter()
        .filter(|log| log.cost.is_none())
        .filter_map(|log| log.model_id.clone())
        .collect();

    if uncosted_models.is_empty() {
        return;
    }

    // 去重，批量查询避免逐条 DB 查询
    let unique_models: Vec<String> = {
        let mut set: Vec<String> = Vec::new();
        for m in uncosted_models {
            if !set.contains(&m) {
                set.push(m);
            }
        }
        set
    };

    let mappings = match ModelMappingRepo::find_by_model_names(pool, &unique_models).await {
        Ok(m) => m,
        Err(_) => return,
    };

    for log in logs.iter_mut() {
        if log.cost.is_some() {
            continue;
        }
        let model_name = match &log.model_id {
            Some(m) => m,
            None => continue,
        };
        if let Some(mapping) = mappings.iter().find(|m| &m.model_name == model_name) {
            if let (Some(input_price), Some(output_price)) =
                (mapping.input_price_per_1m, mapping.output_price_per_1m)
            {
                let inp = log.tokens_input.unwrap_or(0) as f64 / 1_000_000.0 * input_price;
                let out = log.tokens_output.unwrap_or(0) as f64 / 1_000_000.0 * output_price;
                log.cost = Some(inp + out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uncosted_models_filter() {
        let logs = [
            NewRequestLog {
                model_id: Some("gpt-4".to_string()),
                cost: None,
                ..Default::default()
            },
            NewRequestLog {
                model_id: Some("gpt-3.5".to_string()),
                cost: Some(0.01),
                ..Default::default()
            },
        ];
        let uncosted: Vec<String> = logs
            .iter()
            .filter(|log| log.cost.is_none())
            .filter_map(|log| log.model_id.clone())
            .collect();
        assert_eq!(uncosted, vec!["gpt-4"]);
    }
}
