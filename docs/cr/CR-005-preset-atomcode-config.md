# CR-005：预设模块增加 AtomCode 配置管理能力

- 状态：已批准并排期
- 提出人：用户（对话 IM，2026-09-13）
- 分级：中度（用户已确认）
- 排期：本轮实现（用户已确认）
- 需求来源：https://atomcode.atomgit.com/docs/zh/configuration.html

## 原始需求

> https://atomcode.atomgit.com/docs/zh/configuration.html 这个配置文件怎么添加到预设中，参考已有的预设设计一下 fst

## 已确认范围

在 silk 预设模块中新增 **AtomCode** harness 支持：预设表单可配置 AtomCode provider，激活预设时写入 `~/.atomcode/config.toml`（`[providers.silk]` 表 + `default_provider = "silk"` 顶层字段），使 AtomCode CLI 的流量经 silk 网关中转。

**范围决策（基于文档与现有架构，随变更单确认）：**

1. **单激活语义**——`default_provider` 是单值顶层字段，切 provider 即改指向；对齐 claude_code/codex 模式，**不做累加**（区别于 omp/opencode）。
2. **只写旧版 `[providers.*]` 结构**——官方声明 v5.0.7 新版账号结构自动兼容旧配置；silk 不写 `/provider` 账号模型。
3. **api_key 透传不解析**——AtomCode 自带 `$VAR`/`${VAR:-default}`/env 名/留空回退链（`OPENAI_API_KEY`/`ANTHROPIC_API_KEY`/`ATOMCODE_API_KEY`），silk 原样写入用户填写的值。
4. **高级字段走 advanced JSON 兜底**——`vision_preprocessor_provider`、`[network.proxy]`、`[coding]`、`[tools.todo]` 等低频配置不做结构化。
5. **协议类型固定映射**——`type` 支持 `openai`/`claude`/`ollama`（文档全集）；silk 场景默认 `openai` + `base_url = http://127.0.0.1:1877/v1`。

## 影响评估

- 数据库：**一次新增迁移**——`presets` 表 `agent_type` CHECK 白名单加 `'atomcode'`（策略：修改未发布的 CR-004 迁移 or 新增一条，实现时按发布状态决策；**LF 行尾强制**）。
- 后端：新增 `harness/atomcode.rs`（AtomcodeWriter 实现 HarnessWriter trait：live_path = `~/.atomcode/config.toml`（`ATOMCODE_HOME` env 优先）、write_live/remove_from_live/extract_*；TOML 读写）；`harness/mod.rs` 注册 `writer_for("atomcode")`；`agent_type.rs` 补 `requires_restart()` 分支；复用原子写、快照回滚、`_silk_managed` 接管标记。
- 依赖：可能需新增 `toml` crate（确认依赖树，tauri 生态大概率已有）。
- 前端：`harnessForms.ts` 新增 `atomcodeSpec`（provider/type/api_key/model/base_url/context_window/max_tokens）；`PresetsView.vue` 注册 Tab（单激活按钮，复用现有逻辑）。
- 风险：本机有真实 `~/.atomcode/config.toml`——**测试必须用 env 隔离 + ENV_LOCK 串行**（omp 事故同款防线，绝不触碰真实配置）。
- 影响点清单：见 `impact-CR-005.md`（8 项，含迁移、注册、单激活切换、测试隔离、回归）。

## 验收边界

1. 预设表单出现 AtomCode 类型；可编辑 provider 字段（type/api_key/model/base_url/context_window/max_tokens），保存后再次打开完整回显。
2. 激活 AtomCode 预设后，`config.toml` 出现 `[providers.silk]` 段且 `default_provider = "silk"`（原子写）；不破坏用户已有 provider 段与未管理配置。
3. 单激活切换：激活预设 B 后 `default_provider` 指向 B；取消语义对齐 claude_code（单激活无"取消激活"按钮）。
4. 停用/删除预设后 silk provider 段被剥离，`default_provider` 回退安全值（原指向或删除字段）。
5. cargo test 全量通过（当前 164 基线）+ bun run build 通过；本机 atomcode 实测能以 silk 网关启动对话。

## 边界（不做）

- 不实现 `/provider` v5.0.7 账号结构写法
- 不做 vision_preprocessor/network.proxy/coding/todo 结构化（advanced JSON 兜底）
- 不做多 provider 并存管理
