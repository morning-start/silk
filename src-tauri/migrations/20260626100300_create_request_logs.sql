-- 请求日志表
CREATE TABLE IF NOT EXISTS request_logs (
    id                  TEXT PRIMARY KEY,
    request_id          TEXT NOT NULL,
    timestamp           TEXT NOT NULL DEFAULT (datetime('now')),
    method              TEXT NOT NULL DEFAULT '',
    path                TEXT NOT NULL DEFAULT '',
    route_id            TEXT REFERENCES routing_rules(id) ON DELETE SET NULL,
    inbound_protocol    TEXT,
    outbound_protocol   TEXT,
    status_code         INTEGER,
    resp_ms             INTEGER,
    total_duration_ms   INTEGER,
    provider_id         TEXT REFERENCES providers(id) ON DELETE SET NULL,
    error_message       TEXT,
    error_code          TEXT,
    model_id            TEXT,
    model_name          TEXT,
    retry_count         INTEGER NOT NULL DEFAULT 0,
    stream_enabled      INTEGER NOT NULL DEFAULT 0,
    auth_key_name       TEXT,
    channel_key_name    TEXT
);

CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_provider_ts ON request_logs(provider_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_status_ts ON request_logs(status_code, timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_request_id ON request_logs(request_id);

CREATE TABLE IF NOT EXISTS request_log_extra_token (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    request_size_bytes INTEGER,
    response_size_bytes INTEGER,
    tokens_input INTEGER,
    tokens_output INTEGER,
    tokens_sent INTEGER,
    cost REAL
);

CREATE INDEX IF NOT EXISTS idx_log_extra_token_request_id ON request_log_extra_token(request_id);
