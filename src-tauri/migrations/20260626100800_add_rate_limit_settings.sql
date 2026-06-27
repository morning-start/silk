-- 添加限流设置到网关配置表
ALTER TABLE gateway_settings ADD COLUMN rate_limit_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE gateway_settings ADD COLUMN rate_limit_max_requests_per_minute INTEGER NOT NULL DEFAULT 1000;
ALTER TABLE gateway_settings ADD COLUMN rate_limit_max_tokens_per_minute INTEGER NOT NULL DEFAULT 500000;
