-- 新增 total_duration_ms 字段到 request_logs 表
-- resp_ms = 响应时间（首字节），total_duration_ms = 总耗时（末字节，仅流式有值）

ALTER TABLE request_logs ADD COLUMN total_duration_ms INTEGER;
