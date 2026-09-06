-- 官方直连预设种子行（对齐 cc-switch official provider）第二批：codex + gemini_cli。
-- category='official' = 锚定官方信息：端点/模型锚定官方，仅凭据可填（codex 官方为 OAuth
-- 登录无 key，gemini 官方可填 Google API Key 或走 OAuth）；激活时剥离管理足迹回归官方，
-- 前端只读展示凭据表单、后端禁止删除/参与重排（sort_index=0 恒排首位）。
INSERT OR IGNORE INTO presets (id, name, agent_type, settings_config, category, notes, sort_index, is_active)
VALUES
    (
        'official-codex',
        'Codex 官方',
        'codex',
        '{}',
        'official',
        '官方直连：使用 OpenAI 账号 OAuth 登录（codex login），不注入自定义端点/Key',
        0,
        0
    ),
    (
        'official-gemini-cli',
        'Gemini CLI 官方',
        'gemini_cli',
        '{"env":{}}',
        'official',
        '官方直连：Google 账号 OAuth 或 Google AI API Key，不注入自定义端点/模型',
        0,
        0
    );
