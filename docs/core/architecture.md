# 架构文档

> 本文件是全局架构文档，跨迭代共享，保持架构的一致性。

## 架构概述

Silk（丝路）是纯本地桌面 AI 多模型统一中转站，采用 Tauri 2 + Vue 3 + Rust/Axum + SQLite 技术栈。

```
Vue3 + NaiveUI → Tauri IPC (invoke) → commands/ → application/ services
    → gateway/ (Axum 9-stage pipeline) / protocol/ (adapters) / persistence/ (SQLx + SQLite)
```

## 核心组件

### Gateway 网关层

- **职责**：HTTP 请求处理、9 阶段中间件管道、3 级失败回退
- **端口**：`127.0.0.1:1877`
- **管道**：extract → authenticate → resolve_route → select_channel → transform_request → dispatch_upstream → transform_response → persist_log → finalize
- **回退**：重试耗尽 → 换 Key → 换 Provider → 502

### Protocol 协议层

- **职责**：多协议适配 + Prism WASM 转换
- **协议**：OpenAI Chat / Response / Codex / vLLM / Claude Messages / Gemini / Azure OpenAI / Google Vertex（8 种）
- **实现**：数据驱动适配器 + MoonBit 编译的 WASM 转换器

### Application 应用层

- **职责**：业务逻辑编排、Tauri 命令注册、服务协调
- **服务**：gateway_service / provider_service / log_service / settings_service

### GUI 接入层

- **职责**：用户界面展示、Tauri IPC 调用
- **技术**：Vue 3 + NaiveUI + Tailwind CSS
- **视图**：Dashboard / Providers / ModelSquare / AgentProfiles / Logs / Settings（6 个路由）

## 数据模型

| 表 | 用途 |
|---|------|
| `providers` | 服务商渠道配置 |
| `model_mappings` | 模型池映射 |
| `model_mapping_channels` | 映射-渠道多对多 |
| `request_logs` | 请求日志 |
| `request_log_extra_token` | Token 扩展数据 |
| `profiles` | Agent 配置预设 |
| `common_config_snippets` | 配置片段 |

## 架构决策

见 `.agent-workplace/shared/adr/` 目录。

## 版本历史

- **v1.0** (2026-08-27, iteration-001): 从项目现状提取，初始版本
