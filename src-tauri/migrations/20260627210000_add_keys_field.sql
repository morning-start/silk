-- 为渠道增加 keys 字段（JSON 数组），支持多个 API Key
-- 格式：[{"name":"主密钥","value":"<encrypted>","enabled":true}]

ALTER TABLE providers ADD COLUMN keys TEXT NOT NULL DEFAULT '[]';
