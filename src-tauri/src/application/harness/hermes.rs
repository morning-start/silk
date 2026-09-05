use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

pub struct HermesWriter;

#[async_trait]
impl super::HarnessWriter for HermesWriter {
    fn agent_type(&self) -> &'static str { "hermes" }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".hermes").join("config.yaml")
    }

    fn config_format(&self) -> ConfigFormat { ConfigFormat::Yaml }

    async fn write_live_inner(&self, home: &Path, settings: &serde_json::Value, _remove_keys: &[String]) -> Result<(), String> {
        let path = self.live_path(home);
        let mut live: serde_json::Value = if path.exists() {
            let data = tokio::fs::read(&path).await.map_err(|error| format!("读取 config.yaml 失败: {error}"))?;
            let text = String::from_utf8(data).map_err(|error| format!("非 UTF-8: {error}"))?;
            if text.trim().is_empty() { serde_json::json!({}) } else { serde_yaml::from_str(&text).map_err(|error| format!("YAML 解析失败: {error}"))? }
        } else { serde_json::json!({}) };
        let root = live.as_object_mut().ok_or_else(|| "live 配置不是 YAML 对象".to_string())?;
        let providers = root.entry("custom_providers".to_string()).or_insert_with(|| serde_json::json!([])).as_array_mut().ok_or_else(|| "custom_providers 不是列表".to_string())?;
        let provider_name = settings.get("_silk_provider_id").or_else(|| settings.get("name")).and_then(|value| value.as_str()).filter(|value| !value.trim().is_empty()).unwrap_or("silk");
        let mut entry = settings.clone();
        if let Some(object) = entry.as_object_mut() {
            object.remove("_silk_provider_id");
            if let Some(models) = object.remove("models").and_then(|value| value.as_array().cloned()) {
                let mut model_map = serde_json::Map::new();
                for model in models {
                    let Some(mut model) = model.as_object().cloned() else { continue };
                    let Some(id) = model.remove("id").and_then(|value| value.as_str().map(str::trim).filter(|id| !id.is_empty()).map(ToOwned::to_owned)) else { continue };
                    model.remove("name");
                    model_map.insert(id, serde_json::Value::Object(model));
                }
                object.insert("models".to_string(), serde_json::Value::Object(model_map));
            }
            object.insert("name".to_string(), serde_json::json!(provider_name));
            object.insert("_silk_managed".to_string(), serde_json::json!(true));
        }
        if let Some(existing) = providers.iter_mut().find(|item| item.get("name").and_then(|value| value.as_str()) == Some(provider_name)) { *existing = entry; } else { providers.push(entry); }
        let text = serde_yaml::to_string(&live).map_err(|error| format!("YAML 序列化失败: {error}"))?;
        crate::application::config_writer::write_text_atomic(&path, &text).map_err(|error| format!("写入 config.yaml 失败: {error}"))
    }

    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let Some(providers) = live.get("custom_providers").and_then(|value| value.as_array()) else { return Vec::new() };
        providers.iter().filter_map(|value| {
            let mut entry = value.as_object()?.clone();
            let name = entry.get("name")?.as_str()?.trim();
            if name.is_empty() { return None; }
            entry.remove("_silk_managed");
            if let Some(models) = entry.get_mut("models").and_then(|value| value.as_object_mut()) {
                let array = models.iter().map(|(id, value)| {
                    let mut model = value.as_object().cloned().unwrap_or_default();
                    model.insert("id".to_string(), serde_json::Value::String(id.clone()));
                    serde_json::Value::Object(model)
                }).collect::<Vec<_>>();
                entry.insert("models".to_string(), serde_json::Value::Array(array));
            }
            Some(serde_json::Value::Object(entry))
        }).collect()
    }

    fn extract_settings(&self, live: &serde_json::Value, stored: &serde_json::Value) -> Option<serde_json::Value> {
        let provider_name = stored.get("_silk_provider_id").or_else(|| stored.get("name")).and_then(|value| value.as_str())?;
        let providers = live.get("custom_providers")?.as_array()?;
        let mut entry = providers.iter().find(|item| item.get("name").and_then(|value| value.as_str()) == Some(provider_name)).cloned()?;
        if let Some(object) = entry.as_object_mut() {
            object.remove("_silk_managed");
            if let Some(models) = object.get_mut("models").and_then(|value| value.as_object_mut()) {
                let array = models.iter().map(|(id, value)| { let mut model = value.as_object().cloned().unwrap_or_default(); model.insert("id".to_string(), serde_json::Value::String(id.clone())); serde_json::Value::Object(model) }).collect::<Vec<_>>();
                object.insert("models".to_string(), serde_json::Value::Array(array));
            }
        }
        Some(entry)
    }
}
