-- 服务商配置表
CREATE TABLE IF NOT EXISTS providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    protocols       TEXT NOT NULL DEFAULT '[]',
    models          TEXT NOT NULL DEFAULT '[]',
    keys            TEXT NOT NULL DEFAULT '[]',
    key_strategy    TEXT NOT NULL DEFAULT 'round_robin',
    api_base_url    TEXT NOT NULL,
    proxy_url       TEXT,
    timeout_seconds INTEGER NOT NULL DEFAULT 30 CHECK(timeout_seconds > 0 AND timeout_seconds <= 300),
    max_retries     INTEGER NOT NULL DEFAULT 3 CHECK(max_retries >= 0 AND max_retries <= 10),
    status          TEXT NOT NULL DEFAULT 'enabled' CHECK(status IN ('enabled', 'disabled')),
    health_status   TEXT,
    last_health_check_at DATETIME,
    metadata_json   TEXT,
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at      DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_providers_status ON providers(status);
