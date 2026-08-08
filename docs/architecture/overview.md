# Silk 系统架构概述

## 1. 简介

Silk 是一个基于 Tauri 的桌面应用，作为个人AI总机，提供统一的AI服务访问入口。

## 2. 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                 Layer 1: GUI接入层                          │
│                 (用户交互、界面展示)                         │
├─────────────────────────────────────────────────────────────┤
│                 Layer 2: Application应用层                  │
│                 (业务逻辑、状态管理)                         │
├─────────────────────────────────────────────────────────────┤
│                 Layer 3: Gateway网关层                      │
│                 (请求路由、管道处理)                         │
├─────────────────────────────────────────────────────────────┤
│                 Layer 4: Protocol协议转换层                  │
│                 (无状态、流式处理、字节流转换)               │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 Layer 1: GUI接入层

**职责**：
- 用户界面展示
- 用户交互处理
- 状态展示

**边界**：
- 禁止混入HTTP转发、协议转换逻辑
- 仅通过Tauri Commands与Application层交互

**代码位置**：`src/` (Vue组件)

**主要组件**：
- `App.vue` - 应用根组件
- `AppContent.vue` - 主布局组件
- `views/` - 页面组件
- `components/` - 通用组件

### 2.2 Layer 2: Application应用层

**职责**：
- 业务逻辑编排
- 状态管理
- 服务协调

**边界**：
- 不直接处理网络请求
- 通过Gateway层处理HTTP请求

**代码位置**：`src-tauri/src/application/`

**主要服务**：
- `gateway_service.rs` - 网关服务
- `provider_service.rs` - Provider服务
- `log_service.rs` - 日志服务
- `settings_service.rs` - 设置服务
- `auto_detect.rs` - 自动检测服务
- `quick_setup.rs` - 快速配置服务
- `preset_providers.rs` - 预置配置服务

### 2.3 Layer 3: Gateway网关层

**职责**：
- HTTP请求处理
- 中间件管道
- 路由分发

**边界**：
- 不处理协议格式转换细节
- 通过Protocol层进行协议转换

**代码位置**：`src-tauri/src/gateway/`

**主要组件**：
- `pipeline.rs` - 请求处理管道
- `middleware/` - 中间件模块
- `context.rs` - 请求上下文

**中间件管道流程**：
1. `extract` - 提取请求信息
2. `authenticate` - 认证验证
3. `resolve_route` - 路由解析
4. `select_channel` - 渠道选择
5. `transform_request` - 请求转换
6. `dispatch_upstream` - 转发上游
7. `transform_response` - 响应转换
8. `persist_log` - 日志持久化
9. `finalize` - 最终处理

### 2.4 Layer 4: Protocol协议转换层

**职责**：
- 无状态协议转换
- 流式字节流处理
- 支持多种AI协议

**边界**：
- 仅处理字节流，不承担网络通信
- 完全无状态设计

**代码位置**：`src-tauri/src/protocol/`

**主要组件**：
- `converter.rs` - 协议转换抽象层
- `converters/` - 协议转换器实现
- `stream/` - 流式转换器

**支持的协议**：
- OpenAI Chat Completions
- Claude Messages
- OpenAI Responses

## 3. 模块依赖关系

```
GUI接入层
    ↓ Tauri Commands
Application应用层
    ↓ 调用
Gateway网关层
    ↓ 调用
Protocol协议转换层
    ↓ 使用
linguafranca库
```

## 4. 数据流

### 4.1 请求处理流程

```
用户 → GUI → Application → Gateway → Protocol → AI服务
                                    ↓
                              协议转换
                                    ↓
用户 ← GUI ← Application ← Gateway ← Protocol ← AI服务
```

### 4.2 流式响应流程

```
用户 → GUI → Gateway → Protocol → AI服务
                        ↓
                   SSE事件流
                        ↓
用户 ← GUI ← Gateway ← Protocol ← AI服务
```

## 5. 技术栈

### 5.1 前端
- **框架**：Vue 3 + TypeScript
- **UI库**：Naive UI
- **状态管理**：Pinia
- **路由**：Vue Router
- **构建工具**：Vite

### 5.2 后端
- **框架**：Tauri + Rust
- **HTTP服务器**：Axum
- **数据库**：SQLite
- **异步运行时**：Tokio

### 5.3 协议转换
- **库**：linguafranca
- **支持协议**：
  - OpenAI Chat Completions
  - Claude Messages
  - OpenAI Responses

## 6. 部署架构

```
用户电脑
├── Silk桌面应用
├── 本地SQLite数据库
└── 配置文件
        ↓
AI服务
├── OpenAI API
├── Claude API
└── 其他AI服务
```

## 7. 安全考虑

### 7.1 API密钥安全
- API密钥加密存储在本地
- 不上传到任何服务器
- 仅在本地使用

### 7.2 网络安全
- 默认仅监听本地地址（127.0.0.1）
- 可选开启远程访问
- 支持HTTPS（可选）

### 7.3 数据安全
- 所有数据存储在本地
- 不收集用户数据
- 支持数据备份和恢复

## 8. 扩展性

### 8.1 协议扩展
- 支持添加新的AI协议
- 通过实现ProtocolConverter接口
- 插件化设计

### 8.2 功能扩展
- 支持添加新的中间件
- 支持自定义路由规则
- 支持插件系统
