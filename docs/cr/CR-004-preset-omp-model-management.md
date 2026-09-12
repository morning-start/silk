# CR-004：预设模块增加 OMP 模型管理能力

- 状态：已批准并排期
- 提出人：用户（对话 IM，2026-09-12）
- 分级：中度
- 排期：本轮实现
- 执行方略：spec

## 原始需求

> 继续参考omp-switch，我想要silk的预设也增加管理omp的模型的能力。但是omp的模型配置和其他的有较大差别，需要仔细核对。

## 已确认范围

在 silk 预设模块中新增 **OMP** harness 支持：预设表单可配置 OMP provider 与结构化模型元数据，激活预设时投影写入 OMP 的 `models.yml`（providers.<id> 段）。参考 omp-switch 的 provider 管理语义（DB 为事实源、结构化模型定义、原子写、非破坏合并）。

**已确认的范围决策（用户确认）：**

1. **模型元数据表达：结构化模型编辑器** —— 模型字段带 cost（input/output/cacheRead/cacheWrite/contextOver200k）、contextWindow、maxTokens、reasoning、input 类型，前端新增结构化编辑（最贴合 OMP，非 json 透传）。
2. **config.yml 联动：仅 models.yml** —— 激活时只写 models.yml 的 `providers.<id>` 段，不写 config.yml（enabledModels/modelRoles/thinkingBudgets 本次不做）。
3. **模型来源：omp CLI 探测** —— 调用 `omp --list-models`（Windows: `omp`/`omp.cmd`，3 秒超时），未安装时优雅降级为空列表 + 提示。
4. **不含 thinking_level_map** —— 与 CR-003 决策一致，思考档位由 prism 侧任务负责（数据侧预留扩展空间）。

## 影响评估

- 数据库：不变。`presets.settings_config` 继续是配置快照的唯一持久化来源，无迁移。
- 前端：新增 `ompSpec` 表单定义（provider 基础字段 + 结构化模型编辑器）；若新增字段类型需同步 `PresetsView.vue` 渲染。
- 后端：新增 `harness/omp.rs`（OmpWriter 实现 HarnessWriter trait：live_path / write_live / remove_from_live / extract_*）；`harness/mod.rs` 注册 `writer_for("omp")`；复用现有原子写、快照回滚、`_silk_managed` 接管标记。
- 风险：YAML 读写（serde_yaml 已有）；providers 映射与 opencode 同构可复用合并/剥离语义；`omp --list-models` 依赖本机 CLI，需降级处理。
- 影响点清单（供 fst-review 变更针对性测试）：
  - `harness/omp.rs`（新增）：models.yml 读写合并
  - `harness/mod.rs`：writer_for 注册 + registry 测试
  - `src/config/harnessForms.ts`：ompSpec + 结构化模型字段
  - `PresetsView.vue`：模型编辑器渲染（若新增字段类型）
  - 模型探测模块：`omp --list-models` 调用与降级

## 验收边界

1. 预设表单出现 OMP 类型；可编辑 provider 基础字段（id/name/baseURL/apiKey/api/headers）与结构化模型（cost/contextWindow/maxTokens/reasoning/input），保存后再次打开完整回显。
2. 激活 OMP 预设后，`models.yml` 的 `providers.<id>` 段被正确投影写入（原子写），不覆盖用户手动配置的未管理 provider。
3. 停用/切换后，旧预设声明的 provider 段被剥离，其他 provider 保留。
4. 未安装 omp CLI 时，模型探测降级为空列表且不报错。
5. `cargo check` + `cargo test` + `bun run build` 通过。
6. 仅写 models.yml，不触碰 config.yml。
