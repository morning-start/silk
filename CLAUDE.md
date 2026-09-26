# Silk (丝路)

纯本地桌面 AI 多模型中转网关 (Tauri 2 + Vue 3 + Rust/Axum + SQLite)。

## Commands

```bash
bun install               # 安装前端依赖
bun run dev               # Vite dev server (port 1420)
bun run build             # vue-tsc --noEmit && vite build
bun run tauri dev         # 完整 Tauri 开发环境
bun run tauri build       # 生产安装包 (.msi/.dmg/.deb)
bun run bump:version x.x.x # 一键同步版本号 (scripts/bump-version.ts)

# src-tauri/ 目录下
cargo check -p silk       # Rust 类型检查
cargo clippy              # Rust lint
cargo test                # Rust 测试
```

## Architecture

```
Vue3 + NaiveUI → Tauri IPC (invoke) → commands/ → application/ services
    → gateway/ (Axum 9-stage pipeline) / protocol/ (prism.wasm) / persistence/ (SQLx + SQLite)
```

HTTP 网关 `127.0.0.1:1877`：**9 阶段管道** — extract → authenticate → resolve_route → select_channel → transform_request → dispatch_upstream → transform_response → persist_log → finalize

**3 级失败回退**：重试(3次) → 换 Key → 换渠道 → 错误返回（总超时15s，最多10次）
- 401/403/429/503 属**渠道级**错误（换 Key 无意义），跳过换 Key 直接换渠道，**不终止回退**
- 回退耗尽时优先返回**最后一次上游错误**（原样透传），而非 silk 自造的「所有渠道和 Key 均已失败」

## Prism 协议转换

prism.wasm (MoonBit 编译) 处理请求/响应/SSE 流的跨协议转换。

```
请求转换: inbound protocol → prism.convert_request() → outbound protocol
响应转换: outbound protocol → prism.convert_response() → inbound protocol
流式转换: outbound SSE → prism.convert_stream_event() → inbound SSE (逐事件)
```

**支持的协议**: openai, messages, responses, gemini (prism provider 名)

**映射**: silk 协议名 ↔ prism provider 名（`map_provider()` in `prism_wasm.rs`）

### 构建 & 更新 prism.wasm

**常规途径：应用内下载**（推荐）。「关于」页 →「协议内核」卡片可检查/下载官方发布的内核，
下载后自动做 SHA-256 校验 + ABI 探测，通过才替换，重启生效。

**手动构建**（本地调试 prism 改动时）：

```bash
cd E:\Workplace\APP\MoonBit\prism
moon clean && moon build --target wasm --release
# 输出: _build/wasm/release/build/cmd/main/main.wasm

# 复制到 AppData（silk 运行时加载）
copy _build\wasm\release\build\cmd\main\main.wasm %APPDATA%\morning-start.silk\prism.wasm
```

**加载优先级**（`prism_wasm::resolve_wasm_source`，三处同名 `prism.wasm`）：
1. 可执行文件同目录 —— 优先级最高；存在时会**抢占**加载权，应用内下载将拒绝安装并提示
2. 应用数据目录 —— 应用内下载的写入目标
3. 编译时内嵌（`src-tauri/prism.wasm`）—— 兜底，保证开箱即用

**ABI 探测**：内核通过 `wasm_abi_version()` 自报 ABI（当前为 `1`）。宿主在
`SUPPORTED_ABI` 声明兼容版本，安装前用 `prism_wasm::probe_abi()` 在独立实例中
实例化并探测，ABI 不匹配或无法实例化一律拒绝安装。

> ℹ️ **内嵌内核版本**：`src-tauri/prism.wasm` 当前为 prism v0.1.4
> （350,749 字节，sha256 `738dcbc7…46295`），**已含 `wasm_abi_version` 导出**，支持 ABI 探测。
> 早期版本（2026-09-21 前构建）不含该导出，此时「协议内核」卡片 ABI 显示「未知」并提示更新。
> 更新内嵌版本：重新构建 prism 并替换 `src-tauri/prism.wasm`，再重新打包应用。
> `prism_wasm.rs` 的 `test_current_abi_matches_supported` 会断言探测成功 —— 若内嵌内核退回旧版，该测试会失败。

