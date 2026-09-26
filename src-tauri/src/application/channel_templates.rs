//! 渠道模板（内置目录）
//!
//! 与数据库中的「渠道」（`models::Provider`，用户实际配置的转发目标）严格区分：
//! 本模块是**静态模板** —— 内置 JSON 目录，用于在「添加渠道」表单里一键填充
//! 官方端点的 base_url / 协议 / 模型列表 / 申请 Key 的页面地址。
//!
//! 命名说明：早期叫 `PresetProvider`，与 `Provider`（DB 渠道）撞名，
//! 读者常误以为这里是「预置的可用渠道」；实际语义是「渠道的填写模板」，故更名。

use serde::{Deserialize, Serialize};

/// 模板中的预置模型项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTemplateModel {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// 渠道模板（对应 `data/channel_templates.json` 的一个条目）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub protocols: Vec<String>,
    pub models: Vec<ChannelTemplateModel>,
    pub api_base_url: String,
    pub api_key_url: String,
    pub api_key_placeholder: String,
    pub color: String,
}

/// 模板文件结构
#[derive(Debug, Deserialize)]
struct TemplateFile {
    #[allow(dead_code)]
    version: String,
    providers: Vec<ChannelTemplate>,
}

/// 渠道模板服务（只读，数据随二进制内嵌）
pub struct ChannelTemplateService;

impl ChannelTemplateService {
    /// 获取全部模板
    pub fn get_all() -> Vec<ChannelTemplate> {
        let data = include_str!("../data/channel_templates.json");
        serde_json::from_str::<TemplateFile>(data)
            .map(|file| file.providers)
            .unwrap_or_default()
    }

    /// 根据 ID 获取单个模板
    pub fn get_by_id(id: &str) -> Option<ChannelTemplate> {
        Self::get_all().into_iter().find(|t| t.id == id)
    }

    /// 获取所有模板的 ID 列表
    pub fn get_all_ids() -> Vec<String> {
        Self::get_all().iter().map(|t| t.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all() {
        let templates = ChannelTemplateService::get_all();
        assert_eq!(templates.len(), 6);

        // 验证每个模板都有必要的字段
        for template in &templates {
            assert!(!template.id.is_empty());
            assert!(!template.name.is_empty());
            assert!(!template.api_base_url.is_empty());
            assert!(!template.api_key_url.is_empty());
            assert!(!template.color.is_empty());
        }
    }

    #[test]
    fn test_get_by_id() {
        let template = ChannelTemplateService::get_by_id("openai");
        assert!(template.is_some());

        let template = template.unwrap();
        assert_eq!(template.id, "openai");
        assert_eq!(template.name, "OpenAI");
        assert!(template.api_base_url.contains("openai"));
    }

    #[test]
    fn test_get_by_id_claude() {
        let template = ChannelTemplateService::get_by_id("claude");
        assert!(template.is_some());

        let template = template.unwrap();
        assert_eq!(template.id, "claude");
        assert_eq!(template.name, "Claude");
        assert!(template.api_base_url.contains("anthropic"));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let template = ChannelTemplateService::get_by_id("nonexistent");
        assert!(template.is_none());
    }

    #[test]
    fn test_get_all_ids() {
        let ids = ChannelTemplateService::get_all_ids();
        assert_eq!(ids.len(), 6);
        assert!(ids.contains(&"openai".to_string()));
        assert!(ids.contains(&"claude".to_string()));
        assert!(ids.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_channel_template_content() {
        let template = ChannelTemplateService::get_by_id("openai").unwrap();
        assert!(template.protocols.contains(&"openai".to_string()));
        assert!(!template.models.is_empty());
        assert!(template.api_key_placeholder.contains("sk-"));
    }
}
