use std::path::Path;

use crate::application::config_writer::ConfigFormat;
use async_trait::async_trait;

/// OpenCode：写入 opencode.json 的 provider.<id> 段。
pub struct OpenCodeWriter;

#[async_trait]
impl super::HarnessWriter for OpenCodeWriter {
    fn agent_type(&self) -> &'static str { "opencode" }

    fn live_path(&self, home: &Path) -> std::path::PathBuf {
        home.join(".config").join("opencode").join("opencode.json")
    }

    fn config_format(&self) -> ConfigFormat { ConfigFormat::Json }

    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
        _remove_keys: &[String],
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let mut live = match crate::application::config_writer::read_to_value_async(&path).await? {
            Some(value) => value,
            None => serde_json::json!({}),
        };
        let provider_id = settings
            .get("_silk_provider_id")
            .or_else(|| settings.get("id"))
            .or_else(|| settings.get("name"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("silk");
        let providers = live
            .as_object_mut()
            .ok_or_else(|| "live 配置不是 JSON 对象".to_string())?
            .entry("provider".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let providers = providers.as_object_mut().ok_or_else(|| "provider 不是 JSON 对象".to_string())?;
        let mut entry = settings.clone();
        if let Some(object) = entry.as_object_mut() {
            object.remove("_silk_provider_id");
            object.insert("_silk_managed".to_string(), serde_json::json!(true));
        }
        providers.insert(provider_id.to_string(), entry);
        crate::application::config_writer::write_to_path_atomic(&path, &live)
            .map_err(|error| format!("写入 opencode.json 失败: {error}"))
    }

    /// 取消激活：从 opencode.json 删除本 preset 对应的 provider.<id> 段（快照回滚）。
    /// opencode 为累加模式，停用一个 provider 不影响其他已激活 provider。
    async fn remove_from_live(
        &self,
        home: &Path,
        settings: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let snapshot =
            crate::application::config_writer::LiveSnapshot::take(&path)
                .map_err(|error| format!("备份失败: {error}"))?;
        let result: Result<(), String> = async {
            let mut live =
                match crate::application::config_writer::read_to_value_async(&path).await? {
                    Some(value) => value,
                    None => return Ok(()), // 无 live 文件，无需移除
                };
            let provider_id = settings
                .get("_silk_provider_id")
                .or_else(|| settings.get("id"))
                .or_else(|| settings.get("name"))
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("silk");
            let removed = match live
                .as_object_mut()
                .ok_or_else(|| "live 配置不是 JSON 对象".to_string())?
                .get_mut("provider")
                .and_then(|value| value.as_object_mut())
            {
                Some(providers) => providers.remove(provider_id).is_some(),
                None => false,
            };
            if removed {
                crate::application::config_writer::write_to_path_atomic(&path, &live)
                    .map_err(|error| format!("写入 opencode.json 失败: {error}"))?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = &result {
            let _ = snapshot.restore();
            return Err(format!("移除失败，已恢复快照: {error}"));
        }
        result
    }

    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let Some(providers) = live.get("provider").and_then(|value| value.as_object()) else { return Vec::new() };
        providers.iter().filter_map(|(id, value)| {
            let mut entry = value.as_object()?.clone();
            entry.remove("_silk_managed");
            entry.insert("id".to_string(), serde_json::Value::String(id.clone()));
            Some(serde_json::Value::Object(entry))
        }).collect()
    }

    fn extract_settings(&self, live: &serde_json::Value, stored: &serde_json::Value) -> Option<serde_json::Value> {
        let providers = live.get("provider")?.as_object()?;
        let provider_id = stored
            .get("_silk_provider_id")
            .or_else(|| stored.get("id"))
            .or_else(|| stored.get("name"))
            .and_then(|value| value.as_str())?;
        let mut entry = providers.get(provider_id).cloned()?;
        if let Some(object) = entry.as_object_mut() {
            object.insert("id".to_string(), serde_json::Value::String(provider_id.to_string()));
        }
        Some(entry)
    }
}
