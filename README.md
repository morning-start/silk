# Silk 丝路

纯本地桌面 AI 多模型中转网关客户端。提供统一的本地 HTTP 端点 `http://127.0.0.1:xxxx`，桥接三大 LLM 协议范式：OpenAI Chat、Claude Messages、OpenAI Response。

## 功能特性

- **协议双向转换** — 三大 LLM 协议（OpenAI Chat / Claude Messages / OpenAI Response）任意转换，统一为 OpenAI Response 格式输出
- **本地网关** — 在本机启动 HTTP 服务，外部 AI 工具直连 127.0.0.1 即可使用
- **流式响应** — 完整支持 SSE 流式输出，实时返回生成内容
- **图片/视频 AI 透传** — 非 LLM 请求直接转发到上游提供方，无需额外配置
- **Provider 管理** — 支持多家 AI 服务提供商（OpenAI、Claude、通义千问等）统一管理
- **路由分组** — 按 Host/Path/Method/ContentType 匹配路由，支持 Provider 分组和负载均衡
- **本地存储** — 所有数据（日志、配置、Provider 信息）存于 SQLite，零云端依赖

## 技术栈

| 层级 | 技术 |
|------|------|
| UI | Vue 3 + TypeScript + NaiveUI + Tailwind CSS |
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
│  ├─ Group Repo           ├─ Log Repo                    │
│  └─ Gateway Settings     └─ Request Log                 │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│  Tauri Native Layer (tray, window, file I/O)            │
└─────────────────────────────────────────────────────────┘
```

### 网关中间件管道

请求通过 7 阶段管道处理，每个阶段由独立中间件执行：

```
                    ┌──────────────────┐
                    │   HTTP Request   │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  extract::       │
                    │  initialize()    │  生成 request_id，初始化 RequestContext
                    │  read_body()     │  读取请求体（限制 2MB）
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  resolve_route:: │  根据 Host/Path/Method/ContentType
                    │  run()           │  匹配路由规则，解析目标 Provider
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  normalize_      │  从路由规则获取 inbound/outbound
                    │  protocol::run() │  协议标签，标记到上下文
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  transform_      │  根据入站协议选择适配器
                    │  request::run()  │  解析请求为 Canonical 格式
                    └────────┬─────────┘  转换为上游请求格式
                             │
                    ┌────────▼─────────┐
                    │  dispatch_       │  转发到上游 Provider
                    │  upstream::run() │  自动判断流式/非流式
                    │                  │  SSE 断线重连（Last-Event-ID）
                    └────────┬─────────┘  指数退避重试
                             │
                    ┌────────▼─────────┐
                    │  transform_      │  根据出站协议转换响应
                    │  response::run() │  支持流式/非流式两种模式
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

通过 `ProviderAdapter` trait 实现协议转换，支持热插拔：

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn provider_type(&self) -> &'static str;

    /// Canonical Request → 上游请求
    async fn canonicalize_request(
        &self,
        req: &CanonicalRequest,
        provider: &Provider,
    ) -> Result<UpstreamRequest, ProtocolError>;

    /// 上游响应 → Canonical Response
    async fn parse_response(
        &self,
        resp: &UpstreamResponse,
    ) -> Result<CanonicalResponse, ProtocolError>;
}
```

**协议流程：**
```
入站请求 → Inbound Adapter → CanonicalRequest → Outbound Adapter → 上游请求
                                          ↓
上游响应 → Outbound Adapter → CanonicalResponse → Inbound Adapter → 入站响应
```

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
│   ├── App.vue                   # 根组件
│   ├── components/               # 可复用组件
│   ├── stores/                   # Pinia 状态管理
│   └── views/                    # 页面视图
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── lib.rs                # Tauri 命令入口
│   │   ├── main.rs               # 应用入口
│   │   ├── error.rs              # 全局错误类型
│   │   ├── crypto/               # 加密模块 (AES-GCM)
│   │   ├── gateway/              # HTTP 网关
│   │   │   ├── mod.rs            # 服务器启动
│   │   │   ├── pipeline.rs       # 中间件管道
│   │   │   ├── context.rs        # 请求/网关上下文
│   │   │   ├── error.rs          # 网关错误类型
│   │   │   ├── group_manager.rs  # Provider 分组管理
│   │   │   └── middleware/       # 中间件实现
│   │   │       ├── extract.rs              # 请求提取
│   │   │       ├── resolve_route.rs        # 路由解析
│   │   │       ├── normalize_protocol.rs   # 协议归一化
│   │   │       ├── transform_request.rs    # 请求转换
│   │   │       ├── dispatch_upstream.rs    # 上游转发
│   │   │       ├── transform_response.rs   # 响应转换
│   │   │       ├── stream_response.rs      # SSE 流处理
│   │   │       ├── persist_log.rs          # 日志持久化
│   │   │       └── finalize.rs             # 响应构建
│   │   ├── protocol/             # 协议转换
│   │   │   ├── mod.rs            # 协议模块入口
│   │   │   ├── adapter.rs        # 适配器 trait
│   │   │   ├── registry.rs       # 适配器注册表
│   │   │   ├── canonical.rs      # Canonical 格式定义
│   │   │   └── adapters/         # 协议适配器实现
│   │   │       ├── openai_chat.rs         # OpenAI Chat
│   │   │       ├── claude.rs              # Claude Messages
│   │   │       └── openai_response.rs     # OpenAI Response
│   │   ├── persistence/          # SQLite 持久化
│   │   │   ├── mod.rs
│   │   │   ├── provider_repo.rs
│   │   │   ├── routing_rule_repo.rs
│   │   │   ├── group_repo.rs
│   │   │   ├── log_repo.rs
│   │   │   └── gateway_settings_repo.rs
│   │   ├── models/               # 数据模型
│   │   │   ├── mod.rs
│   │   │   ├── provider.rs
│   │   │   ├── routing_rule.rs
│   │   │   ├── provider_group.rs
│   │   │   ├── request_log.rs
│   │   │   └── gateway_settings.rs
│   │   └── commands/             # Tauri IPC 命令
│   │       ├── mod.rs
│   │       ├── providers.rs
│   │       ├── routing_rules.rs
│   │       ├── groups.rs
│   │       ├── logs.rs
│   │       ├── settings.rs
│   │       └── gateway.rs
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
