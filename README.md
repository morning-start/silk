# Silk 丝路

纯本地桌面 AI 多模型中转网关客户端。提供统一的本地 HTTP 端点 `http://127.0.0.1:xxxx`，桥接三大 LLM 协议范式：OpenAI Chat、Claude Messages、OpenAI Response。

## 功能特性

- **协议双向转换** — 三大 LLM 协议（OpenAI Chat / Claude Messages / OpenAI Response）任意转换，统一为 OpenAI Response 格式输出
- **本地网关** — 在本机启动 HTTP 服务，外部 AI 工具直连 127.0.0.1 即可使用
- **流式响应** — 完整支持 SSE 流式输出，实时返回生成内容
- **图片/视频 AI 透传** — 非 LLM 请求直接转发到上游提供方，无需额外配置
- **Provider 管理** — 支持多家 AI 服务提供商（OpenAI、Claude、通义千问等）统一管理
- **路由规则** — 按 Host/Path/Method/ContentType 匹配路由，支持模型映射优先、Provider 回退和故障转移
- **本地存储** — 所有数据（日志、配置、Provider 信息）存于 SQLite，零云端依赖

## 技术栈

| 层级 | 技术 |
|------|------|
| UI | Vue 3 + TypeScript + NaiveUI |
| 状态管理 | Pinia |
| 桌面框架 | Tauri 2 |
| 后端 | Rust + Axum + Tokio |
| 数据库 | SQLite (SQLx) |
| HTTP 客户端 | Reqwest (rustls-tls) |
| SSE 处理 | tokio-stream + bytes |
| 加密存储 | AES-GCM + PBKDF2 |
| 包管理 | Bun |

## 架构设计

### 5 层架构

```
┌─────────────────────────────────────────────────────────┐
│  Vue3 + NaiveUI + Tailwind (UI Layer)                   │
└─────────────────────┬───────────────────────────────────┘
                      │ Tauri IPC (invoke / events)
┌─────────────────────▼───────────────────────────────────┐
│  Rust Backend                                           │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Axum HTTP Gateway (127.0.0.1:port)               │  │
│  │  ├─ Health Check: /health                         │  │
│  │  └─ Fallback: GatewayPipeline                     │  │
│  └───────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Protocol Converters (Protocol Adapter Layer)     │  │
│  │  ├─ OpenAI Chat Adapter                           │  │
│  │  ├─ Claude Messages Adapter                       │  │
│  │  └─ OpenAI Response Adapter (Canonical Format)    │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────┬───────────────────────────────────┘
                      │ async functions
┌─────────────────────▼───────────────────────────────────┐
│  Persistence (SQLite via SQLx)                          │
│  ├─ Provider Repo        ├─ Routing Rule Repo           │
│  ├─ Gateway Key Repo     ├─ Log Repo                    │
│  └─ Model Mapping Repo   └─ Gateway Settings / Stats    │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│  Tauri Native Layer (tray, window, file I/O)            │
└─────────────────────────────────────────────────────────┘
```

### 网关中间件管道（9 阶段 + 插件钩子）

请求通过 9 阶段管道处理，每个阶段由独立中间件执行，插件钩子在关键节点注入：

```
                    ┌──────────────────┐
                    │   HTTP Request   │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  extract::       │  生成 request_id，初始化 RequestContext
                    │  initialize()    │  读取请求体（限制 2MB）
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  authenticate::  │  验证 Gateway Key（仅 /v1/* 路径）
                    │  run()           │  支持 Bearer / x-api-key 两种方式
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  resolve_route:: │  模型映射优先 → 路由规则 → 路径兜底
                    │  run()           │  确定 provider + inbound/outbound 协议
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  [before_route]  │  插件钩子（如 Prompt 缓存检测）
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  select_channel  │  从 provider 的 Key 池中选择上游 Key
                    │  ::run()         │  支持轮询 / 加权 / 主备策略
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  transform_      │  根据入站/出站协议选择适配器
                    │  request::run()  │  转换请求体为上游期望格式
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  [before_        │  插件钩子（日志压缩、滑动窗口等）
                    │   upstream]      │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  dispatch_       │  转发到上游 Provider
                    │  upstream::run() │  自动判断流式/非流式
                    │                  │  SSE 断线重连、指数退避重试
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  transform_      │  根据出站协议转换响应
                    │  response::run() │  支持流式/非流式两种模式
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  [after_         │  插件钩子（响应统计等）
                    │   upstream]      │
                    └────────┬─────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
     ┌────────▼─────────┐          ┌────────▼─────────┐
     │  persist_log::   │          │  finalize::       │
     │  run()           │          │  success/failure  │
     │  异步写入 SQLite │          │  构建最终响应     │
     └──────────────────┘          └──────────────────┘
```

