# iteration-007 验收记录

> 迭代范围：偿还 TD-004（批次 A：`run_with_failover` 拆解）+ TD-005（批次 B：前端域 API 导入迁移）。
> 验收时间：2026-09-08 · 验收方式：fst-review（N6）· 分支：`feat/iteration-007`（c742bfe..HEAD）
> 提交：0c9c4f4（批次 A）、80dd2fc（Gate 修复）、2c2f8c1（批次 B）、3bba9f5（iteration-006 验收记录定稿）
> 过程态产物：`.agent-workplace/iterations/iteration-007/`（dod-checklist / retrospective / tech-debt）

## 1. DoD 核销清单

| # | 项 | 必须 | 结果 | 证据 |
|---|---|---|---|---|
| 1 | 功能完成（含骨架冒烟通过） | ✅ | ✅ | 批次 A：`AttemptOutcome` + `attempt_upstream` 提炼，`run_with_failover` 瘦身为调度结构；批次 B：9 个文件迁移域导入，聚合对象删除；无未确认需求骨架 |
| 2 | 变更针对性测试通过 | ✅ | ✅ | 批次 A：cargo test 122/122 + 6 条日志标记 grep 核对一条不增不减 + diff 逐段审查（错误分类语义等价：429/401/403/503 提前返回，RetryKey/SwitchChannel 映射正确，原 `should_retry` 死代码移除属等价简化）；批次 B：vue-tsc + vite build 通过，聚合引用残留 grep 零命中 |
| 3 | 核心主干回归通过 | ✅ | ✅ | `cargo check -p silk` ✅；`cargo test`（含 doctest 1 条）122/122 ✅；`bun run build` ✅；`gateway_service` 单测（start_existing_gateway）通过 |
| 4 | 文档同步（PRD/设计文档） | ✅ | ✅ | docs/plan/iteration-007-plan.json + docs/task/iteration-007-P1-B1/P2-B2.json 落 `.agent-workplace/docs/` |
| 5 | 变更记录归档 | ✅ | ✅ | plan/task/retrospective/tech-debt 落 `.agent-workplace/iterations/iteration-007*`；`state/current-iteration.json` 更新为 review_handoff |
| 6 | 待定需求不纳入交付验收 | — | ✅ | 无待定需求骨架 |

## 2. 变更针对性测试（改动点 + 关联影响点）

- **批次 A（pipeline.rs，正确性敏感）**：
  - 行为等价性：`attempt_upstream` 逐段对照原内层循环体（select_channel → transform_request → before_upstream → dispatch_upstream → after_upstream → 错误分类），语句与顺序完全一致；
  - 换 Key / 换渠道决策语义不变：`has_available_keys` 判定、`failed_providers` 记录、`selected_api_key=None` 防复用均保留；
  - 6 条日志标记（失败回退总超时 / 达到最大尝试次数 / transform_request 完成 / 上游返回不可重试错误 / 尝试失败，准备换 Key / 所有渠道和 Key 均已失败）一条不增不减；
  - 回归：cargo test 122/122（含 pipeline 相关既有测试）。
- **批次 B（前端迁移）**：调用签名逐一映射（`api.xxx` → 域 API），invoke 命令名未变；`vue-tsc --noEmit` + vite build 通过；聚合引用 grep 零命中。

## 3. 核心回归

| 检查 | 结果 |
|---|---|
| `cargo check -p silk` | ✅ |
| `cargo test`（lib 122 + doctest 1） | ✅ 全部通过 |
| `bun run build`（vue-tsc + vite） | ✅ |
| `git diff c742bfe..HEAD`（14 文件，+246/−222） | ✅ 已逐段审查 |

## 4. 灰度方案（N7）

桌面单机应用，无灰度流量概念。放量决策等价于**验收通过 → 合并 `feat/iteration-007` → main**；合并后清理分支。回滚预案：合并为普通 merge，必要时 `git revert` 对应提交。

## 5. 缺陷清单

| 缺陷 | 等级 | 状态 |
|---|---|---|
| preset_service 导入测试过时（官方种子 + opencode 激活语义，13ce6fe 引入，与批次无关） | 轻微（测试） | ✅ 已修复（80dd2fc） |
| logging.rs doctest 缺 crate 路径无法编译 | 轻微（测试） | ✅ 已修复（80dd2fc） |

无 P0/P1 阻断缺陷。

## 6. 结论

**验收通过（passed）**。DoD 6 项全部核销；核心回归全绿；缺陷 0 阻断；批次 A 回退语义与批次 B 调用语义均经针对性验证。

下一步：合并 `feat/iteration-007` → main，进入 fst-iterate 回顾闭环（N8，retrospective 已生成），排 iteration-008（TD-002/003 + 可选 TD-006）。
