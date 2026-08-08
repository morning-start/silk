//! 一键配置服务
//!
//! 根据检测结果自动生成AI服务配置。

use serde::{Deserialize, Serialize};

/// 快速配置请求
#[derive(Debug, Deserialize)]
pub struct QuickSetupRequest {
    /// 选择的服务ID列表
    pub services: Vec<String>,
    /// 服务ID -> API密钥的映射
    pub api_keys: std::collections::HashMap<String, String>,
}

/// 快速配置响应
#[derive(Debug, Serialize)]
pub struct QuickSetupResponse {
    /// 是否成功
    pub success: bool,
    /// 消息
    pub message: String,
    /// 已配置的服务列表
    pub configured_services: Vec<String>,
}

/// 预置服务配置
struct PresetServiceConfig {
    name: &'static str,
    protocols: &'static str,
    models: &'static str,
    api_base_url: &'static str,
}

/// 获取预置服务配置
fn get_preset_config(service_id: &str) -> Option<PresetServiceConfig> {
    match service_id {
        "openai" => Some(PresetServiceConfig {
            name: "OpenAI",
            protocols: r#"["openai_chat"]"#,
            models: r#"["gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"]"#,
            api_base_url: "https://api.openai.com",
        }),
        "claude" => Some(PresetServiceConfig {
            name: "Claude",
            protocols: r#"["claude_messages"]"#,
            models: r#"["claude-3-opus-20240229", "claude-3-sonnet-20240229", "claude-3-haiku-20240307"]"#,
            api_base_url: "https://api.anthropic.com",
        }),
        "gemini" => Some(PresetServiceConfig {
            name: "Google Gemini",
            protocols: r#"["openai_chat"]"#,
            models: r#"["gemini-pro", "gemini-pro-vision"]"#,
            api_base_url: "https://generativelanguage.googleapis.com",
        }),
        "wenxin" => Some(PresetServiceConfig {
            name: "文心一言",
            protocols: r#"["openai_chat"]"#,
            models: r#"["ernie-bot-4", "ernie-bot"]"#,
            api_base_url: "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop",
        }),
        "tongyi" => Some(PresetServiceConfig {
            name: "通义千问",
            protocols: r#"["openai_chat"]"#,
            models: r#"["qwen-max", "qwen-turbo"]"#,
            api_base_url: "https://dashscope.aliyuncs.com/api/v1",
        }),
        "deepseek" => Some(PresetServiceConfig {
            name: "DeepSeek",
            protocols: r#"["openai_chat"]"#,
            models: r#"["deepseek-chat", "deepseek-coder"]"#,
            api_base_url: "https://api.deepseek.com",
        }),
        _ => None,
    }
}

/// 一键配置服务
pub struct QuickSetupService;

impl QuickSetupService {
    /// 执行快速配置
    pub async fn setup(request: QuickSetupRequest) -> Result<QuickSetupResponse, String> {
        let pool = crate::get_db_pool()
            .ok_or("数据库未初始化")?;

        let mut configured_services = Vec::new();

        for service_id in &request.services {
            if let Some(api_key) = request.api_keys.get(service_id) {
                if api_key.trim().is_empty() {
                    continue;
                }

                if let Some(config) = get_preset_config(service_id) {
                    let new_provider = crate::models::NewProvider {
                        name: config.name.to_string(),
                        protocols: serde_json::from_str(config.protocols).unwrap_or_default(),
                        models: serde_json::from_str(config.models).unwrap_or_default(),
                        keys: vec![crate::models::ProviderKeyEntry {
                            name: "主密钥".to_string(),
                            value: api_key.clone(),
                            enabled: true,
                            weight: 1,
                        }],
                        api_base_url: config.api_base_url.to_string(),
                        key_strategy: Some("round_robin".to_string()),
                        proxy_url: None,
                        timeout_seconds: Some(30),
                        max_retries: Some(3),
                        status: Some("enabled".to_string()),
                        health_status: None,
                        last_health_check_at: None,
                        metadata_json: None,
                        custom_headers: None,
                    };

                    crate::persistence::ProviderRepo::create(pool, &new_provider)
                        .await
                        .map_err(|e| format!("保存配置失败: {e}"))?;

                    configured_services.push(service_id.clone());
                }
            }
        }

        // 刷新LookupCache
        if let Some(pool) = crate::get_db_pool() {
            let cache = crate::load_lookup_cache(pool).await;
            // 这里需要AppState来更新缓存，但Tauri命令中无法直接访问
            // 实际上会在前端调用其他命令时自动刷新
            let _ = cache;
        }

        Ok(QuickSetupResponse {
            success: true,
            message: format!("成功配置 {} 个服务", configured_services.len()),
            configured_services,
        })
    }
}

/// Tauri命令：保存引导配置
#[tauri::command]
pub async fn save_onboarding_config(
    services: Vec<String>,
    api_keys: std::collections::HashMap<String, String>,
) -> Result<QuickSetupResponse, String> {
    let request = QuickSetupRequest {
        services,
        api_keys,
    };

    QuickSetupService::setup(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_preset_config() {
        let config = get_preset_config("openai");
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "OpenAI");

        let config = get_preset_config("claude");
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "Claude");

        let config = get_preset_config("unknown");
        assert!(config.is_none());
    }

    #[test]
    fn test_preset_config_content() {
        let config = get_preset_config("openai").unwrap();
        assert!(config.protocols.contains("openai_chat"));
        assert!(config.models.contains("gpt-4"));
        assert!(config.api_base_url.starts_with("https://"));

        let config = get_preset_config("claude").unwrap();
        assert!(config.protocols.contains("claude_messages"));
        assert!(config.models.contains("claude-3"));
        assert!(config.api_base_url.contains("anthropic"));
    }
}
