-- 修复 request_log_extra_token
--
-- 1. 原 FK: FOREIGN KEY (request_id) REFERENCES request_logs(request_id)
--    request_logs.request_id 不是 PRIMARY KEY 也没有 UNIQUE 约束，SQLite 拒绝该 FK，
--    导致写入扩展日志失败（foreign key mismatch）。
-- 2. 新增 tokens_sent 字段：发送给上游渠道的请求 token 估算值
--
-- 修复：删除 FK + 添加 tokens_sent 列

DROP TABLE IF EXISTS request_log_extra_token;
CREATE TABLE request_log_extra_token (
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
