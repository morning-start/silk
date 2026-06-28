-- 扩展模型映射表：增加更多模型元信息
ALTER TABLE model_mappings ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE model_mappings ADD COLUMN vendor TEXT NOT NULL DEFAULT '';
ALTER TABLE model_mappings ADD COLUMN knowledge_cutoff TEXT;
ALTER TABLE model_mappings ADD COLUMN model_family TEXT NOT NULL DEFAULT '';
ALTER TABLE model_mappings ADD COLUMN reference_url TEXT;
