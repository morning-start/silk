//! 自动检测已安装的AI应用
//!
//! 检测本地已安装的AI客户端，用于引导用户快速配置。

use serde::{Deserialize, Serialize};

/// 已安装的AI应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAiApp {
    /// 应用名称
    pub name: String,
    /// 应用描述
    pub description: String,
    /// 是否已安装
    pub installed: bool,
    /// 配置文件路径
    pub config_path: Option<String>,
    /// 应用图标（用于前端展示）
    pub icon: String,
    /// 应用颜色（用于前端展示）
    pub color: String,
}

/// AI应用检测服务
pub struct AiAppDetector;

impl AiAppDetector {
    /// 检测所有支持的AI应用
    pub fn detect_all() -> Vec<InstalledAiApp> {
        vec![
            Self::detect_openai(),
            Self::detect_claude(),
            Self::detect_gemini(),
        ]
    }

    /// 检测OpenAI应用
    fn detect_openai() -> InstalledAiApp {
        let config_paths = vec![
            // Windows
            dirs::config_dir().map(|p| p.join("OpenAI").join("config.json")),
            // macOS / Linux
            dirs::home_dir().map(|p| p.join(".openai").join("config.json")),
        ];

        let installed = config_paths.iter().any(|p| {
            p.as_ref().map_or(false, |path| path.exists())
        });

        let config_path = config_paths
            .into_iter()
            .find(|p| p.as_ref().map_or(false, |path| path.exists()))
            .flatten()
            .map(|p| p.to_string_lossy().to_string());

        InstalledAiApp {
            name: "OpenAI (ChatGPT)".to_string(),
            description: "访问GPT-4、GPT-3.5等模型".to_string(),
            installed,
            config_path,
            icon: "logo-openai".to_string(),
            color: "#10a37f".to_string(),
        }
    }

    /// 检测Claude应用
    fn detect_claude() -> InstalledAiApp {
        let config_paths = vec![
            // Windows
            dirs::config_dir().map(|p| p.join("Claude").join("config.json")),
            // macOS
            dirs::home_dir().map(|p| p.join("Library").join("Application Support").join("Claude").join("config.json")),
        ];

        let installed = config_paths.iter().any(|p| {
            p.as_ref().map_or(false, |path| path.exists())
        });

        let config_path = config_paths
            .into_iter()
            .find(|p| p.as_ref().map_or(false, |path| path.exists()))
            .flatten()
            .map(|p| p.to_string_lossy().to_string());

        InstalledAiApp {
            name: "Claude".to_string(),
            description: "访问Claude 3 Opus、Sonnet等模型".to_string(),
            installed,
            config_path,
            icon: "chatbox-outline".to_string(),
            color: "#d97706".to_string(),
        }
    }

    /// 检测Gemini应用
    fn detect_gemini() -> InstalledAiApp {
        let config_paths = vec![
            // Windows
            dirs::config_dir().map(|p| p.join("Google").join("Gemini").join("config.json")),
        ];

        let installed = config_paths.iter().any(|p| {
            p.as_ref().map_or(false, |path| path.exists())
        });

        let config_path = config_paths
            .into_iter()
            .find(|p| p.as_ref().map_or(false, |path| path.exists()))
            .flatten()
            .map(|p| p.to_string_lossy().to_string());

        InstalledAiApp {
            name: "Google Gemini".to_string(),
            description: "访问Gemini Pro等模型".to_string(),
            installed,
            config_path,
            icon: "logo-google".to_string(),
            color: "#4285f4".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_all() {
        let apps = AiAppDetector::detect_all();
        assert_eq!(apps.len(), 3);

        // 验证每个应用都有必要的字段
        for app in apps {
            assert!(!app.name.is_empty());
            assert!(!app.description.is_empty());
            assert!(!app.icon.is_empty());
            assert!(!app.color.is_empty());
        }
    }

    #[test]
    fn test_detect_openai() {
        let app = AiAppDetector::detect_openai();
        assert_eq!(app.name, "OpenAI (ChatGPT)");
        assert_eq!(app.icon, "logo-openai");
        assert_eq!(app.color, "#10a37f");
    }

    #[test]
    fn test_detect_claude() {
        let app = AiAppDetector::detect_claude();
        assert_eq!(app.name, "Claude");
        assert_eq!(app.icon, "chatbox-outline");
        assert_eq!(app.color, "#d97706");
    }

    #[test]
    fn test_detect_gemini() {
        let app = AiAppDetector::detect_gemini();
        assert_eq!(app.name, "Google Gemini");
        assert_eq!(app.icon, "logo-google");
        assert_eq!(app.color, "#4285f4");
    }
}
