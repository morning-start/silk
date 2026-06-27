-- 模型映射表（模型广场）
CREATE TABLE IF NOT EXISTS model_mappings (
    id                      TEXT PRIMARY KEY,
    model_name              TEXT NOT NULL UNIQUE,
    provider_group_id       TEXT REFERENCES provider_groups(id),
    -- Token 限制
    max_input_tokens        INTEGER,
    max_context_tokens      INTEGER,
    max_output_tokens       INTEGER,
    -- 定价（每百万 token）
    input_price_per_1m      REAL,
    output_price_per_1m     REAL,
    -- 能力标签（JSON 数组：["thinking","image","text","draw","code","audio"]）
    capabilities            TEXT NOT NULL DEFAULT '[]',
    -- 状态
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at              DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at              DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_model_mappings_group ON model_mappings(provider_group_id);
CREATE INDEX IF NOT EXISTS idx_model_mappings_enabled ON model_mappings(enabled);
