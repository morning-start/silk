use serde::{Deserialize, Serialize};

/// Agent（harness）类型，对齐 cc-switch 的 AppType。
/// 首批 5 个（claude_code/codex/opencode/hermes/gemini_cli），
/// 其余（claude-desktop/grokbuild/openclaw/pi）后续批次扩展。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentType {
    pub id: String,
    pub name: String,
}

impl AgentType {
    pub const fn all() -> &'static [(&'static str, &'static str)] {
        &[
            ("claude_code", "Claude Code"),
            ("codex", "Codex"),
            ("opencode", "OpenCode"),
            ("hermes", "Hermes"),
            ("gemini_cli", "Gemini CLI"),
        ]
    }

    pub fn all_typed() -> Vec<Self> {
        Self::all()
            .iter()
            .map(|(id, name)| Self {
                id: id.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    pub fn is_valid(id: &str) -> bool {
        Self::all().iter().any(|(a, _)| *a == id)
    }

    pub fn name_for(id: &str) -> Option<&'static str> {
        Self::all().iter().find(|(a, _)| *a == id).map(|(_, n)| *n)
    }

    /// 是否需要重启终端/应用才能生效（与 cc-switch 一致：仅 opencode/hermes 免重启）
    pub fn requires_restart(id: &str) -> bool {
        !matches!(id, "opencode" | "hermes")
    }
}
