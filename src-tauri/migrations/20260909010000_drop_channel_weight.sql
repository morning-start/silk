-- 渠道级 weight 已废弃（权重改为模型级，存于 selected_models JSON），删除冗余列。
ALTER TABLE model_mapping_channels DROP COLUMN weight;
