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
    async fn write_live(
        &self,
        home: &Path,
        settings: &serde_json::Value,
    ) -> Result<(), String> {
        let path = self.live_path(home);
        let snapshot = LiveSnapshot::take(&path).map_err(|e| format!("备份失败: {e}"))?;

        let result = self.write_live_inner(home, settings).await;
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
    ) -> Result<(), String>;

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
