use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Claude Code：写入 ~/.claude/settings.json 的 env 子对象
/// （ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN）+ `_silk_managed` 标记。
/// 对齐 cc-switch live.rs sanitize_claude_settings_for_live + 原子写。
pub struct ClaudeCodeWriter;

#[async_trait]
impl super::HarnessWriter for ClaudeCodeWriter {
    fn agent_type(&self) -> &'static str {
        "claude_code"
    }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".claude").join("settings.json")
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

        // env 子对象：注入注入网关字段（仅添加缺失项，不覆盖用户已有键）
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
            // 顶层兜底注入（如 base_url/api_key 直接放 settings 顶层时）
            for key in ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"] {
                if let Some(v) = settings.get(key) {
                    e.entry(key.to_string()).or_insert_with(|| v.clone());
                }
            }
        }
        // 接管标记
        if let Some(o) = live.as_object_mut() {
            o.insert("_silk_managed".to_string(), serde_json::json!(true));
        }

        crate::application::config_writer::write_to_path_atomic(&path, &live)
            .map_err(|e| format!("写入 settings.json 失败: {e}"))
    }
}
