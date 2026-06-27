-- 删除 model_name 列，已由 models（JSON 数组）替代

-- SQLite 不支持直接 DROP COLUMN（需 3.35.0+），此处使用兼容方式
-- 对于旧版 SQLite，列保留但不使用；新版直接删除
ALTER TABLE providers DROP COLUMN model_name;
