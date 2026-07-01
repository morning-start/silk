# CLAUDE.md

指导 AI 助手在此仓库中工作时遵循的约定与上下文。

## 项目

**Silk（丝路）** — 纯本地桌面 AI 多模型中转/网关客户端。提供 `http://127.0.0.1:2013` 统一端点，桥接三种 LLM 协议（OpenAI Chat、Claude Messages、OpenAI Response），入站协议双向转换。数据纯本地，无云、无服务端组件。

核心定位：**单人单用户、桌面端本地**。不需要分布式、多用户、容器化部署。

## 当前状态

代码审查报告见 `docs/code-review.md`。综合评分 8.8/10（在单人本地约束框内 9.0/10）。

### 已实现（Rust 后端）
- **Axum HTTP 网关** — 9 阶段中间件管道 + 三级失败回退（重试 → 换 Key → 换 Provider）
- **协议适配器** — `ProviderAdapter` trait（3 方法）+ `AdapterRegistry` + 3 种内置适配器，`build_upstream<T>()` 泛型复用
- **SSE 流式** — SSE 解析、心跳、Last-Event-ID 重连
- **SQLite 持久化** — 8 个 Repo，WAL 模式，异步批量写日志
- **负载均衡** — `LoadBalancer<T>` 通用选择器，支持 RoundRobin / Weighted / LeastConn（连接追踪）/ Failover
- **Provider 缓存** — TTL 5 分钟内存缓存 + 手动失效
- **路由管理** — Host/Path/Method/ContentType 四维匹配 + 分组负载均衡
- **加密** — AES-GCM 加密 API Key，SHA-256 哈希网关 Key

### 已实现（Vue3 前端）
- **10 个视图**：Dashboard、Providers、RoutingRules、Groups、Settings、Logs、Analytics、Monitoring、ModelSquare、Debugger
- **Pinia Stores**：gateway、providers (SWR)、routingRules (SWR)、groups (SWR)、logs
- **SWR 缓存**（`useSwrCache`）：30s TTL，避免视图切换重复拉取
- **跨 Store 联动**（`useCrossStoreNotify`）：Dashboard 监听 providers/routingRules/groups 变更自动刷新
- **全局错误边界**（`AppContent.vue`）：`onErrorCaptured` 捕获子组件渲染错误，显示降级 UI
- **IPC 层**：统一 `api/index.ts`，~50 个 Tauri invoke 方法

## Commands

```bash
bun install                   # 安装前端依赖
bun run dev                   # Vite 开发服务器 (port 1420)
bun run build                 # Type-check (vue-tsc) + 生产构建
bun run tauri dev             # 完整 Tauri 开发（前端 + Rust 编译）
bun run tauri build           # 构建生产安装包 (.msi/.dmg/.deb)

# Rust 检查（在 src-tauri/ 内执行）
cargo check -p silk           # Rust 类型检查
cargo clippy                  # Rust 代码风格检查
cargo test                    # 运行 Rust 测试
```

## 架构

