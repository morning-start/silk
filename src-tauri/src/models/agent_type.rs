use serde::{Deserialize, Serialize};

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
            ("gemini_cli", "Gemini CLI"),
            ("opencode", "OpenCode"),
            ("hermes", "Hermes"),
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
}