**三级失败回退**：`dispatch_upstream` 内部重试耗尽 → 换 Key（`select_channel` 排除已失败 Key）→ 换渠道（`resolve_route::try_next_channel`）→ 502

### 请求上下文 (RequestContext)

每个请求在整个管道中携带 `RequestContext`，记录完整的处理状态：

```rust
pub struct RequestContext {
    // 基础信息
    request_id: String,           // UUID 唯一标识
    started_at: Instant,          // 请求开始时间
    method: Method,               // HTTP 方法
    uri: Uri,                     // 请求 URI
    headers: HeaderMap,           // 请求头
    body: Bytes,                  // 请求体

    // 路由信息
    host: Option<String>,
    path: String,
    content_type: Option<String>,
    route: Option<RoutingRule>,   // 匹配的路由规则
    provider: Option<Provider>,   // 解析出的目标 Provider

    // 协议信息
    inbound_protocol: Option<String>,   // 入站协议类型
    outbound_protocol: Option<String>,  // 出站协议类型

    // 上游响应
    upstream_status: Option<StatusCode>,
    upstream_headers: Option<HeaderMap>,
    upstream_body: Option<Bytes>,

    // 错误信息
    final_status: Option<StatusCode>,
    error_message: Option<String>,
    error_code: Option<String>,

    // 流式支持
    response: Option<Response>,
    response_bytes_sent: u64,
    last_event_id: Option<String>,  // SSE 断线重连用
}
```

### 协议适配器

通过 `ProviderAdapter` trait 实现协议转换，直接操作 JSON，无中间格式：

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// 适配器类型标识（如 "openai_chat", "claude_messages", "openai_response"）
    fn provider_type(&self) -> &'static str;

    /// 将原始请求体（JSON 字节）转为上游请求
    async fn transform_request(
        &self,
        req_body: &[u8],
        provider: &Provider,
        selected_api_key: &str,
    ) -> Result<UpstreamRequest, ProtocolError>;

    /// 将上游响应转为客户端期望的格式（JSON）
    async fn transform_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<serde_json::Value, ProtocolError>;
}
```

**新增适配器**：实现 trait → 在 `adapters/mod.rs` 添加模块 → 在 `builtin_adapters.rs` 调用 `registry.register(Arc::new(MyAdapter))` 即可，无需改 `registry.rs`。

### SSE 流式处理

流式响应采用异步管道设计：

```
上游 SSE 流
    │
    ▼
┌─────────────────┐
│ SseParser       │  解析 SSE 事件，追踪 last_event_id
│ (后台读取任务)  │  检测 [DONE] 结束标记
└────────┬────────┘  心跳检测超时
         │
         ▼
┌─────────────────┐
│ mpsc::channel   │  256 容量缓冲
│ (逐 chunk 推送) │  支持背压
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ StreamBody      │  构建 axum Response
│ (主线程接收)    │  Body::from_stream()
└────────┬────────┘
         │
         ▼
    HTTP Response
```

**特性：**
- 心跳保活：每 15 秒发送 `: keep-alive\n\n`
- 断线重连：携带 `Last-Event-ID` 头
- 超时保护：30 秒无数据自动超时
- 指数退避重试：最多 3 次，退避 500ms → 8s

### Provider 缓存

Provider 信息通过 TTL 缓存（5 分钟），避免频繁数据库查询：

```rust
pub struct ProviderCache {
    inner: Arc<RwLock<HashMap<String, CachedProvider>>>,
    ttl: Duration,  // 默认 300s
}

// 获取 Provider（命中缓存直接返回）
async fn get(&self, id: &str) -> Option<Provider>;

// 写入缓存
async fn put(&self, provider: Provider);

// 使缓存失效（配置更新时调用）
async fn invalidate(&self, id: &str);
```

### 日志异步写入

通过 channel 异步批量写入，避免阻塞请求处理：

```
中间件 persist_log::run()
    │
    ▼
mpsc::channel (日志消息)
    │
    ▼
后台任务 spawn_log_writer()
    ├─ 批量收集（最多 50 条/批）
    ├─ 定时刷新（每 5 秒）
    └─ 批量写入 SQLite (insert_batch)
```

**日志字段：**
- 请求信息：request_id, method, path, headers, body
- 路由信息：route_id, inbound/outbound_protocol, provider_id
- 响应信息：status, headers, body, duration_ms
- 流式信息：stream_enabled, response_bytes_sent

## 开发环境

### 前置要求

- [Node.js](https://nodejs.org/) 18+
- [Bun](https://bun.sh/) 1.0+
- [Rust](https://rustup.rs/) 1.70+
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)

### 安装依赖

```bash
bun install
```

### 开发运行

```bash
bun run tauri dev
```

### 构建生产包

```bash
bun run tauri build
```

## 命令参考

```bash
# 前端开发
bun run dev              # Vite 开发服务器 (port 1420)
bun run build            # 类型检查 + 生产构建
bun run preview          # 预览生产构建

