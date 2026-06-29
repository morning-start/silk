-- P1: 外键约束 + 复合索引 + 请求日志表重构
--
-- 注：此迁移删除并重建 request_logs 表，增加 ON DELETE SET NULL 外键约束。
-- 开发阶段可删除 silk.db 重建；如已有数据，该表将被清空。

-- 删除 20260629010000 创建的单列索引（将被复合索引替代）
DROP INDEX IF EXISTS idx_request_logs_provider_id;
DROP INDEX IF EXISTS idx_request_logs_status_code;

-- 重建 request_logs 表，添加外键约束
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
    request_headers TEXT,
    request_body TEXT,
    status_code INTEGER,
    response_headers TEXT,
    response_body TEXT,
    duration_ms INTEGER,
    provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
    error_message TEXT,
    error_code TEXT,
    model_used TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    stream_enabled INTEGER NOT NULL DEFAULT 0,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    request_size_bytes INTEGER,
    response_size_bytes INTEGER,
    tokens_input INTEGER,
    tokens_output INTEGER,
    cost REAL,
    auth_key_name TEXT
);

-- 复合索引：按时间戳查询（stats、分页、清理）
CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(timestamp);
-- 复合索引：按渠道 + 时间查询
CREATE INDEX IF NOT EXISTS idx_request_logs_provider_ts ON request_logs(provider_id, timestamp);
-- 复合索引：按状态码 + 时间查询（成功率统计）
CREATE INDEX IF NOT EXISTS idx_request_logs_status_ts ON request_logs(status_code, timestamp);
-- 单列索引：按请求 ID 查询（详情跳转）
CREATE INDEX IF NOT EXISTS idx_request_logs_request_id ON request_logs(request_id);