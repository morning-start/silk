-- presets 表 agent_type 允许 'omp'（OMP 模型管理，CR-004）
-- SQLite 不支持修改 CHECK 约束，需重建表（无外键引用 presets，安全）
CREATE TABLE presets_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude-desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes', 'grokbuild', 'pi', 'omp'
    )),
    settings_config TEXT NOT NULL,
    category TEXT,
    notes TEXT,
    sort_index INTEGER,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO presets_new (id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at)
SELECT id, name, agent_type, settings_config, category, notes, sort_index, is_active, created_at, updated_at FROM presets;

DROP TABLE presets;
ALTER TABLE presets_new RENAME TO presets;

CREATE INDEX IF NOT EXISTS idx_presets_agent_type ON presets(agent_type);
CREATE INDEX IF NOT EXISTS idx_presets_active ON presets(agent_type, is_active) WHERE is_active = 1;
