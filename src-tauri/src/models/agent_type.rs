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
            ("omp", "OMP"),
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

    /// silk 网关 prism 可转换的协议全集（与 prism_wasm::map_provider 白名单一致）。
    /// 命名的单一事实来源：各 harness 的能力表以此区分「可走网关自动转换」与「仅直连」。
    pub const GATEWAY_CONVERTIBLE_PROTOCOLS: &'static [&'static str] =
        &["openai", "responses", "messages", "gemini"];

    /// 各 harness 原生协议能力（silk 规范名，即 CLI 对上位端点实际能说的 wire 协议）：
    /// - 单协议 harness（claude_code/codex/gemini_cli）：协议由 CLI 固定，表单无需协议字段，
    ///   silk 自动转换该协议到任意上游；
    /// - 多协议 harness（opencode 经 npm SDK、hermes 经 api_mode）：表单保留协议选择器；
    /// - bedrock 等不在 GATEWAY_CONVERTIBLE 内 = 仅直连第三方端点可用，silk 网关不转换。
    pub const HARNESS_NATIVE_PROTOCOLS: &'static [(&'static str, &'static [&'static str])] = &[
        ("claude_code", &["messages"]),
        ("codex", &["responses"]),
        ("opencode", &["openai", "responses", "messages", "gemini", "bedrock"]),
        ("hermes", &["openai", "messages", "responses", "bedrock"]),
        ("gemini_cli", &["gemini"]),
        // OMP models.yml 的 api 字段取值：openai-completions/openai-responses/
        // anthropic-messages/google-generative-ai（对应 openai/responses/messages/gemini）
        ("omp", &["openai", "responses", "messages", "gemini"]),
    ];

    /// 返回 harness 原生支持的协议集（未登记返回空）
    pub fn native_protocols(agent_type: &str) -> &'static [&'static str] {
        Self::HARNESS_NATIVE_PROTOCOLS
            .iter()
            .find(|(id, _)| *id == agent_type)
            .map(|(_, protocols)| *protocols)
            .unwrap_or(&[])
    }

    /// 该协议是否可由 silk 网关 prism 转换
    pub fn is_gateway_convertible(protocol: &str) -> bool {
        Self::GATEWAY_CONVERTIBLE_PROTOCOLS.contains(&protocol)
    }
}