### prism 调试

```bash
# 运行 prism 测试
cd E:\Workplace\APP\MoonBit\prism && moon test

# 查看转换日志（silk 日志中搜索）
grep "事件转换完成\|转换输出\|SSE 流式转换" silk.log
```

## 数据目录 & 文件位置

**AppData 目录**: `%APPDATA%\morning-start.silk\` (Windows) / `~/Library/Application Support/morning-start.silk/` (macOS)

| 文件/目录 | 说明 |
|-----------|------|
| `silk.db` | SQLite 数据库（请求日志、配置） |
| `gateway.json` | 网关配置（channels、providers、keys、端口等） |
| `api-key` | 本地 API Key（`sk-silk-xxx` 格式） |
| `prism.wasm` | 协议转换模块（优先于编译时嵌入版本） |
| `prism.release.json` | 已安装内核的构建清单（版本比对依据；由「协议内核」下载写入） |
| `prism.wasm.bak` | 内核替换前的备份。**与 `prism.release.json.bak` 成对写入**，成对还原 |
| `prism.release.json.bak` | 与 `prism.wasm.bak` 配对的版本记录（避免回滚后版本号指向不存在的内核） |
| `logs/` | 日志目录 |
| `logs/silk.log` | 当前日志文件 |
| `logs/silk.log.YYYY-MM-DD` | 按天轮转的历史日志 |

**前端配置**: Tauri Store (`tauri-plugin-store`) 存储 UI 设置

## 日志系统

**日志级别** (gateway.json 中配置):
- `log_level`: 控制台输出级别（默认 info）
- `file_level`: 文件输出级别（默认 debug）
- `log_modules`: 模块级覆盖，如 `{"silk_lib::gateway::middleware::dispatch_upstream": "debug"}`

**日志格式**: 每行包含 `时间戳 级别 线程ID 模块路径:行号 消息`

```bash
# 查看实时日志
Get-Content "$env:APPDATA\morning-start.silk\logs\silk.log" -Wait

# 查看最近50行
Get-Content "$env:APPDATA\morning-start.silk\logs\silk.log" -Tail 50

# 搜索错误
Select-String -Path "$env:APPDATA\morning-start.silk\logs\silk.log" -Pattern "ERROR|WARN"

# 搜索特定请求（按 request_id）
Select-String -Path "$env:APPDATA\morning-start.silk\logs\silk.log" -Pattern "request_id=xxx"

# 搜索流式事件
Select-String -Path "$env:APPDATA\morning-start.silk\logs\silk.log" -Pattern "收到 SSE 事件|SSE 流|转换输出"

# 按日期查看历史日志
Get-Content "$env:APPDATA\morning-start.silk\logs\silk.log.2026-08-30" -Tail 100
```

**关键日志标记**:
- `请求开始处理` — 新请求进入
- `发送上游请求 url=... body_bytes=` — 请求发出
- `收到 SSE 事件 event_type=... is_end=` — 上游事件到达
- `转换输出 converted_bytes=` — 转换后数据
- `SSE 流正常结束 total_events=... bytes_sent=` — 流完成
- `请求处理完成 status=... elapsed_ms=` — 请求结束
- `上游返回错误状态码 status=` — 上游错误
- `失败回退总超时` / `达到最大尝试次数` — 重试耗尽

## 数据库查询

```bash
# 用 sqlite3 查看请求日志
sqlite3 "$env:APPDATA\morning-start.silk\silk.db" "SELECT request_id, method, path, status, elapsed_ms FROM request_logs ORDER BY created_at DESC LIMIT 20"

# 查看表结构
sqlite3 "$env:APPDATA\morning-start.silk\silk.db" ".schema request_logs"
```

## 流式 SSE 调试

```bash
# 流式测试（加 -m 超时，避免挂起）
curl -m 15 -N -X POST http://127.0.0.1:1877/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: YOUR_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -d '{"model":"test","max_tokens":50,"stream":true,"messages":[{"role":"user","content":"hi"}]}'

# 非流式测试
curl -m 10 http://127.0.0.1:1877/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: YOUR_KEY" \
  -d '{"model":"test","max_tokens":50,"stream":false,"messages":[{"role":"user","content":"hi"}]}'