```
Vue3 + NaiveUI (UI 层)
    ↕ Tauri IPC (invoke)
commands/ (Tauri 胶水层, ~150 行)
    ↕
application/ (业务服务层)
    ├── gateway_service    网关控制（start/stop/restart/status）
    ├── provider_service   渠道 CRUD + 测试
    ├── routing_service    路由规则 CRUD
    ├── settings_service   网关设置 CRUD (broadcast 通知重启)
    ├── group_service      分组 CRUD + 成员管理
    └── model_fetcher      远程模型列表获取
    ↕
gateway/ (Axum HTTP 网关核心)
    ├── context.rs          GatewayContext + RequestContext(Inner) + ProviderCache + RouteManager
    ├── pipeline.rs         9 阶段中间件管道 (含三级回退)
    ├── middleware/          10 个模块（9 阶段 + 工具函数）
    ├── group_manager.rs    分组负载均衡 + 连接追踪
    ├── load_balancer.rs    通用负载均衡器 (RoundRobin/Weighted/LeastConn/Failover)
    ├── log_cost.rs         cost 计算（消费侧异步）
    └── log_cleanup.rs      日志定时清理
    ↕
protocol/ (协议适配器)
    ├── adapter.rs          ProviderAdapter trait + build_upstream<T>() 泛型 + 共享工具
    ├── builtin_adapters.rs 内置适配器注册入口
    ├── registry.rs         适配器注册表
    └── adapters/           OpenAI Chat / Claude / OpenAI Response
    ↕
persistence/ (SQLite Repo 层, via SQLx)
    ├── provider_repo       渠道 CRUD
    ├── routing_rule_repo   路由规则（按优先级排序）
    ├── group_repo          分组 + 成员管理
    ├── log_repo            日志批量插入 + 统计查询
    ├── gateway_settings_repo 单例设置 CRUD
    ├── gateway_key_repo    网关 Key 哈希查找
    ├── model_mapping_repo  模型费用查询
    └── stats_repo          仪表盘统计聚合
    ↕
models/ (数据模型)
    ├── provider.rs         渠道（含 keys JSON 字段 + AES-GCM 加密）
    ├── gateway_settings.rs 设置（含 NetworkConfig / RateLimitConfig 语义子类型）
    ├── routing_rule.rs     路由规则（四维匹配 + inbound/outbound 协议）
    └── request_log.rs      请求日志（含 cost、token 用量）
```

### 核心数据流（9 阶段管道）

```
外部 AI 工具 → Axum 网关 (127.0.0.1:2013)
    ↓
extract         → 生成 request_id、读取请求体（2MB 限制）
    ↓
authenticate    → Bearer / x-api-key 认证
    ↓
resolve_route   → 四维匹配 + 分组路由 → 找到 Provider（注入 adapter_registry）
    ↓
select_channel  → 从 Provider 选择 API Key（排除已失败 Key）
    ↓
transform_request → build_upstream<T>() 泛型协议转换
    ↓
dispatch_upstream → 转发上游（含重试 + 退避 + SSE 重连 + LeastConn 连接追踪）
    ↓
transform_response → 适配器出站→入站协议转换
    ↓
persist_log     → 异步日志写入（channel 批处理）+ 消费侧 cost 计算
    ↓
finalize        → 构建最终 HTTP 响应
```

### 三级失败回退

```
dispatch_upstream 重试耗尽
    → select_channel 排除已失败 Key，选择下一个 Key（Level 2）
    → 所有 Key 失败：resolve_route::try_next_channel 选择下一个 Provider（Level 3）
    → 所有渠道失败：返回 502
```

### 关键设计决策

| 决策 | 原因 |
|------|------|
| Tauri 2（非 Electron） | 5-30MB 安装包 vs 150MB+，原生性能 |
| Axum（非 Actix/Salvo） | 最佳 Tokio/Tower 生态兼容性 |
| NaiveUI（非 Element Plus） | 更好的 TS 支持，更小，优秀主题系统 |
| SQLx + SQLite（非 redb/sled） | 需要 SQL 分页/过滤日志 |
| hyper-rustls（非 native-tls） | 纯 Rust TLS，无跨平台 OpenSSL 问题 |
| broadcast channel 解耦 | `settings_service` 广播变更事件，`lib.rs` 监听重启网关，避免循环依赖 |
| Inner + Deref 拆分 | `RequestContextInner` 自动 derive Clone，`response` 字段单独在外层 |

### RequestContext 设计

`RequestContext` 是管道核心数据载体。采用 **Inner + Deref** 模式：

- `RequestContextInner`：29 个字段，`#[derive(Clone)]`，通过 `Deref`/`DerefMut` 透明代理
- `RequestContext`：仅持 `response: Option<Response>`（axum::Response 非 Clone）
- `clone()` 时 `response` 置为 `None`，其余自动推导
- `adapter_registry` 为 `Option<Arc<AdapterRegistry>>`，在 `resolve_route` 阶段注入（延迟初始化）
- `selected_group_member`：路由到分组时存储选中的 `GroupMember`，用于 LeastConn 连接追踪

### 错误处理

`error.rs` — 结构化错误字段：

