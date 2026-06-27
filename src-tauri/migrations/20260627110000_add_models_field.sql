-- 为 Provider 增加 models 字段（JSON 数组），支持多模型
-- 并将现有 model_name 数据迁移到 models

ALTER TABLE providers ADD COLUMN models TEXT NOT NULL DEFAULT '[]';

-- 迁移：将 model_name 转入 models（非空时）
UPDATE providers SET models = CASE
    WHEN model_name IS NOT NULL AND model_name != '' THEN '["' || model_name || '"]'
    ELSE '[]'
END;
