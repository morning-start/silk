use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// Hermes：写入 ~/.hermes/config.yaml 的 custom_providers 列表（按 name 匹配合并）。
/// 对齐 cc-switch hermes_config 的 custom_providers 列表结构。
pub struct HermesWriter;

#[async_trait]
impl super::HarnessWriter for HermesWriter {
    fn agent_type(&self) -> &'static str {
        "hermes"
    }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".hermes").join("config.yaml")
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Yaml
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

        // custom_providers 必须是列表（对齐 cc-switch：不存在则创建）
        if !live.get("custom_providers").is_some_and(|v| v.is_array()) {
            live["custom_providers"] = serde_json::json!([]);
        }

        // provider 条目 = settings 内容（注入 base_url/api_key 后），name 必须存在
        let provider_name = settings
            .get("_silk_provider_id")
            .and_then(|v| v.as_str())
            .unwrap_or("silk");
        let mut entry = settings.clone();
        if let Some(o) = entry.as_object_mut() {
            o.remove("_silk_provider_id");
            o.insert("name".to_string(), serde_json::json!(provider_name));
            o.insert("_silk_managed".to_string(), serde_json::json!(true));
        }

        // 按 name 匹配替换，不存在则追加
        let list = live
            .get_mut("custom_providers")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| "custom_providers 不是列表".to_string())?;
        if let Some(existing) = list
            .iter_mut()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(provider_name))
        {
            *existing = entry;
        } else {
            list.push(entry);
        }

        let text = serde_yaml::to_string(&live).map_err(|e| format!("YAML 序列化失败: {e}"))?;
        crate::application::config_writer::write_text_atomic(&path, &text)
            .map_err(|e| format!("写入 config.yaml 失败: {e}"))
    }
}
