//! Harness 配置写入器（对齐 cc-switch 各 config 模块）
//!
//! 每个 agent 一个 writer，负责把 Preset 的 settings_config 投影写入
//! 对应的 live 配置文件（原子写 + 快照回滚 + 接管标记 `_silk_managed`）。

use std::path::{Path, PathBuf};

use async_trait::async_trait;

pub mod claude_code;
pub mod codex;
pub mod gemini;
pub mod hermes;
pub mod opencode;

use super::config_writer::{ConfigFormat, LiveSnapshot};

/// Agent 配置写入器 trait（对齐 cc-switch 的 apply_provider / write_*_live）
#[async_trait]
pub trait HarnessWriter: Send + Sync {
    fn agent_type(&self) -> &'static str;

    fn live_path(&self, home: &Path) -> PathBuf;

    fn config_format(&self) -> ConfigFormat;

    async fn read_live(&self, home: &Path) -> Result<Option<Vec<u8>>, String> {
        let path = self.live_path(home);
        if !path.exists() {
            return Ok(None);
        }
        tokio::fs::read(&path).await.map(Some).map_err(|e| e.to_string())
    }

    /// 写入 live 配置：备份 → 合并 → 原子写；失败恢复快照。
    ///
    /// 语义（对齐 cc-switch 切换：新预设控制键**覆盖** live，旧预设独有键**剥离**）：
    /// - `settings` 中声明的键 → 覆盖 live 中的同名键（切换预设必须真正换模型/换端点）
    /// - `remove_keys` 中的键 → 从 live 中移除（上一预设声明、本预设不再声明的残留键）
    /// - 其余键（用户手动配置、与预设无关的字段）→ 保留不动
    async fn write_live(
        &self,
        home: &Path,
        settings: &serde_json::Value,
        remove_keys: &[String],
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let snapshot = LiveSnapshot::take(&path).map_err(|e| format!("备份失败: {e}"))?;

        let result = self.write_live_inner(home, settings, remove_keys).await;
        if let Err(e) = &result {
            let _ = snapshot.restore();
            return Err(format!("写入失败，已恢复快照: {e}"));
        }
        Ok(())
    }

    /// 实际写入逻辑（由各 harness 实现）
    async fn write_live_inner(
        &self,
        home: &Path,
        settings: &serde_json::Value,
        remove_keys: &[String],
    ) -> Result<(), String>;

    /// 从现有 live 配置提取可导入的 preset 片段。
    /// 一个 live 文件可能包含多个 provider，因此返回多个配置。
    fn extract_startup_settings(&self, live: &serde_json::Value) -> Vec<serde_json::Value> {
        let _ = live;
        Vec::new()
    }

    /// 从 live 中提取当前预设拥有的配置片段，供切换前回填 DB。
    /// 默认不回填，避免把整份 live 文件误存进 preset。
    fn extract_settings(
        &self,
        live: &serde_json::Value,
        stored: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let _ = (live, stored);
        None
    }
    /// live 文件是否被 silk 接管（存在 `_silk_managed` 标记）
    async fn is_managed(&self, home: &Path) -> Result<bool, String> {
        let Some(data) = self.read_live(home).await? else {
            return Ok(false);
        };
        let text = String::from_utf8(data).map_err(|e| format!("live 配置非 UTF-8: {e}"))?;
        Ok(text.contains("_silk_managed"))
    }
}

/// 按 agent_type 获取 writer（对齐 cc-switch writer_for）
pub fn writer_for(agent_type: &str) -> Option<Box<dyn HarnessWriter>> {
    match agent_type {
        "claude_code" => Some(Box::new(claude_code::ClaudeCodeWriter)),
        "codex" => Some(Box::new(codex::CodexWriter)),
        "opencode" => Some(Box::new(opencode::OpenCodeWriter)),
        "hermes" => Some(Box::new(hermes::HermesWriter)),
        "gemini_cli" => Some(Box::new(gemini::GeminiWriter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_exposes_all_supported_harnesses() {
        for agent_type in ["claude_code", "codex", "opencode", "hermes", "gemini_cli"] {
            assert!(writer_for(agent_type).is_some(), "missing writer: {agent_type}");
        }
    }

    #[test]
    fn claude_extraction_does_not_import_unowned_live_env() {
        let writer = claude_code::ClaudeCodeWriter;
        let live = json!({"env": {"ANTHROPIC_MODEL": "new", "UNRELATED": "keep"}});
        let stored = json!({"env": {"ANTHROPIC_MODEL": "old"}});
        assert_eq!(writer.extract_settings(&live, &stored), Some(json!({"env": {"ANTHROPIC_MODEL": "new"}})));
    }

    #[test]
    fn opencode_extraction_selects_only_stored_provider() {
        let writer = opencode::OpenCodeWriter;
        let live = json!({"provider": {"relay": {"npm": "relay"}, "other": {"npm": "other"}}});
        let stored = json!({"id": "relay"});
        assert_eq!(writer.extract_settings(&live, &stored), Some(json!({"id": "relay", "npm": "relay"})));
    }

    #[test]
    fn hermes_extraction_restores_model_ids() {
        let writer = hermes::HermesWriter;
        let live = json!({"custom_providers": [{"name": "relay", "models": {"model-a": {"context_length": 100}}}, {"name": "other"}]});
        let stored = json!({"name": "relay"});
        assert_eq!(writer.extract_settings(&live, &stored), Some(json!({"name": "relay", "models": [{"context_length": 100, "id": "model-a"}]})));
    }

    #[test]
    fn codex_extraction_reads_active_provider_only() {
        let writer = codex::CodexWriter;
        let live = json!({
            "model_provider": "relay",
            "model": "model-a",
            "model_providers": {
                "relay": {"base_url": "https://relay.example/v1", "experimental_bearer_token": "key"},
                "other": {"base_url": "https://other.example/v1"}
            }
        });
        let stored = json!({"model_provider": "relay"});
        let extracted = writer.extract_settings(&live, &stored).expect("active provider");
        assert_eq!(extracted["base_url"], "https://relay.example/v1");
        assert_eq!(extracted["api_key"], "key");
        assert_eq!(extracted["model"], "model-a");
        assert!(extracted.get("other").is_none());
    }

    #[test]
    fn opencode_startup_extraction_imports_all_provider_entries() {
        let writer = opencode::OpenCodeWriter;
        let live = json!({
            "provider": {
                "relay": {"npm": "relay", "options": {"baseURL": "https://relay.example"}},
                "other": {"npm": "other"}
            }
        });
        let imported = writer.extract_startup_settings(&live);
        assert_eq!(imported.len(), 2);
        assert!(imported.iter().any(|item| item["id"] == "relay"));
        assert!(imported.iter().any(|item| item["id"] == "other"));
    }

    #[test]
    fn hermes_startup_extraction_imports_all_custom_providers() {
        let writer = hermes::HermesWriter;
        let live = json!({
            "custom_providers": [
                {"name": "relay", "api_mode": "chat_completions"},
                {"name": "other", "api_mode": "anthropic_messages"}
            ]
        });
        let imported = writer.extract_startup_settings(&live);
        assert_eq!(imported.len(), 2);
        assert!(imported.iter().any(|item| item["name"] == "relay"));
        assert!(imported.iter().any(|item| item["name"] == "other"));
    }
}
