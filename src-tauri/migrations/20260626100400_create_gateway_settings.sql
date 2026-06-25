-- 网关全局设置表
CREATE TABLE IF NOT EXISTS gateway_settings (
    id                  TEXT PRIMARY KEY CHECK(id = 'default'),
    bind_host           TEXT NOT NULL DEFAULT '127.0.0.1',
    bind_port           INTEGER NOT NULL DEFAULT 2013 CHECK(bind_port > 0 AND bind_port <= 65535),
    allow_remote        INTEGER NOT NULL DEFAULT 0 CHECK(allow_remote IN (0, 1)),
    auth_token_hash     TEXT,
    log_retention_days  INTEGER NOT NULL DEFAULT 30 CHECK(log_retention_days >= 1 AND log_retention_days <= 3650),
    default_provider_id TEXT REFERENCES providers(id),
    default_route_id    TEXT REFERENCES routing_rules(id),
    created_at          DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at          DATETIME NOT NULL DEFAULT (datetime('now'))
);
