-- 为渠道增加密钥负载均衡策略字段
ALTER TABLE providers ADD COLUMN key_strategy TEXT NOT NULL DEFAULT 'round_robin';
