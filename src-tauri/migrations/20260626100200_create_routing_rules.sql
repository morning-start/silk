-- 路由规则表
CREATE TABLE IF NOT EXISTS routing_rules (
    id                      TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    match_path              TEXT NOT NULL,
    match_method            TEXT NOT NULL DEFAULT '*' CHECK(match_method IN ('GET', 'POST', 'PUT', 'DELETE', '*')),
    match_content_type      TEXT,
    target_provider_id      TEXT NOT NULL REFERENCES providers(id),
    protocol_conversion     INTEGER NOT NULL DEFAULT 1 CHECK(protocol_conversion IN (0, 1)),
    model_name_override     TEXT,
    priority                INTEGER NOT NULL DEFAULT 100,
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at              DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at              DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_routing_rules_enabled_priority ON routing_rules(enabled, priority);
