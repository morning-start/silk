-- 重构 request_logs 表：精简主表字段，迁出扩展字段到 request_log_extra_token
--
-- 变更：
--   移除: request_headers, request_body, response_headers, response_body
--   重命名: duration_ms → resp_ms, model_used → model_id
--   新增: model_name（模型池名称）
--   迁出: cache_hit, request_size_bytes, response_size_bytes, tokens_input, tokens_output, cost → request_log_extra_token

-- 1. 删除旧的 request_logs 索引
DROP INDEX IF EXISTS idx_request_logs_ts;
DROP INDEX IF EXISTS idx_request_logs_provider_ts;
DROP INDEX IF EXISTS idx_request_logs_status_ts;
DROP INDEX IF EXISTS idx_request_logs_request_id;
DROP INDEX IF EXISTS idx_logs_timestamp;
DROP INDEX IF EXISTS idx_logs_provider;
DROP INDEX IF EXISTS idx_logs_request_id;
DROP INDEX IF EXISTS idx_request_logs_route_timestamp;
DROP INDEX IF EXISTS idx_request_logs_protocol_timestamp;

-- 2. 重建 request_logs 表
DROP TABLE IF EXISTS request_logs;
CREATE TABLE request_logs (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    method TEXT NOT NULL DEFAULT '',
    path TEXT NOT NULL DEFAULT '',
    route_id TEXT REFERENCES routing_rules(id) ON DELETE SET NULL,
    inbound_protocol TEXT,
    outbound_protocol TEXT,
    status_code INTEGER,
    resp_ms INTEGER,
    provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
    error_message TEXT,
    error_code TEXT,
    model_id TEXT,
    model_name TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    stream_enabled INTEGER NOT NULL DEFAULT 0,
    auth_key_name TEXT,
    channel_key_name TEXT
);

-- 3. 重建索引
CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_provider_ts ON request_logs(provider_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_status_ts ON request_logs(status_code, timestamp);
CREATE INDEX IF NOT EXISTS idx_request_logs_request_id ON request_logs(request_id);

-- 4. 创建 Token 扩展信息表
CREATE TABLE IF NOT EXISTS request_log_extra_token (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    request_size_bytes INTEGER,
    response_size_bytes INTEGER,
    tokens_input INTEGER,
    tokens_output INTEGER,
    cost REAL,
    FOREIGN KEY (request_id) REFERENCES request_logs(request_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_log_extra_token_request_id ON request_log_extra_token(request_id);
