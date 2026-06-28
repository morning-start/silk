-- 清除旧加密 API Key 数据（已移除 AES-GCM 加密，改为明文存储）
-- 同时修复 FOREIGN KEY 冲突：将日志表中已删除 Provider 的引用置空

-- 1. 清除 providers 表中仍是加密格式的密钥（JSON 密文格式如 {"nonce":"...","ciphertext":"..."}）
UPDATE providers
SET keys = '[]'
WHERE keys LIKE '%"ciphertext"%';

-- 2. 清除 request_logs 中引用已删除 provider 的 provider_id
UPDATE request_logs
SET provider_id = NULL
WHERE provider_id IS NOT NULL
  AND provider_id NOT IN (SELECT id FROM providers);

-- 3. 清除 model_mapping_channels 中引用已删除 provider 的记录
DELETE FROM model_mapping_channels
WHERE provider_id IS NOT NULL
  AND provider_id NOT IN (SELECT id FROM providers);