| 变体 | 字段 |
|------|------|
| `BadRequest` | `{ message: String, code: Option<String> }` |
| `Internal` | `{ message: String, detail: Option<String> }` |
| `NotFound` | `{ message: String }` |
| `Database` | `{ message: String }` |
| `DbNotInitialized` | (无字段) |

`GatewayError`（网关管道专用）支持上游错误透传：`UpstreamError { status, body }` 直接返回原始 HTTP 状态码和错误体。

### Header 处理

**入站认证**（`authenticate.rs`）：
- 支持多种认证方式：`Authorization: Bearer {key}`、`Authorization: Token {key}`、`x-api-key: {key}`、`X-API-Key: {key}`
- 根据请求头自动识别认证方式

**Header 转发**（`dispatch_upstream.rs`）：
- 使用 `HeaderConfig` 配置管理 header 转发
- 默认转发通用 header（`user-agent`、`accept` 等）
- 默认转发请求追踪 header（`x-request-id`、`x-trace-id` 等）
- 默认转发 AI 工具特定 header（`x-cursor-client-id`、`x-windsurf-version` 等）
- 默认排除认证和传输相关 header（`authorization`、`x-api-key`、`host`、`connection` 等）

## 约定

- **Vue**: 始终使用 `<script setup lang="ts">`（Composition API）
- **Tauri 2**: 从 `@tauri-apps/api/core` import `invoke()`（非 `@tauri-apps/api/tauri`）
- **TypeScript**: 严格模式 — `noUnusedLocals`、`noUnusedParameters` 开启
- **包管理器**: Bun，非 npm
- **注释/文档**: 中文用于文档和 UI 文本；代码注释中英文均可
- **中间件模式**: 每个阶段是 `middleware/` 下的独立模块，统一 `run(ctx) → Result` 签名
- **协议适配器**: 实现 `ProviderAdapter` trait，在 `builtin_adapters.rs` 注册。`transform_request` 使用 `build_upstream<T>()` 泛型复用反序列化→构建流程
- **错误处理**: `ServiceError` enum + `thiserror` derive，结构化字段（非裸 String）
- **前端 Store**: 使用 `useSwrCache`（30s TTL）+ `useCrossStoreNotify` 跨 Store 联动
- **负载均衡**: `LoadBalancer<T>` 通用选择器，新增条目需实现 `LoadBalancedItem` trait

## API 端点

| 方法 | 路径 | 处理 |
|------|------|------|
| GET | `/health` | `{"status": "ok", "service": "silk-gateway"}` |
| * | `/*` | `GatewayPipeline`（完整 9 阶段管道） |

## 数据库表

- `providers` — AI 服务商（API Key 用 AES-GCM 加密存储）
- `routing_rules` — 路由规则（四维匹配 + 优先级）
- `provider_groups` — 分组（加权轮询负载均衡）
- `group_members` — 分组-渠道关联（含 weight 权重）
- `request_logs` — 请求日志（复合索引：timestamp + provider_id + status_code + request_id）
- `gateway_settings` — 网关配置（单行单例）
- `gateway_keys` — 网关认证 Key（SHA-256 哈希存储）
- `model_mappings` — 模型费用映射

## 注意事项（常见陷阱）

- **不要添加分布式/多用户能力** — 单人单机桌面应用，评估架构建议时以此为首要约束
- **不要引入 Redis/消息队列** — 单进程内 Tokio channel 完全足够
- **不要引入云依赖** — 纯本地运行，SQLite 是持久化上限
- **修改中间件时检查 pipeline.rs** — 执行顺序和失败回退逻辑在 `run_main()` + `run_with_failover()` 中定义
- **新增适配器时**：创建文件 → `adapters/mod.rs` 加模块 → `builtin_adapters.rs` 注册（无需改 `registry.rs`）
- **设置变更流程**：`settings_service::update()` 发送 broadcast → `lib.rs` 中监听任务重启网关（不要直接调用 `gateway_service::restart()`）
- **新增负载均衡条目**：实现 `LoadBalancedItem` trait（`weight()` + `enabled()`），`LoadBalancer<T>` 自动处理选择逻辑
- **LeastConn 策略**：通过 `LoadBalancer::connection_started()`/`connection_finished()` 追踪活跃连接数，`dispatch_upstream` 在请求前后调用
