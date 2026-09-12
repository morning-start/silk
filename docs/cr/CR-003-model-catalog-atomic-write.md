# CR-003：内置模型目录（模型池元数据补充）+ 配置原子写入与外部监听

- 状态：已批准并排期
- 提出人：用户（对话 IM，2026-09-12）
- 分级：中度
- 排期：本轮实现
- 执行方略：spec

## 原始需求

> 1. 内置模型目录是可以的，可以收集场景信息为一个json或toml或者csv或者db数据库都可以，但是要文件存储，不能代码中。只对模型池的模型补充模型元数据，渠道的不要。
> 2. 原子写入可以有、外部监督可以有
> 思考了档位是不是应prism实现？调用fst实现

## 已确认范围

**1. 内置模型目录（模型池元数据补充）**

- 元数据以**独立文件**存储（JSON 格式，`serde_json` 现成依赖），不嵌入代码、不硬编码 Rust 常量。
- 文件位置：应用数据目录 `model-catalog.json`；种子文件作为 Tauri 打包资源（`bundle.resources`），首次启动时复制到数据目录，用户可手动编辑。
- Schema v1 字段：`model_name`（主键）+ `max_input_tokens` / `max_context_tokens` / `max_output_tokens` / `capabilities` / `description` / `vendor` / `knowledge_cutoff` / `model_family` / `reference_url` / `reasoning` / `input_types`。
- **不含 `thinking_level_map`**：设计决策为「思考档位由 prism 实现」（另立任务），本次不预留该字段。
- **只对模型池补充**：合并点 `ModelMappingResponse`（model_mappings 的响应），DB 字段有值优先、空值从目录补充；**渠道（Provider）模型完全不碰**。

**2. 配置原子写入**

- `GatewaySettings::save` 由 `std::fs::write` 改为复用现有 `config_writer::write_text_atomic`（临时文件 + 重命名）。

**3. 外部监督（watcher）**

- 新增 `notify` 依赖，监听应用数据目录下 `gateway.json` 与 `model-catalog.json` 的外部修改。
- `gateway.json` 外部编辑 → 重载设置并在网关运行中自动重启（复用现有 `settings_change_tx` broadcast 机制）。
- `model-catalog.json` 外部编辑 → 重载目录缓存，模型池响应即时反映。
- 应用自身写入不触发循环重启（去抖 + 内容比对）。

**4. 设计决策确认**

- 「思考档位是否应由 prism 实现」→ **是**：档位是语义概念，各协议表达不同（OpenAI `reasoning_effort` / Anthropic `budget_tokens` / Gemini `thinkingLevel`），翻译属协议转换职责，由 prism `convert_request` 实现；数据（每模型支持档位）未来放目录文件。本次不实施。

## 影响评估

- 数据库：不变。`model_mappings` 表结构不动，目录仅做响应层补充。
- 前端：若响应结构新增字段（如 `reasoning`/`input_types`），需同步类型定义；无字段新增则零改动。
- 后端：新增 `application/model_catalog.rs`（加载/缓存/合并）与 watcher 模块；修改 `gateway_settings.rs`、`lib.rs`、`tauri.conf.json`。
- 风险：低。DB 有值优先保证不覆盖用户数据；原子写仅换写盘方式；watcher 只监听不主动写。
- 影响点清单（供 fst-review 变更针对性测试）：
  - `model_mapping_service.rs` 响应合并
  - `gateway_settings.rs::save` 原子写
  - 新增 `model_catalog.rs` + 种子资源文件
  - 新增 watcher 模块 + `Cargo.toml` 增加 `notify`
  - `lib.rs` 初始化目录 + 拉起 watcher
  - `tauri.conf.json` `bundle.resources` 打包种子

## 验收边界

1. `model-catalog.json` 存在时，模型池列表中 DB 空字段被目录补全；缺失/解析失败时模型池响应与现状一致（不报错、不崩溃）。
2. 渠道模型元数据完全不受目录影响。
3. `gateway.json` 保存走原子写（临时文件 + 重命名）。
4. 外部修改 `gateway.json` → 网关运行中自动重启生效；应用自身保存不触发循环重启。
5. 外部修改 `model-catalog.json` → 模型池响应即时反映新元数据。
6. `cargo check` 与 `bun run build` 通过。
