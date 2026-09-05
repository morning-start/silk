use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Codex：写入 ~/.codex/config.toml。
pub struct CodexWriter;

pub const RESERVED_PROVIDER_IDS: [&str; 2] = ["ollama", "lmstudio"];

#[async_trait]
impl super::HarnessWriter for CodexWriter {
    fn agent_type(&self) -> &'static str { "codex" }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".codex").join("config.toml")
    }

    fn config_format(&self) -> ConfigFormat { ConfigFormat::Toml }

    async fn write_live_inner(&self, home: &Path, settings: &serde_json::Value, _remove_keys: &[String]) -> Result<(), String> {
        let provider_id = settings.get("model_provider").and_then(serde_json::Value::as_str).filter(|value| !value.trim().is_empty()).unwrap_or("custom");
        if RESERVED_PROVIDER_IDS.contains(&provider_id) {
            return Err(format!("Codex 禁止覆盖内置 provider `{provider_id}`（0.148 起会拒绝加载整份配置）"));
        }
        let path = self.live_path(home);
        let mut live: toml::Value = if path.exists() {
            let data = tokio::fs::read(&path).await.map_err(|error| format!("读取 config.toml 失败: {error}"))?;
            let text = String::from_utf8(data).map_err(|error| format!("非 UTF-8: {error}"))?;
            if text.trim().is_empty() { toml::Value::Table(Default::default()) } else { toml::from_str(&text).map_err(|error| format!("TOML 解析失败: {error}"))? }
        } else { toml::Value::Table(Default::default()) };
        let root = live.as_table_mut().ok_or_else(|| "config.toml 顶层不是表".to_string())?;
        root.insert("model_provider".to_string(), toml::Value::String(provider_id.to_string()));
        for key in ["model", "model_reasoning_effort", "model_context_window", "model_auto_compact_token_limit"] {
            if let Some(value) = settings.get(key) { root.insert(key.to_string(), serde_json::from_value(value.clone()).map_err(|error| format!("{key} 不是 TOML 值: {error}"))?); }
        }
        let providers = root.entry("model_providers".to_string()).or_insert_with(|| toml::Value::Table(Default::default())).as_table_mut().ok_or_else(|| "model_providers 不是表".to_string())?;
        let provider = providers.entry(provider_id.to_string()).or_insert_with(|| toml::Value::Table(Default::default())).as_table_mut().ok_or_else(|| format!("model_providers.{provider_id} 不是表"))?;
        provider.insert("name".to_string(), toml::Value::String(provider_id.to_string()));
        for key in ["base_url", "wire_api", "http_headers"] {
            if let Some(value) = settings.get(key) { provider.insert(key.to_string(), serde_json::from_value(value.clone()).map_err(|error| format!("{key} 不是 TOML 值: {error}"))?); }
        }
        if let Some(api_key) = settings.get("api_key").and_then(serde_json::Value::as_str) { provider.insert("experimental_bearer_token".to_string(), toml::Value::String(api_key.to_string())); }
        root.insert("_silk_managed".to_string(), toml::Value::Boolean(true));
        let text = toml::to_string_pretty(&live).map_err(|error| format!("TOML 序列化失败: {error}"))?;
        crate::application::config_writer::write_text_atomic(&path, &text).map_err(|error| format!("写入 config.toml 失败: {error}"))
    }

    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let Some(provider_id) = live.get("model_provider").and_then(|value| value.as_str()) else { return Vec::new() };
        self.extract_settings(live, &serde_json::json!({ "model_provider": provider_id })).into_iter().collect()
    }

    fn extract_settings(&self, live: &serde_json::Value, stored: &serde_json::Value) -> Option<serde_json::Value> {
        let provider_id = live.get("model_provider").and_then(|value| value.as_str())?;
        let provider = live.get("model_providers")?.get(provider_id)?.as_object()?;
        let mut result = stored.as_object().cloned().unwrap_or_default();
        for key in ["base_url", "wire_api", "http_headers"] { if let Some(value) = provider.get(key) { result.insert(key.to_string(), value.clone()); } }
        if let Some(value) = provider.get("experimental_bearer_token") { result.insert("api_key".to_string(), value.clone()); }
        for key in ["model_provider", "model", "model_reasoning_effort", "model_context_window", "model_auto_compact_token_limit"] { if let Some(value) = live.get(key) { result.insert(key.to_string(), value.clone()); } }
        Some(serde_json::Value::Object(result))
    }
}
