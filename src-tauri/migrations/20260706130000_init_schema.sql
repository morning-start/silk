-- Silk 初始数据库结构（不保留历史升级链）
-- 仅服务于全新安装：直接创建当前最终 schema

-- 服务商配置表
CREATE TABLE providers (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL UNIQUE,
    protocols            TEXT NOT NULL DEFAULT '[]',
    models               TEXT NOT NULL DEFAULT '[]',
    keys                 TEXT NOT NULL DEFAULT '[]',
    key_strategy         TEXT NOT NULL DEFAULT 'round_robin',
    api_base_url         TEXT NOT NULL,
    proxy_url            TEXT,
    timeout_seconds      INTEGER NOT NULL DEFAULT 30 CHECK(timeout_seconds > 0 AND timeout_seconds <= 300),
    max_retries          INTEGER NOT NULL DEFAULT 3 CHECK(max_retries >= 0 AND max_retries <= 10),
    status               TEXT NOT NULL DEFAULT 'enabled' CHECK(status IN ('enabled', 'disabled')),
    health_status        TEXT,
    last_health_check_at DATETIME,
    metadata_json        TEXT,
    created_at           DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at           DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_providers_status ON providers(status);

-- 模型映射表（模型池）
CREATE TABLE model_mappings (
    id                 TEXT PRIMARY KEY,
    model_name         TEXT NOT NULL UNIQUE,
    max_input_tokens   INTEGER,
    max_context_tokens INTEGER,
    max_output_tokens  INTEGER,
    input_price_per_1m REAL,
    output_price_per_1m REAL,
    capabilities       TEXT NOT NULL DEFAULT '[]',
    description        TEXT NOT NULL DEFAULT '',
    vendor             TEXT NOT NULL DEFAULT '',
    knowledge_cutoff   TEXT,
    model_family       TEXT NOT NULL DEFAULT '',
    reference_url      TEXT,
    strategy           TEXT NOT NULL DEFAULT 'round_robin'
        CHECK(strategy IN ('round_robin', 'weighted', 'least_conn')),
    enabled            INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at         DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at         DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_model_mappings_enabled ON model_mappings(enabled);
CREATE INDEX idx_model_mappings_model_name ON model_mappings(model_name);

-- 模型映射渠道关联表
CREATE TABLE model_mapping_channels (
    id              TEXT PRIMARY KEY,
    mapping_id      TEXT NOT NULL REFERENCES model_mappings(id) ON DELETE CASCADE,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    selected_models TEXT NOT NULL DEFAULT '[]',
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(mapping_id, provider_id)
);

CREATE INDEX idx_mmc_mapping ON model_mapping_channels(mapping_id);
CREATE INDEX idx_mmc_provider ON model_mapping_channels(provider_id);
CREATE INDEX idx_mmc_enabled ON model_mapping_channels(enabled);

-- 网关 API Key 管理表
CREATE TABLE gateway_keys (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,
    key_hash            TEXT NOT NULL UNIQUE,
    encrypted_key_value TEXT NOT NULL,
    enabled             INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    expires_at          DATETIME,
    max_concurrent      INTEGER NOT NULL DEFAULT 10,
    created_at          DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at          DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_gateway_keys_hash ON gateway_keys(key_hash);
CREATE INDEX idx_gateway_keys_enabled ON gateway_keys(enabled);

-- 请求日志主表
CREATE TABLE request_logs (
    id                TEXT PRIMARY KEY,
    request_id        TEXT NOT NULL DEFAULT '',
    timestamp         DATETIME NOT NULL DEFAULT (datetime('now')),
    method            TEXT NOT NULL DEFAULT '',
    path              TEXT NOT NULL DEFAULT '',
    inbound_protocol  TEXT,
    outbound_protocol TEXT,
    status_code       INTEGER,
    resp_ms           INTEGER,
    total_duration_ms INTEGER,
    provider_id       TEXT REFERENCES providers(id) ON DELETE SET NULL,
    error_message     TEXT,
    error_code        TEXT,
    model_id          TEXT,
    model_name        TEXT,
    retry_count       INTEGER NOT NULL DEFAULT 0,
    stream_enabled    INTEGER NOT NULL DEFAULT 0,
    auth_key_name     TEXT,
    channel_key_name  TEXT
);

CREATE INDEX idx_request_logs_ts ON request_logs(timestamp);
CREATE INDEX idx_request_logs_provider_ts ON request_logs(provider_id, timestamp);
CREATE INDEX idx_request_logs_status_ts ON request_logs(status_code, timestamp);
CREATE INDEX idx_request_logs_request_id ON request_logs(request_id);

-- 请求日志扩展表（缓存、体积、token）
CREATE TABLE request_log_extra_token (
    id                  TEXT PRIMARY KEY,
    request_id          TEXT NOT NULL,
    cache_hit           INTEGER NOT NULL DEFAULT 0,
    request_size_bytes  INTEGER,
    response_size_bytes INTEGER,
    tokens_input        INTEGER,
    tokens_output       INTEGER,
    tokens_sent         INTEGER
);

CREATE INDEX idx_log_extra_token_request_id ON request_log_extra_token(request_id);

-- Profile 表
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude_desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes'
    )),
    config_json TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    sort_index INTEGER,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_profiles_agent_type ON profiles(agent_type);
CREATE INDEX idx_profiles_active ON profiles(agent_type, is_active) WHERE is_active = 1;

-- 通用配置片段表
CREATE TABLE common_config_snippets (
    id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL CHECK(agent_type IN (
        'claude_code', 'claude_desktop', 'codex',
        'gemini_cli', 'opencode', 'openclaw', 'hermes'
    )),
    content TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(agent_type)
);

CREATE INDEX idx_common_snippets_agent ON common_config_snippets(agent_type);
