use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// OpenCode：写入 ~/.config/opencode/opencode.json 的 provider.<id> 段（累加模式）。
/// 对齐 cc-switch opencode_config::set_provider。
pub struct OpenCodeWriter;

#[async_trait]
impl super::HarnessWriter for OpenCodeWriter {
    fn agent_type(&self) -> &'static str {
        "opencode"
    }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".config").join("opencode").join("opencode.json")
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

        // provider 段必须是对象（对齐 cc-switch：非对象重置为空）
        if !live.get("provider").is_some_and(|v| v.is_object()) {
            if live.get("provider").is_some() {
                tracing::warn!("[harness:opencode] provider 不是对象，已重置为空对象");
            }
            live["provider"] = serde_json::json!({});
        }

        // provider 条目 = settings 内容（注入 base_url/api_key 后）
        let provider_id = settings
            .get("_silk_provider_id")
            .and_then(|v| v.as_str())
            .unwrap_or("silk");
        let mut entry = settings.clone();
        if let Some(o) = entry.as_object_mut() {
            o.remove("_silk_provider_id");
            o.insert("_silk_managed".to_string(), serde_json::json!(true));
        }

        if let Some(providers) = live.get_mut("provider").and_then(|v| v.as_object_mut()) {
            providers.insert(provider_id.to_string(), entry);
        }

        crate::application::config_writer::write_to_path_atomic(&path, &live)
            .map_err(|e| format!("写入 opencode.json 失败: {e}"))
    }
}
