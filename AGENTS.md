# Silk (丝路)

纯本地桌面 AI 多模型中转网关 (Tauri 2 + Vue 3 + Rust/Axum + SQLite)。

## Commands

```bash
bun install               # 安装前端依赖
bun run dev               # Vite dev server (port 1420)
bun run build             # vue-tsc --noEmit && vite build
bun run tauri dev         # 完整 Tauri 开发环境
bun run tauri build       # 生产安装包 (.msi/.dmg/.deb)

# 在 src-tauri/ 目录下执行
cargo check -p silk       # Rust 类型检查
cargo clippy              # Rust lint
cargo test                # Rust 测试
```

## Architecture

```
Vue3 + NaiveUI → Tauri IPC (invoke) → commands/ → application/ services
    → gateway/ (Axum 9-stage pipeline) / protocol/ (adapters) / persistence/ (SQLx + SQLite)
```

HTTP 网关 `127.0.0.1:2013` 核心：**9 阶段中间件管道** — extract → authenticate → resolve_route → select_channel → transform_request → dispatch_upstream → transform_response → persist_log → finalize

**3 级失败回退**：重试耗尽 → 换 Key → 换 Provider → 502

## Conventions

- **包管理器**: Bun，非 npm（`bun.lock` 是唯一锁文件，`package-lock.json` 是残留文件）
- **Tauri 2 IPC**: `import { invoke } from '@tauri-apps/api/core'`（非 `@tauri-apps/api/tauri`）
- **Vue**: `<script setup lang="ts">`, Composition API
- **TS strict**: `noUnusedLocals`, `noUnusedParameters` 开启（`vue-tsc --noEmit` 在 build 时运行）
- **Frontend Store**: `useSwrCache` (30s TTL) + `useCrossStoreNotify`
- **UI 文本**: 中文；代码注释中英文均可
- **前端工具函数**: `src/utils/` 下独立文件，如 `formatMs()`（耗时格式化）
- **无前端测试框架**，无 CI/CD 配置
- **中间件**: `middleware/` 下独立模块，统一 `run(ctx) → Result` 签名
- **新增协议适配器**: 实现 `ProviderAdapter` trait → `adapters/mod.rs` 加模块 → `builtin_adapters.rs` 注册（无需改 `registry.rs`）
- **新增网关插件**: 实现 `GatewayPlugin` trait → `gateway/plugins/mod.rs` 注册并实现生命周期拦截钩子（`before_route`, `before_upstream`, `after_upstream`）
- **新增负载均衡条目**: 实现 `LoadBalancedItem` trait（`weight()` + `enabled()`）

## 架构决策门禁（Architecture Decision Gate）

在对代码架构做任何修改（重构、分层、抽象、设计模式引入）之前，必须走完以下三步。这是强制门禁，不允许跳过。

### 门禁第一步：定位检查

回答三个问题，有一个不满足则终止架构改动：

1. **这个项目的本质是什么？**
   → 单人桌面本地应用。不是团队项目、不是微服务、不是 SaaS。
2. **谁在维护？**
   → 一个人。没有多人协作、没有代码审查流水线、没有新人 onboarding。
3. **什么情况下这个改动有实际收益？**
   → 区分"理论收益"和"实际收益"。如果收益场景在当前定位下永远不会发生（如单元测试覆盖、长周期团队交接），则收益不成立。

### 门禁第二步：收益计算

```
改动收益 = 可避免的未来成本 - 本次改动成本
```

- **只算确定会发生的未来成本**。如果某个未来成本（如"要写单元测试"）在当前项目定位下不是确定事件，不能算入收益。
- **改动成本包括**：新文件、函数签名变更、调用点修改、不一致引入、review 负担。
- **只有收益 ≥ 成本时才能改**。

### 门禁第三步：场景推演

用"如果...会怎样"测试假设，至少测试三个场景：

1. 如果明天加类似功能，改动更方便了还是更麻烦了？
2. 如果半年后完全不记得代码结构，能凭直觉找到改哪里吗？
3. 如果上级要求解释"为什么这样设计"，能给出**项目定位层面的理由**，而不只是"因为分层架构要求这样"吗？

### 强制回避清单

在单人桌面应用场景下，以下行为**默认禁止**，除非有极其充分的理由（在注释中写明原因）：

- ❌ 引入 trait-based service 层（不需要可替换实现）
- ❌ 为"理论可测试性"引入纯函数参数传递（收益不兑现）
- ❌ 将完整的用例拆成多层抽象（如 CSV 导出拆成"查询 + 格式化 + 写入"三层）
- ❌ 为了追逐"最新架构模式"而重构（如引入 CQRS、Event Sourcing、Actor Model）
- ✅ **优先选择**：与现有代码保持一致的简单方案

### 一句话原则

**架构不是把代码放进正确的文件夹，而是知道什么时候不该放。** 好的架构让明天的你能记住代码在哪改，而不是让今天的你为一个永远不会来的"大型分布式团队"做准备。

## Constraints

- **单人单机桌面应用** — 不要引入分布式、多用户、Redis、消息队列、云依赖
- **设置变更**: `settings_service::update()` 发送 broadcast → `lib.rs` 中的监听任务重启网关（不要直接调 `gateway_service::restart()`）
- **API Key**: AES-GCM 加密存储；网关 Key: SHA-256 哈希
- **非流式不支持**: 网关强制所有请求走流式 SSE（`transform_request` 注入 `stream: true`），`dispatch_upstream` 统一使用流式客户端，`transform_response` 纯透传（SSE 在后台任务中逐事件协议转换）。客户端如果期望非流式响应会收到 SSE 流，需要注意这个前提。
- **修改中间件**: 先检查 `pipeline.rs` 中 `run_main()` 和 `run_with_failover()` 的执行顺序，注意插件钩子执行点
- **数据库迁移**: `src-tauri/migrations/`（22 个文件，启动时自动执行 via `sqlx::migrate!`）
- **SQLx**: `.sqlx/` 目录包含离线查询缓存（`cargo sqlx prepare` 生成）

## Key Files

| File | Purpose |
|------|---------|
| `src/main.ts` | Vue 入口 |
| `src/utils/` | 前端工具函数（`formatMs` 等） |
| `src/api/index.ts` | ~50 Tauri invoke 封装 |
| `src-tauri/src/lib.rs` | Tauri 初始化、DB 迁移、命令注册、网关生命周期 |
| `src-tauri/src/main.rs` | 调用 `silk_lib::run()` |
| `src-tauri/src/gateway/pipeline.rs` | 9 阶段管道编排 + 三级回退逻辑 + 插件钩子分发 |
| `src-tauri/src/gateway/context.rs` | GatewayContext + RequestContext (Inner+Deref 模式) |
| `src-tauri/src/gateway/plugin.rs` | `GatewayPlugin` 生命周期拦截钩子定义 |
| `src-tauri/src/gateway/plugins/mod.rs` | 内置 Token 节省插件列表（Prompt缓存、滑动窗口、日志压缩） |
| `src-tauri/src/protocol/` | 协议适配层（adapter.rs, registry.rs, builtin_adapters.rs, adapters/） |
| `vite.config.ts` | Vite 配置 |
