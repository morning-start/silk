//! 预置AI服务配置
//!
//! 提供常用AI服务的预置配置，用于快速添加服务。

use serde::{Deserialize, Serialize};

/// 预置模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetModel {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// 预置AI服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetProvider {
    pub id: String,
    pub name: String,
    pub description: String,
    pub protocols: Vec<String>,
    pub models: Vec<PresetModel>,
    pub api_base_url: String,
    pub api_key_url: String,
    pub api_key_placeholder: String,
    pub color: String,
}

/// 预置配置文件结构
#[derive(Debug, Deserialize)]
struct PresetConfig {
    #[allow(dead_code)]
    version: String,
    providers: Vec<PresetProvider>,
}

/// 预置配置服务
pub struct PresetProviderService;

impl PresetProviderService {
    /// 获取所有预置配置
    pub fn get_all() -> Vec<PresetProvider> {
        let data = include_str!("../data/preset_providers.json");
        serde_json::from_str::<PresetConfig>(data)
            .map(|config| config.providers)
            .unwrap_or_default()
    }

    /// 根据ID获取预置配置
    pub fn get_by_id(id: &str) -> Option<PresetProvider> {
        Self::get_all().into_iter().find(|p| p.id == id)
    }

    /// 获取所有预置配置的ID列表
    pub fn get_all_ids() -> Vec<String> {
        Self::get_all().iter().map(|p| p.id.clone()).collect()
    }
}

/// Tauri命令：获取所有预置配置
#[tauri::command]
pub async fn get_preset_providers() -> Result<Vec<PresetProvider>, String> {
    Ok(PresetProviderService::get_all())
}

/// Tauri命令：根据ID获取预置配置
#[tauri::command]
pub async fn get_preset_provider_by_id(id: String) -> Result<Option<PresetProvider>, String> {
    Ok(PresetProviderService::get_by_id(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all() {
        let providers = PresetProviderService::get_all();
        assert_eq!(providers.len(), 6);

        // 验证每个Provider都有必要的字段
        for provider in &providers {
            assert!(!provider.id.is_empty());
            assert!(!provider.name.is_empty());
            assert!(!provider.api_base_url.is_empty());
            assert!(!provider.api_key_url.is_empty());
            assert!(!provider.color.is_empty());
        }
    }

    #[test]
    fn test_get_by_id() {
        let provider = PresetProviderService::get_by_id("openai");
        assert!(provider.is_some());

        let provider = provider.unwrap();
        assert_eq!(provider.id, "openai");
        assert_eq!(provider.name, "OpenAI");
        assert!(provider.api_base_url.contains("openai"));
    }

    #[test]
    fn test_get_by_id_claude() {
        let provider = PresetProviderService::get_by_id("claude");
        assert!(provider.is_some());

        let provider = provider.unwrap();
        assert_eq!(provider.id, "claude");
        assert_eq!(provider.name, "Claude");
        assert!(provider.api_base_url.contains("anthropic"));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let provider = PresetProviderService::get_by_id("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_get_all_ids() {
        let ids = PresetProviderService::get_all_ids();
        assert_eq!(ids.len(), 6);
        assert!(ids.contains(&"openai".to_string()));
        assert!(ids.contains(&"claude".to_string()));
        assert!(ids.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_preset_provider_content() {
        let provider = PresetProviderService::get_by_id("openai").unwrap();
        assert!(provider.protocols.contains(&"openai".to_string()));
        assert!(!provider.models.is_empty());
        assert!(provider.api_key_placeholder.contains("sk-"));
    }
}
