-- 彻底清理：删除 api_key 列，统一使用 keys（JSON 数组）
ALTER TABLE providers DROP COLUMN api_key;
