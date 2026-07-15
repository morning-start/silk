-- Profile 表
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude_desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes'
    )),
    config_json TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    sort_index INTEGER,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_profiles_agent_type ON profiles(agent_type);
CREATE INDEX idx_profiles_active ON profiles(agent_type, is_active) WHERE is_active = 1;

-- 通用配置片段表
CREATE TABLE common_config_snippets (
    id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude_desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes'
    )),
    content TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(agent_type)
);

CREATE INDEX idx_common_snippets_agent ON common_config_snippets(agent_type);
