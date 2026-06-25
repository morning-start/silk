-- 请求日志表
CREATE TABLE IF NOT EXISTS request_logs (
    id              TEXT PRIMARY KEY,
    request_id      TEXT NOT NULL,
    timestamp       DATETIME NOT NULL DEFAULT (datetime('now')),
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    request_headers TEXT,
    request_body    TEXT,
    response_status INTEGER,
    response_headers TEXT,
    response_body   TEXT,
    duration_ms     INTEGER,
    provider_id     TEXT REFERENCES providers(id),
    error_message   TEXT,
    model_used      TEXT,
    tokens_input    INTEGER,
    tokens_output   INTEGER
);

CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_logs_provider ON request_logs(provider_id);
CREATE INDEX IF NOT EXISTS idx_logs_request_id ON request_logs(request_id);
CREATE INDEX IF NOT EXISTS idx_logs_status ON request_logs(response_status);