```

**日志关键字段**:
- `收到 SSE 事件 event_type=... is_end=` — 上游事件
- `转换输出 converted_bytes=` — 转换后输出
- `SSE 流正常结束 total_events=... bytes_sent=` — 流结束
- `SSE 流式转换完成 events_converted=... errors=` — 转换统计

**常见问题排查**:
- 流不结束 → 检查 `is_end()` 是否正确识别结束事件
- 内容丢失 → 检查解码器是否处理了对应的事件类型
- 请求挂起 → 检查 failover 循环是否有总超时限制

## Conventions

- **包管理器**: Bun（`bun.lock` 是唯一锁文件）
- **Tauri 2 IPC**: `import { invoke } from '@tauri-apps/api/core'`
- **Vue**: `<script setup lang="ts">`, Composition API
- **TS strict**: `noUnusedLocals`, `noUnusedParameters`
- **UI 文本**: 中文；代码注释中英文均可
- **中间件**: `middleware/` 下独立模块，统一 `run(ctx) → Result` 签名
- **无前端测试框架**，无 CI/CD
- **预设端点自动补全**: 仅渠道快速填充时做协议适配（如 codex 自动补 `/v1`、claude_code 自动去 `/v1`），以用户为准 —— 用户手动修改的端点不做任何规范化改写（`toSettings` 原样透传）

## Constraints

- **单人单机桌面应用** — 不引入分布式、多用户、Redis、消息队列
- **设置变更**: `settings_service::update()` 发送 broadcast → `lib.rs` 监听任务重启网关
- **API Key**: AES-GCM 加密存储；网关 Key: SHA-256 哈希
- **非流式不支持**: 网关强制所有请求走流式 SSE（`transform_request` 注入 `stream: true`），客户端期望非流式会收到 SSE 流
- **修改中间件**: 先检查 `pipeline.rs` 中 `run_main()` 和 `run_with_failover()` 的执行顺序
- **数据库迁移**: `src-tauri/migrations/`（启动时自动执行 via `sqlx::migrate!`）
- **SQLx**: `.sqlx/` 目录包含离线查询缓存

## Key Files

| File | Purpose |
|------|---------|
| `src/main.ts` | Vue 入口 |
| `src/api/index.ts` | ~50 Tauri invoke 封装 |
| `src-tauri/src/lib.rs` | Tauri 初始化、DB 迁移、命令注册、网关生命周期 |
| `src-tauri/src/gateway/pipeline.rs` | 9 阶段管道 + 三级回退 + 插件钩子 |
| `src-tauri/src/gateway/context.rs` | GatewayContext + RequestContext |
| `src-tauri/src/gateway/middleware/dispatch_upstream.rs` | 上游请求分发 + SSE 流读取 |
| `src-tauri/src/gateway/middleware/stream_response.rs` | SSE 事件解析 + 协议转换调度 |
| `src-tauri/src/gateway/middleware/transform_request.rs` | 请求体跨协议转换 |
| `src-tauri/src/protocol/prism_wasm.rs` | prism.wasm 加载 + 函数调用封装 |
| `src-tauri/src/gateway/plugin.rs` | GatewayPlugin 生命周期钩子定义 |

## Tauri Plugins

| Plugin | 用途 |
|--------|------|
| `tauri-plugin-autostart` | 开机自启动 |
| `tauri-plugin-clipboard-manager` | 系统剪贴板读写 |
| `tauri-plugin-dialog` | 原生对话框 |
| `tauri-plugin-notification` | 系统通知 |
| `tauri-plugin-opener` | 打开文件/URL |
| `tauri-plugin-store` | 持久化 KV 存储 |
| `tauri-plugin-updater` | 应用自动更新 |

## 架构决策原则

单人桌面应用，改动前评估：**收益 = 可避免的未来成本 - 本次改动成本**。

- 只算确定会发生的未来成本
- 优先与现有代码保持一致的简单方案
- 不为"理论可测试性"或"未来团队协作"引入不必要的抽象
- 新增协议适配器: 实现 `ProviderAdapter` trait → 注册
- 新增网关插件: 实现 `GatewayPlugin` trait → 注册生命周期钩子
