# CR-001 — 完善预设管理（E9 完整实现，P0）

> 变更申请单 · 归档定稿 · flowstate schema 5.3
> 归档时间：2026-08-31 · 审批人：用户 · 排期：iteration-002

## 需求原文（逐字记录）

- 「实现 预设管理 这部分，要怎么做」
- 「先进行silk的预设修改能力探讨。需要提供什么样的能力，基础来说什么样的功能，什么功能扩展。基于我这个silk的设计背景进行分析」
- 「设计规划迭代」（flowstate 路由意图）

**需求本质**：完善 silk 预设管理 —— 需求文档 E9「非 Claude Code Agent 预设」由「⚠️ 部分完成」改为完整实现，并修复调研发现的 4 个 harness 写入结构偏差（预设激活后 CLI 无法识别的缺陷）。

## 分级与决策

| 项 | 值 |
|---|---|
| 变更编号 | CR-001 |
| 提出人 / 渠道 | 用户（对话 IM，2026-08-31） |
| 分级 | **重大**（核心流程改动：`config_writer` 合并逻辑为所有 agent 切换共用写入核心） |
| 决策 | **接受** |
| 审批人 | 用户（2026-08-31） |
| 排期 | **iteration-002** |
| 范围 | **P0**（让预设真正可用） |

## 影响评估

| 项 | 值 |
|---|---|
| 是否改库 | 否（`profiles` / `common_config_snippets` 表已存在） |
| 是否重构 | 是（`config_writer.rs` 合并策略 + `profile_service.rs` 4 个 Writer 结构） |
| 是否影响旧功能 | 是（OpenCode/Hermes 现有写入结构错误，已激活配置需重新写入；Claude Code 预设回归验证） |
| 工时估算 | 约 2-3 天 |
| 返工量 | 已激活的 OpenCode/Hermes 配置需重写 |

## 影响点清单（供 fst-review 变更针对性测试）

1. `config_writer.rs` `MergeStrategy` 与 `apply_merge`：OpenCode 写入键 `providers` → 原生 `provider`（单数）；Hermes 字典 → `custom_providers` 列表 + `model` 段
2. `profile_service.rs` Writer：Codex 增加 `[model_providers.<id>]` 表与保留 id 校验（openai→`openai_base_url`）；Gemini CLI 密钥 `.env` 文件
3. `switch()` 注入位置：base_url/api_key 按 harness 映射到正确字段（Claude→`env.ANTHROPIC_*` 等）
4. 前端 `AgentProfilesView.vue`：OpenCode 真保存（create/update_profile）、新增 Codex / Gemini CLI / Hermes 三 Tab
5. 校验：`validate_profile_payload` 增加按 agent 的 schema 校验（Codex 必填 `model`/`wire_api` 等）
6. Claude Code 既有预设回归（现有 config_json 契约 `{"roles": {...}}` 是否受影响）

## 需同步的文档清单（由 fst-iterate 执行时一并更新）

- `docs/core/requirements.md` — E9 状态「⚠️ 部分完成」→「✅ 已完成」
- `docs/reference/dev-guide.md` / `docs/guides/api.md` — 如命令/结构变化
- `docs/core/architecture.md` — 如 Writer/合并策略描述变化
- `AGENTS.md` — 如有架构说明变化

## 归档状态

- 草稿：`.agent-workplace/state/CR-001-preset-management.draft.json`（过程态）
- 定稿：本文件 `docs/cr/CR-001-preset-management.md`（提交）
- 交接信号：**变更已归档，可进入 fst-iterate 排期实现**
