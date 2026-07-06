-- 删除 request_logs 表的 route_id 列及其外键约束
-- routing_rules 功能已彻底移除，该外键指向的表已不存在

PRAGMA foreign_keys = OFF;

CREATE TABLE request_logs_new (
    id                  TEXT NOT NULL PRIMARY KEY,
    request_id          TEXT NOT NULL DEFAULT '',
    timestamp           TEXT NOT NULL DEFAULT (datetime('now')),
    method              TEXT NOT NULL DEFAULT '',
    path                TEXT NOT NULL DEFAULT '',
    inbound_protocol    TEXT,
    outbound_protocol   TEXT,
    status_code         INTEGER,
    resp_ms             INTEGER,
    total_duration_ms   INTEGER,
    provider_id         TEXT,
    error_message       TEXT,
    error_code          TEXT,
    model_id            TEXT,
    model_name          TEXT,
    retry_count         INTEGER NOT NULL DEFAULT 0,
    stream_enabled      INTEGER NOT NULL DEFAULT 0,
    auth_key_name       TEXT,
    channel_key_name    TEXT
);

INSERT INTO request_logs_new (
    id, request_id, timestamp, method, path,
    inbound_protocol, outbound_protocol, status_code, resp_ms, total_duration_ms,
    provider_id, error_message, error_code, model_id, model_name,
    retry_count, stream_enabled, auth_key_name, channel_key_name
)
SELECT
    id, request_id, timestamp, method, path,
    inbound_protocol, outbound_protocol, status_code, resp_ms, total_duration_ms,
    provider_id, error_message, error_code, model_id, model_name,
    retry_count, stream_enabled, auth_key_name, channel_key_name
FROM request_logs;

DROP TABLE request_logs;
ALTER TABLE request_logs_new RENAME TO request_logs;

PRAGMA foreign_keys = ON;
