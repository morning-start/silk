-- 为 Provider 增加 protocols 字段（JSON 数组），替代 provider_type
-- 自动迁移现有数据的 provider_type 到 protocols

ALTER TABLE providers ADD COLUMN protocols TEXT NOT NULL DEFAULT '["chat"]';

-- 迁移已有数据：根据 provider_type 映射到对应协议列表
UPDATE providers SET protocols = '["chat","response"]' WHERE provider_type = 'openai';
UPDATE providers SET protocols = '["message"]' WHERE provider_type = 'anthropic';
UPDATE providers SET protocols = '["chat"]' WHERE provider_type = 'azure';
UPDATE providers SET protocols = '["chat"]' WHERE provider_type = 'custom';

-- 旧列保留不动（向后兼容），新代码不再使用
