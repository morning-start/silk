# iteration-002 验收记录（CR-001 完善预设管理）

> 定稿：2026-08-31 · 验收技能：fst-review（N6/N7）· 状态：✅ 已验收
> 草稿位置：`.agent-workplace/iterations/iteration-002/`（过程态，不提交 git）

## 1. 变更针对性测试

针对 CR-001 影响点清单逐项验证：

| 影响点 | 验证方式 | 结果 |
|--------|---------|------|
| config_writer 合并策略（OpenCode `provider` 键、Hermes `custom_providers` 列表 + `model` 段） | 4 个单测：`into_provider_writes_singular_provider_key`、`into_hermes_provider_writes_custom_providers_list_and_model`、`into_hermes_provider_replaces_existing_by_name`、`remove_provider_from_live_respects_strategy` | ✅ |
| CodexWriter（`[model_providers.<id>]` 表、保留 id 校验） | 3 个单测：`codex_writer_builds_model_providers_table`、`codex_writer_openai_uses_openai_base_url`、`codex_writer_rejects_reserved_ollama` | ✅ |
| switch() 注入位置按 harness 映射 | 代码核对：claude_code/gemini_cli→env 子对象、codex→model_providers 表/顶层、opencode/hermes→条目内 | ✅ |
| schema 校验（codex/gemini_cli/opencode/hermes 必填字段） | 3 个单测：`schema_codex_requires_model_and_wire_api`、`schema_gemini_requires_env_key`、`schema_opencode_hermes_require_provider_id` | ✅ |
| 前端 4 Tab（OpenCode 真保存 + Codex/Gemini CLI/Hermes） | `bun run build`（vue-tsc 类型检查）+ vite 构建 | ✅ |

新增单测合计：**10 个**（config_writer 4 + profile_service 6）。

## 2. 核心回归

| 项 | 命令 | 结果 |
|----|------|------|
| 全量 Rust 测试 | `cargo test -p silk --lib` | ✅ 117 passed / 0 failed |
| Rust 类型检查 | `cargo check -p silk` | ✅ |
| 前端类型 + 构建 | `bun run build` | ✅ vue-tsc + vite 全绿 |
| Claude Code 预设链路 | diff 核对：ClaudeCodeWriter 本体未改动，roles 契约（`{"roles": {...}}`）保持，仅注入位置改为 env 子对象 | ✅ |

**已知测试波动**：`prism_wasm::test_convert_request_trace` 在全量并行下偶发失败（全局单例共享日志缓冲竞态），已通过 stash 干净代码 + 连续 3 次全量复跑确认与 CR-001 无关，登记为 TD-001。

## 3. DoD 核销（schema 5.4）

| 项 | 必须 | 通过 |
|----|------|------|
| 功能完成（含骨架冒烟） | ✅ | ✅ |
| 变更针对性测试通过 | ✅ | ✅ |
| 核心主干回归通过 | ✅ | ✅ |
| 文档同步 | ✅ | ✅ |
| 变更记录归档（CR-001） | ✅ | ✅ |
| 待定需求不纳入验收 | — | ✅ |

**all_passed = true → 可进入灰度**

## 4. 灰度方案与放量决策（N7）

单机桌面应用 → 灰度 = 分支合并 + 本机实测：

- **验证清单**：G1 五 Tab 可切换 / G2-G5 Codex·Gemini·Hermes·OpenCode 创建+激活 / G6 Claude Code 既有预设回归 / G7 live 配置写入结构（`model_providers` 表、`provider` 键、`custom_providers` 列表）
- **监控指标**：无 P0 阻断缺陷、激活不报错、live 配置与 CLI 原生格式一致
- **回滚预案**：提交 `923b295`（合并）/ `c0b0ecf`（功能）保留，阻断可 revert
- **决策**：用户确认「合并并本机验证」✅

## 5. 交接

- 合并提交：`923b295 Merge feat/CR-001`（main）
- 缺陷清单：无 P0 阻断缺陷；TD-001（测试竞态）已登记待偿还
- 反馈汇总：无遗漏需求；下轮候选已入迭代回顾（P1/P2 扩展、TD-001 偿还）

**验收结论：PASSED → 进入 fst-iterate 回顾闭环（N8）或排下轮迭代**
