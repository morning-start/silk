-- 模型映射渠道关联表（规范化设计）
-- 替代 model_mappings.provider_group_id 字段

-- 1. model_mappings 新增负载均衡策略字段
ALTER TABLE model_mappings ADD COLUMN strategy TEXT NOT NULL DEFAULT 'round_robin'
    CHECK(strategy IN ('round_robin', 'weighted', 'least_conn'));

-- 2. 创建关联表
CREATE TABLE IF NOT EXISTS model_mapping_channels (
    id              TEXT PRIMARY KEY,
    mapping_id      TEXT NOT NULL REFERENCES model_mappings(id) ON DELETE CASCADE,
    provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    -- 该渠道下选中的远程模型名列表（JSON 数组），如 ["gpt-4","gpt-4-turbo"]
    -- 请求体 model 匹配此列表时直接转发；否则使用列表第一个模型名覆盖
    selected_models TEXT NOT NULL DEFAULT '[]',
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at      DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE(mapping_id, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_mmc_mapping ON model_mapping_channels(mapping_id);
CREATE INDEX IF NOT EXISTS idx_mmc_provider ON model_mapping_channels(provider_id);
CREATE INDEX IF NOT EXISTS idx_mmc_enabled ON model_mapping_channels(enabled);

-- 3. 迁移已有数据：将 provider_group_id 的分组成员写入关联表
INSERT INTO model_mapping_channels (id, mapping_id, provider_id, selected_models, enabled, created_at)
SELECT
    lower(hex(randomblob(16))),
    m.id,
    gm.provider_id,
    '["' || m.model_name || '"]',  -- 默认将 mapping 的模型名作为选中模型
    1,
    datetime('now')
FROM model_mappings m
JOIN group_members gm ON gm.group_id = m.provider_group_id AND gm.enabled = 1
WHERE m.provider_group_id IS NOT NULL;