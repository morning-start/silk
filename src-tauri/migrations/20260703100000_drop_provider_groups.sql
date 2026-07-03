-- 分组管理功能已合并到模型池
-- provider_groups 和 group_members 表功能已被 model_mappings + model_mapping_channels 替代
DROP TABLE IF EXISTS group_members;
DROP TABLE IF EXISTS provider_groups;
