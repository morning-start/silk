-- P0: 删除废弃字段
DROP INDEX IF EXISTS idx_providers_type;
DROP INDEX IF EXISTS idx_model_mappings_group;
DROP INDEX IF EXISTS idx_logs_status;
ALTER TABLE providers DROP COLUMN provider_type;
ALTER TABLE model_mappings DROP COLUMN provider_group_id;
ALTER TABLE request_logs DROP COLUMN response_status;
ALTER TABLE gateway_settings DROP COLUMN auth_token_hash;

-- P1: 补充缺失索引
CREATE INDEX IF NOT EXISTS idx_request_logs_provider_id ON request_logs(provider_id);
CREATE INDEX IF NOT EXISTS idx_request_logs_status_code ON request_logs(status_code);
CREATE INDEX IF NOT EXISTS idx_model_mappings_model_name ON model_mappings(model_name);
