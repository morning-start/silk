-- 网关 API Key 管理表
CREATE TABLE IF NOT EXISTS gateway_keys (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL UNIQUE,
    key_prefix      TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    expires_at      DATETIME,
    max_concurrent  INTEGER NOT NULL DEFAULT 10,
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at      DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_gateway_keys_hash ON gateway_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_gateway_keys_enabled ON gateway_keys(enabled);
