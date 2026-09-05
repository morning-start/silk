use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Claude Code：写入 ~/.claude/settings.json 的 env 子对象。
pub struct ClaudeCodeWriter;

#[async_trait]
impl super::HarnessWriter for ClaudeCodeWriter {
    fn agent_type(&self) -> &'static str { "claude_code" }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".claude").join("settings.json")
    }

    fn config_format(&self) -> ConfigFormat { ConfigFormat::Json }

    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
        remove_keys: &[String],
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let mut live = match crate::application::config_writer::read_to_value_async(&path).await? {
            Some(value) => value,
            None => serde_json::json!({}),
        };
        let env = live
            .as_object_mut()
            .ok_or_else(|| "live 配置不是 JSON 对象".to_string())?
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let env = env.as_object_mut().ok_or_else(|| "env 不是 JSON 对象".to_string())?;
        for key in remove_keys { env.remove(key); }
        if let Some(settings_env) = settings.get("env").and_then(|value| value.as_object()) {
            for (key, value) in settings_env { env.insert(key.clone(), value.clone()); }
        }
        for key in ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"] {
            if let Some(value) = settings.get(key) { env.insert(key.to_string(), value.clone()); }
        }
        live.as_object_mut().expect("上面已校验根对象").insert("_silk_managed".to_string(), serde_json::json!(true));
        crate::application::config_writer::write_to_path_atomic(&path, &live)
            .map_err(|error| format!("写入 settings.json 失败: {error}"))
    }

    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let mut settings = live.clone();
        if let Some(object) = settings.as_object_mut() { object.remove("_silk_managed"); }
        vec![settings]
    }

    fn extract_settings(&self, live: &serde_json::Value, stored: &serde_json::Value) -> Option<serde_json::Value> {
        let live_env = live.get("env")?.as_object()?;
        let stored_env = stored.get("env").and_then(|value| value.as_object());
        let mut env = serde_json::Map::new();
        for key in stored_env.into_iter().flat_map(|values| values.keys()) {
            if let Some(value) = live_env.get(key) { env.insert(key.clone(), value.clone()); }
        }
        Some(serde_json::json!({ "env": env }))
    }
}
