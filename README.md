# Silk 丝路

<p align="center">
  <img src="https://img.shields.io/badge/version-0.2.0-blue" alt="version">
  <img src="https://img.shields.io/badge/Tauri-2.0-purple" alt="tauri">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license">
</p>

> **一个本地端点，管理所有 AI 提供商。** 装个桌面应用，加几把 API Key，你的所有 AI 工具就都能用上 OpenAI、Claude、通义千问……协议自动转换，失败自动换 Key 换 Provider，Key 加密存本地。

Silk 是一个运行在你桌面的 AI 多模型网关（Tauri 2 + Rust/Axum）。你在管理多个 AI 提供商时遇到的所有麻烦——协议不兼容、API Key 分散、一个限流整个服务挂掉——它用一个本地 HTTP 端点 `http://127.0.0.1:9876` 替你搞定。

## ✨ 功能亮点

- **🌐 统一接入** — 你的 ChatBox、LobeChat、VS Code 插件、OpenAI-compatible 客户端……所有工具指向一个地址就够了。Silk 根据模型名自动路由到正确的提供商
- **🔄 协议自动转换** — 用 OpenAI Chat 格式发请求，发给 Claude 也能正确响应。三种主流协议（OpenAI Chat / Claude Messages / OpenAI Response）任意互转，不用记三套 API 文档
- **🛡️ 三级失败回退** — 上游超时 → 自动重试（指数退避）→ 换一个 API Key → 换一个提供商。用户无感知，你的 AI 体验不中断
- **🔐 纯本地，隐私由你** — 所有数据存本地 SQLite，API Key 用 AES-GCM 加密，没有云端组件。你发的每个请求，你说了算
- **🔌 可插拔中间件** — Prompt 缓存省 Token、滑动窗口控上下文、请求日志压缩……网关中间件按需加载，不想要就关掉
- **🖥️ 图形化管理** — 开箱即用的桌面 UI（Vue 3 + NaiveUI），管理提供商、路由规则、查看日志——不用写一行配置

## 快速开始

```bash
# 安装依赖
bun install

# 启动开发模式（会自动打开桌面窗口）
bun run tauri dev
```

### 首次使用（30 秒）

1. 打开 Silk 桌面应用
2. 在"提供商管理"页面添加 AI 服务商和 API Key（支持 OpenAI、Claude、通义千问等）
3. 点击"启动网关"
4. 把你的 AI 工具 HTTP 端点改成 `http://127.0.0.1:9876`

### 验证网关在工作

```bash
curl http://127.0.0.1:9876/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: 你在应用里设的网关Key" \
  -d '{"model":"gpt-4","messages":[{"role":"user","content":"你好"}]}'
```

返回 SSE 流式响应，说明已经跑通了。

## 为什么选 Silk，而不是直接用各家 API？

| 场景 | 直接用各家 API | Silk |
|------|---------------|------|
| 同时用 OpenAI + Claude | 两套 API 格式、两套认证、两套 SDK | 一个本地端点，协议自动转换 |
| API Key 配额用尽 / 被限流 | 手动切 Key，服务中断 | 自动换 Key / 换 Provider，用户无感 |
| 多个 AI 工具接入 | 每个工具单独配 API Key 和端点 | 统一指向 `127.0.0.1:9876`，一处管理 |
| 数据隐私 / 审计 | 请求直发外部，日志不受控 | Key 加密存本地，日志可查可删，全在你机器上 |
| 路由策略（测试→生产） | 不支持，或手动改代码 | 模型映射 + 路由规则 + 路径兜底，三层匹配灵活调度 |

## 架构速览（进阶内容）

<details>
<summary>展开查看 10 阶段网关管道</summary>

请求经过 10 个独立中间件阶段处理，插件在关键节点注入：

```
提取 → 认证 → 路由解析 → 限流 → [插件: before_route]
→ 选择 Key → 请求转换 → [插件: before_upstream]
→ 上游转发（SSE流式+重试+换Key换Provider） → 响应转换
→ [插件: after_upstream] → 持久化日志 → 构建响应
```

</details>

<details>
<summary>展开查看技术栈</summary>

| 层级 | 技术 |
|------|------|
| UI | Vue 3 + TypeScript + NaiveUI |
| 桌面框架 | Tauri 2 |
| 后端网关 | Rust + Axum + Tokio |
| 数据库 | SQLite (SQLx) |
| 加密 | AES-GCM + PBKDF2 |
| 包管理 | Bun |

</details>

<details>
<summary>展开看开发命令</summary>

```bash
bun run dev              # Vite 前端开发 (port 1510)
bun run tauri dev        # 完整桌面应用开发
bun run tauri build      # 打包安装包
cargo check              # Rust 类型检查（在 src-tauri/ 下）
```

</details>

## 相关文档

- [API 使用指南](docs/API使用指南.md) — 网关路由、Provider 配置、模型映射完整说明
- [开发者指南](docs/开发者指南.md) — 扩展协议适配器、添加中间件、理解数据模型
- [网关 API 文档](docs/网关API文档.md) — HTTP 端点参考

## 许可证

AGPLv3 — 查看 [LICENSE](LICENSE)
