-- 请求日志表增加费用字段（仅计算，不扣费）
ALTER TABLE request_logs ADD COLUMN cost REAL;
