-- 服务商分组表（负载均衡）
CREATE TABLE IF NOT EXISTS provider_groups (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    model_name      TEXT NOT NULL,
    strategy        TEXT NOT NULL DEFAULT 'round_robin' CHECK(strategy IN ('round_robin', 'weighted', 'least_conn')),
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at      DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_groups_model ON provider_groups(model_name);
CREATE INDEX IF NOT EXISTS idx_groups_enabled ON provider_groups(enabled);

-- 分组成员表
CREATE TABLE IF NOT EXISTS group_members (
    id              TEXT PRIMARY KEY,
    group_id        TEXT NOT NULL REFERENCES provider_groups(id) ON DELETE CASCADE,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    weight          INTEGER NOT NULL DEFAULT 1 CHECK(weight >= 1 AND weight <= 100),
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(group_id, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_members_group ON group_members(group_id);
CREATE INDEX IF NOT EXISTS idx_members_provider ON group_members(provider_id);

-- 路由规则新增 target_group_id 字段
ALTER TABLE routing_rules ADD COLUMN target_group_id TEXT REFERENCES provider_groups(id);
