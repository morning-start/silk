# ADR-0001: 项目架构选型

> 状态：Accepted | 日期：2026-08-27

## 背景

Silk 需要一个纯本地桌面 AI 中转网关，要求轻量、高性能、跨平台、长期稳定。

## 决策

| 层 | 选型 | 理由 |
|---|------|------|
| 桌面框架 | Tauri 2 | 极致轻量（对比 Electron），Rust 内核，系统原生 WebView |
| 前端 | Vue 3 + NaiveUI + Tailwind | 响应式、组件丰富、开发效率高 |
| 后端 | Rust + Axum + Tokio | 高性能异步、内存安全、零 GC 停顿 |
| 数据库 | SQLite + SQLx | 纯本地、零依赖、编译期校验查询 |
| 协议转换 | Prism WASM (MoonBit) | 无状态、可热更新、跨语言 |
| 构建 | Bun + Vite | 快速、原生 TS 支持 |

## 后果

- 正面：极致轻量、高性能、单二进制分发
- 负面：Tauri 2 生态相对年轻，某些平台特性需要自行适配

## 相关

- 项目定位：`docs/PRD.md`
- 技术约束：`AGENTS.md`
