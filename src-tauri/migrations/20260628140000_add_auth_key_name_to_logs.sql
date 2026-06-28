-- 请求日志表增加认证 Key 名称字段
ALTER TABLE request_logs ADD COLUMN auth_key_name TEXT;
