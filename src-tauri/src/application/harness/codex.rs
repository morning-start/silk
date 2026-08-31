use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Codex：写入 ~/.codex/config.toml
/// 顶层 model_provider / model / wire_api + [model_providers.<id>] 表 + 保留 id 校验。
/// 对齐 cc-switch prepare_codex_provider_live_config / write_codex_live_atomic。
pub struct CodexWriter;

/// Codex 保留内置 provider id：覆盖会导致整份配置拒载（0.148+）
pub const RESERVED_PROVIDER_IDS: [&str; 2] = ["ollama", "lmstudio"];

#[async_trait]
impl super::HarnessWriter for CodexWriter {
    fn agent_type(&self) -> &'static str {
        "codex"
    }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".codex").join("config.toml")
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }

    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
    ) -> Result<(), String> {
        // 保留 id 校验（前端已拦截，后端双保险）
        if let Some(provider_id) = settings.get("model_provider").and_then(|v| v.as_str()) {
            if RESERVED_PROVIDER_IDS.contains(&provider_id) {
                return Err(format!(
                    "Codex 禁止覆盖内置 provider `{provider_id}`（0.148 起会拒绝加载整份配置）"
                ));
            }
        }

        let path = self.live_path(home);
        // TOML live：自读文本并 toml 解析（read_to_value_async 是 JSON 专用）
        let mut live: toml::Value = if path.exists() {
            let data = tokio::fs::read(&path)
                .await
                .map_err(|e| format!("读取 config.toml 失败: {e}"))?;
            let text = String::from_utf8(data).map_err(|e| format!("非 UTF-8: {e}"))?;
            if text.trim().is_empty() {
                toml::Value::Table(Default::default())
            } else {
                toml::from_str(&text).map_err(|e| format!("TOML 解析失败: {e}"))?
            }
        } else {
            toml::Value::Table(Default::default())
        };

        // 顶层 model_provider / model / wire_api 覆盖为 settings 值
        if let Some(t) = live.as_table_mut() {
            if let Some(model_provider) = settings.get("model_provider").and_then(|v| v.as_str()) {
                t.insert(
                    "model_provider".to_string(),
                    toml::Value::String(model_provider.to_string()),
                );
            }
            if let Some(model) = settings.get("model").and_then(|v| v.as_str()) {
                t.insert("model".to_string(), toml::Value::String(model.to_string()));
            }
            if let Some(wire_api) = settings.get("wire_api").and_then(|v| v.as_str()) {
                t.insert("wire_api".to_string(), toml::Value::String(wire_api.to_string()));
            }
            // 注入网关 base_url/api_key（顶层，CodexWriter 投影到 provider 表）
            if let Some(base_url) = settings.get("base_url").and_then(|v| v.as_str()) {
                t.insert(
                    "base_url".to_string(),
                    toml::Value::String(base_url.to_string()),
                );
            }
            if let Some(api_key) = settings.get("api_key").and_then(|v| v.as_str()) {
                t.insert("api_key".to_string(), toml::Value::String(api_key.to_string()));
            }
            // 接管标记（TOML 注释方式不可行，用顶层键标记）
            t.insert("_silk_managed".to_string(), toml::Value::Boolean(true));
        }

        let text = toml::to_string_pretty(&live).map_err(|e| format!("TOML 序列化失败: {e}"))?;
        crate::application::config_writer::write_text_atomic(&path, &text)
            .map_err(|e| format!("写入 config.toml 失败: {e}"))
    }
}