# Tauri
bun run tauri dev        # 完整开发环境 (前端 + Rust)
bun run tauri build      # 构建安装包 (.msi/.dmg/.deb)

# Rust (在 src-tauri/ 目录下)
cargo check              # 快速类型检查
cargo clippy             # Rust lint
cargo test               # 运行测试
```

## 项目结构

```
silk/
├── src/                          # Vue 前端源码
│   ├── App.vue                   # 根组件（主题、全局壳）
│   ├── AppContent.vue            # 布局 / 菜单 / 网关控制
│   ├── api/index.ts              # ~50 个 Tauri invoke 封装
│   ├── components/               # 可复用组件
│   ├── router/index.ts           # 页面路由
│   ├── stores/                   # Pinia 状态管理（useSwrCache + useCrossStoreNotify）
│   └── views/                    # 页面视图（8 个管理页面）
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── lib.rs                # Tauri 初始化、DB 迁移、命令注册、网关生命周期
│   │   ├── main.rs               # 调用 silk_lib::run()
│   │   ├── error.rs              # ServiceError（应用层统一错误类型）
│   │   ├── load_balancer.rs      # 负载均衡策略（round_robin / weighted / failover）
│   │   ├── crypto/               # AES-GCM 加密（API Key 存储）
│   │   ├── gateway/              # HTTP 网关
│   │   │   ├── mod.rs            # 服务器启动、日志写入任务
│   │   │   ├── pipeline.rs       # 9 阶段管道编排 + 三级回退 + 插件钩子分发
│   │   │   ├── context.rs        # GatewayContext + RequestContext (Inner+Deref 模式)
│   │   │   ├── error.rs          # GatewayError 枚举
│   │   │   ├── plugin.rs         # GatewayPlugin 生命周期拦截钩子定义
│   │   │   ├── header_config.rs  # Header 转发配置
│   │   │   ├── log_cleanup.rs    # 定时日志清理任务
│   │   │   ├── log_cost.rs       # Token 费用计算
│   │   │   ├── plugins/          # 内置网关插件
│   │   │   └── middleware/       # 中间件实现（11 个模块，含 stream_response）
│   │   │       ├── extract.rs              # 请求提取（initialize + read_body）
│   │   │       ├── authenticate.rs         # 网关 Key 认证
│   │   │       ├── rate_limit.rs           # IP 级限流
│   │   │       ├── resolve_route.rs        # 路由解析（模型映射→路由规则→路径兜底）
│   │   │       ├── select_channel.rs       # 上游 Key 选择
│   │   │       ├── transform_request.rs    # 请求协议转换
│   │   │       ├── dispatch_upstream.rs    # 上游转发（含 SSE + 重试）
│   │   │       ├── transform_response.rs   # 响应协议转换
│   │   │       ├── stream_response.rs      # SSE 流式处理（crate 内可见）
│   │   │       ├── persist_log.rs          # 异步日志写入
│   │   │       └── finalize.rs             # 构建最终响应
│   │   ├── protocol/             # 协议适配层
│   │   │   ├── adapter.rs        # ProviderAdapter trait + UpstreamRequest/Response
│   │   │   ├── registry.rs       # AdapterRegistry（注册 + 查找）
│   │   │   ├── builtin_adapters.rs  # 内置适配器注册（新增适配器改这里）
│   │   │   └── adapters/         # 协议适配器实现
│   │   │       ├── openai_chat.rs         # OpenAI Chat Completions
│   │   │       ├── claude.rs              # Claude Messages
│   │   │       └── openai_response.rs     # OpenAI Responses API
│   │   ├── application/          # 应用服务层
│   │   │   ├── gateway_service.rs
│   │   │   ├── provider_service.rs
│   │   │   ├── routing_service.rs
│   │   │   ├── settings_service.rs
│   │   │   ├── log_service.rs
│   │   │   ├── stats_service.rs
│   │   │   ├── gateway_key_service.rs
│   │   │   └── model_mapping_service.rs
│   │   ├── persistence/          # SQLite 持久化（Repo 模式）
│   │   ├── models/               # 数据模型
│   │   └── commands/mod.rs       # 所有 Tauri IPC 命令（按功能分组）
│   ├── migrations/               # SQLite 迁移文件（启动时自动执行）
│   └── Cargo.toml
├── docs/                         # 项目文档
└── public/                       # 静态资源
```

## 相关文档

- [API 使用指南](docs/API使用指南.md) — 网关使用、路由配置、Provider设置
- [开发者指南](docs/开发者指南.md) — 扩展适配器、添加中间件、数据模型
- [项目完整需求 & 定位](docs/规划/Silk（丝路）项目完整需求&定位&功能总结.md)
- [技术选型说明](docs/规划/规划使用库.md)

## 许可证

MIT
