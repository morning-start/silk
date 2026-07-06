-- 模型映射渠道关联表
CREATE TABLE IF NOT EXISTS model_mapping_channels (
    id              TEXT PRIMARY KEY,
    mapping_id      TEXT NOT NULL REFERENCES model_mappings(id) ON DELETE CASCADE,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    selected_models TEXT NOT NULL DEFAULT '[]',
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(mapping_id, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_mmc_mapping ON model_mapping_channels(mapping_id);
CREATE INDEX IF NOT EXISTS idx_mmc_provider ON model_mapping_channels(provider_id);
CREATE INDEX IF NOT EXISTS idx_mmc_enabled ON model_mapping_channels(enabled);
