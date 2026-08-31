# iteration-004 验收记录（fst-review N6/N7）

> 日期：2026-08-31 · 分支：feat/iteration-004 → main（fast-forward `7d2924e..1818d0a`）

## 范围

Codex 保留 id 前端提示 + live 导入多 provider 增强 + 模型池变更联动刷新（用户确认，spec 方略，3 批次）。

## 交付内容

| 批次 | 内容 | 提交 |
|------|------|------|
| B1 | Codex 表单 ollama/lmstudio 保留 id 前端拦截提示（与后端错误一致） | `1818d0a` |
| B2 | `list_importable_providers` + `import_live_config(provider_key)` 指定条目导入 + 前端多 provider 选择弹窗 | `1818d0a` |
| B3 | 模型池/渠道变更联动：`AgentProfilesView` 订阅 providers/groups data-changed 信号自动刷新 | `1818d0a` |

## 测试结果

- **变更针对性测试**：B2 新增 2 个指定导入单测（opencode/hermes pick specified provider），profile_service 16/16；B1/B3 前端经 `bun run build`（vue-tsc + vite）验证 ✅
- **核心主干回归**：cargo check ✅；连续 5 次全量 `cargo test -p silk --lib` 128/128 ✅；`bun run build` ✅
- **iteration-003 回归**：clean_imported×5、inject_gateway_config、MCP、三态回显相关单测全部通过，无回归

## DoD 核销

| 项 | 结果 |
|----|------|
| 功能完成（含骨架冒烟） | ✅ |
| 变更针对性测试通过 | ✅ |
| 核心主干回归通过 | ✅ |
| 文档同步 | ✅（E9 范围内增强，requirements.md 已覆盖） |
| 变更记录归档 | ✅（plan/task/retrospective/dod-checklist） |
| 待定需求不纳入交付验收 | ✅（无待定需求） |

全量核销 ✅ → 进入灰度。

## 灰度方案（单机）

- **形态**：单机无流量 → 合并 main + 本机 `bun run tauri dev` 实测
- **合并**：fast-forward 至 `1818d0a`（已执行）
- **回滚点**：`7d2924e`（合并前 main HEAD），必要时 `git revert 1818d0a`
- **门槛**：G 系列本机验证通过率 ≥ 99%、无 P0 阻断缺陷

## 本机验证清单（G1~G4）

- [ ] G1：Codex 表单 `model_provider` 输入 ollama/lmstudio 点保存被拦截并提示，其他值正常保存
- [ ] G2：opencode/hermes live 配置含多个 provider 时，「从 live 配置导入」弹出选择列表，选中后导入对应条目（检查 config_json 的 `_silk_provider_id`）
- [ ] G3：单一 provider 时无弹窗直接导入（回归 iteration-003 行为）
- [ ] G4：在模型/渠道页增删模型映射后，切回预设管理页模型下拉与激活 Profile 的「模型不存在」提示自动刷新（无需重新激活）

## 结果

- 状态：**PASSED（待本机验证 G1~G4 确认）**
- 缺陷：0 个
- 遗留：G1~G4 本机验证由用户在 `bun run tauri dev` 环境确认；通过后本记录升级为最终验收，失败则 `git revert 1818d0a` 并走 fst-change

## 交接

- 验收通过 → fst-iterate 回顾闭环（回顾报告已生成，状态文件已更新至 review_handoff）
- 下轮候选（iteration-005）：MCP 各 harness 实测、模型池联动跨视图推送、导入选择弹窗摘要优化
