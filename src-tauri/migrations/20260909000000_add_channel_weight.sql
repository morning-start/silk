-- 模型映射渠道增加权重列（负载均衡加权轮询/加权随机用，默认 1）。
ALTER TABLE model_mapping_channels ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;
