-- 官方直连预设种子行（对齐 cc-switch official provider，首批 claude_code 样板）：
-- category='official' = 锚定官方信息：激活时不注入网关端点/Key，写入时剥离全部管理键
-- 回归官方登录；前端只读展示、后端禁止编辑/删除/参与重排（sort_index=0 恒排首位）。
INSERT OR IGNORE INTO presets (id, name, agent_type, settings_config, category, notes, sort_index, is_active)
VALUES (
    'official-claude-code',
    'Claude Code 官方',
    'claude_code',
    '{"env":{}}',
    'official',
    '官方直连：使用 Claude 账号登录，不注入自定义端点/Key/模型',
    0,
    0
);
