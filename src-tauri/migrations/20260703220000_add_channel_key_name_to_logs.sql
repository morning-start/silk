-- 请求日志表增加渠道 Key 名称字段
ALTER TABLE request_logs ADD COLUMN channel_key_name TEXT;
