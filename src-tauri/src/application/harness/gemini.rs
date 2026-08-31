use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Gemini CLI：写入 ~/.gemini/settings.json 的 env 子对象
/// （GOOGLE_GEMINI_BASE_URL / GEMINI_API_KEY）。
/// 对齐 cc-switch write_gemini_env_atomic（env 子对象而非 .env 文件）。
pub struct GeminiWriter;

#[async_trait]
impl super::HarnessWriter for GeminiWriter {
    fn agent_type(&self) -> &'static str {
        "gemini_cli"
    }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".gemini").join("settings.json")
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let mut live = match crate::application::config_writer::read_to_value_async(&path).await? {
            Some(v) => v,
            None => serde_json::json!({}),
        };

        // env 子对象：注入网关字段（仅添加缺失项）+ 用户自定义 env 键
        let env = live
            .as_object_mut()
            .ok_or_else(|| "live 配置不是 JSON 对象".to_string())?
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(e) = env.as_object_mut() {
            if let Some(s) = settings.get("env").and_then(|v| v.as_object()) {
                for (k, v) in s {
                    e.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            for key in ["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY"] {
                if let Some(v) = settings.get(key) {
                    e.entry(key.to_string()).or_insert_with(|| v.clone());
                }
            }
        }
        if let Some(o) = live.as_object_mut() {
            o.insert("_silk_managed".to_string(), serde_json::json!(true));
        }

        crate::application::config_writer::write_to_path_atomic(&path, &live)
            .map_err(|e| format!("写入 settings.json 失败: {e}"))
    }
}
