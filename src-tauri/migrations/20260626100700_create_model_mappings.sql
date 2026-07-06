-- 模型映射表（模型广场）
CREATE TABLE IF NOT EXISTS model_mappings (
    id                  TEXT PRIMARY KEY,
    model_name          TEXT NOT NULL UNIQUE,
    max_input_tokens    INTEGER,
    max_context_tokens  INTEGER,
    max_output_tokens   INTEGER,
    input_price_per_1m  REAL,
    output_price_per_1m REAL,
    capabilities        TEXT NOT NULL DEFAULT '[]',
    description         TEXT NOT NULL DEFAULT '',
    vendor              TEXT NOT NULL DEFAULT '',
    knowledge_cutoff    TEXT,
    model_family        TEXT NOT NULL DEFAULT '',
    reference_url       TEXT,
    strategy            TEXT NOT NULL DEFAULT 'round_robin'
        CHECK(strategy IN ('round_robin', 'weighted', 'least_conn')),
    enabled             INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at          DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at          DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_model_mappings_enabled ON model_mappings(enabled);
CREATE INDEX IF NOT EXISTS idx_model_mappings_model_name ON model_mappings(model_name);
