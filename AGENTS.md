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
- **无前端测试框架**，无 CI/CD 配置
- **中间件**: `middleware/` 下独立模块，统一 `run(ctx) → Result` 签名
- **新增协议适配器**: 实现 `ProviderAdapter` trait → `adapters/mod.rs` 加模块 → `builtin_adapters.rs` 注册（无需改 `registry.rs`）
- **新增负载均衡条目**: 实现 `LoadBalancedItem` trait（`weight()` + `enabled()`）

## Constraints

- **单人单机桌面应用** — 不要引入分布式、多用户、Redis、消息队列、云依赖
- **设置变更**: `settings_service::update()` 发送 broadcast → `lib.rs` 中的监听任务重启网关（不要直接调 `gateway_service::restart()`）
- **API Key**: AES-GCM 加密存储；网关 Key: SHA-256 哈希
- **修改中间件**: 先检查 `pipeline.rs` 中 `run_main()` 和 `run_with_failover()` 的执行顺序
- **数据库迁移**: `src-tauri/migrations/`（22 个文件，启动时自动执行 via `sqlx::migrate!`）
- **SQLx**: `.sqlx/` 目录包含离线查询缓存（`cargo sqlx prepare` 生成）

## Key Files

| File | Purpose |
|------|---------|
| `src/main.ts` | Vue 入口 |
| `src/api/index.ts` | ~50 Tauri invoke 封装 |
| `src-tauri/src/lib.rs` | Tauri 初始化、DB 迁移、命令注册、网关生命周期 |
| `src-tauri/src/main.rs` | 调用 `silk_lib::run()` |
| `src-tauri/src/gateway/pipeline.rs` | 9 阶段管道编排 + 三级回退逻辑 |
| `src-tauri/src/gateway/context.rs` | GatewayContext + RequestContext (Inner+Deref 模式) |
| `src-tauri/src/protocol/` | 协议适配层（adapter.rs, registry.rs, builtin_adapters.rs, adapters/） |
| `vite.config.ts` | Vite 配置 |

See [CLAUDE.md](./CLAUDE.md) for detailed architecture reference.
