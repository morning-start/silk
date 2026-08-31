# iteration-003 验收记录（fst-review N6/N7）

> 日期：2026-08-31 · 分支：feat/iteration-003 → main（fast-forward `b0cb39a..4fb02ef`）

## 范围

P1 体验闭环 + P2 对标 cc-switch + TD-001 偿还（用户确认，spec 方略，4 批次）。

## 交付内容

| 批次 | 内容 | 提交 |
|------|------|------|
| B1 | TD-001 偿还：prism_wasm 日志缓冲测试串行锁 | `2c602b0` |
| B2 | live 导入（`import_live_config`）+ 三态回显（`get_agent_live_status`）+ 切换失败回滚 | `46ff119` |
| B3 | env 注入扩展 + 冲突检测 + 网关联动模型校验 | `46ff119` |
| B4 | MCP 服务器管理（`apply_mcp_servers` + `McpEditor.vue` × 5 弹窗） | `46ff119` |
| 验收补测 | `clean_imported_config` 单测 ×5 + opencode/hermes 剥离修复 | `4fb02ef` |

## 测试结果

- **变更针对性测试**：新增 9 个单测全部通过（clean_imported×5、inject_gateway_config、referenced_models、missing_referenced_models、apply_mcp_servers）；profile_service 14/14、config_writer 5/5
- **核心主干回归**：cargo check ✅；连续 9 次全量 `cargo test -p silk --lib` 126/126 ✅；`bun run build`（vue-tsc + vite）✅
- **CR-001 写入结构回归**：codex_writer×3、schema×3、into_provider/hermes 单测全部通过，无回归

## DoD 核销

| 项 | 结果 |
|----|------|
| 功能完成（含骨架冒烟） | ✅ |
| 变更针对性测试通过 | ✅ |
| 核心主干回归通过 | ✅ |
| 文档同步（requirements.md E9 行更新） | ✅ |
| 变更记录归档（plan/task/retrospective/dod-checklist） | ✅ |
| 待定需求不纳入交付验收 | ✅（无待定需求） |

全量核销 ✅ → 进入灰度。

## 灰度方案（单机）

- **形态**：单机无流量 → 合并 main + 本机 `bun run tauri dev` 实测
- **合并**：fast-forward 至 `4fb02ef`（已执行）
- **回滚点**：`b0cb39a`（合并前 main HEAD），必要时 `git revert 4fb02ef`
- **门槛**：G 系列本机验证通过率 ≥ 99%、无 P0 阻断缺陷

## 本机验证清单（G1~G7）

- [ ] G1：5 Tab 可切换，Profile 列表正常（CR-001 回归）
- [ ] G2：三态标签——激活 profile 显示「当前」+「需重启」（非 opencode/hermes）、live 未管理 agent 显示「未管理」
- [ ] G3：「从 live 配置导入」——对已存在 live 配置生成 Profile（5 个 agent 各测一次）
- [ ] G4：切换失败回滚——live 配置设为只读/损坏，切换应报错且 DB is_active 不变
- [ ] G5：MCP 编辑——各 Tab 弹窗增删 MCP 服务器，激活后检查 live 段（claude/gemini→`mcpServers`、opencode→`mcp`、codex/hermes→`mcp_servers`）
- [ ] G6：网关联动——激活引用不存在模型的 profile，warnings 提示「模型在模型池中不存在」
- [ ] G7：env 注入——claude_code profile 自定义 `env.ANTHROPIC_MODEL` 激活后 live settings.json 保留

## 结果

- 状态：**PASSED（待本机验证确认）**
- 缺陷：0 个阻断；验收补测发现并修复 1 个实现缺陷（opencode/hermes 导入未剥离注入键，`4fb02ef`）
- 遗留：G 系列本机验证由用户在 `bun run tauri dev` 环境确认；通过后本记录升级为最终验收，失败则 `git revert 4fb02ef` 并走 fst-change

## 交接

- 验收通过 → 回 fst-iterate N8 回顾闭环（回顾报告已生成，状态文件已更新至 review_handoff）
- 下轮候选（iteration-004）：Codex 保留 id 前端提示、MCP 各 harness 实测、live 导入多 provider 增强、模型池变更联动刷新
