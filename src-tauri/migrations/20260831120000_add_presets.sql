-- Preset 预设表（对齐 cc-switch providers 概念）
CREATE TABLE IF NOT EXISTS presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude-desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes', 'grokbuild', 'pi'
    )),
    settings_config TEXT NOT NULL,
    category TEXT,
    notes TEXT,
    sort_index INTEGER,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_presets_agent_type ON presets(agent_type);
CREATE INDEX IF NOT EXISTS idx_presets_active ON presets(agent_type, is_active) WHERE is_active = 1;
