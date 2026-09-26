# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 协议内核（prism.wasm）下载与更新：应用内检查/下载官方发布的内核，
  SHA-256 校验 + ABI 探测双重把关后原子替换，支持一键重启生效
- 协议内核回滚：更新后可退回上一版内核（回滚前用 wasmtime 探测备份，
  坏备份拒绝回滚），是「内核更新后起不来」的唯一自救出口
- 启动画面 (Splash Screen)
- 引导向导 (Onboarding Wizard)
- 快捷操作面板
- 帮助系统
- 自动检测已安装的AI应用
- 渠道模板（原「预设AI服务提供商」）：内置目录用于快速填充渠道表单
- 一键快速配置
- 架构设计文档
- API 文档
- 用户手册
- 使用教程
- FAQ 文档

### Changed
- **错误诚实化**：上游错误改为**原样透传**（原始字节 + Content-Type，不做 JSON
  解析后重包装，非 JSON 错误体不再被截断或改形）；silk 自身错误统一加 `【silk】`
  标记，并在标准 `error{message,type,origin}` 对象中同步，避免把 silk 的问题误判成上游的问题
- **403 不再终止回退**：401/403/429/503 属渠道级错误，改为跳过换 Key 直接换渠道；
  回退耗尽时优先返回最后一次上游错误，而非「所有渠道和 Key 均已失败」
- 流式响应中途失败改为下发 SSE `error` 事件后再收尾，避免 hyper 直接中止 body
  导致客户端只看到「连接被重置」
- 预设渠道模板 `PresetProvider` 更名 `ChannelTemplate`（命令 `get_channel_templates`），
  消除与数据库实体 `Provider`（渠道）的命名冲突
- 累加模式判定收敛到 `AgentType::is_additive()`，前端不再自建副本
  （经 `list_agent_types.additive` 下发）
- 协议转换器解耦，支持独立扩展
- 流转换器解耦，支持独立扩展

### Fixed
- **URL 双 `/v1` 前缀**：渠道 `api_base_url` 已带 `/v1` 时不再拼出
  `/v1/v1/chat/completions`（该错误此前表现为 401/404，极易误判为密钥问题）
- 内嵌内核升级至 prism v0.1.4（补 `wasm_abi_version` 导出，重新支持 ABI 探测）
- 内核备份改为**成对写入**（`prism.wasm.bak` + `prism.release.json.bak`），
  修复「内核已回滚、版本号仍是新的」的分裂状态
- 流转换器线程安全问题 (RefCell → Mutex)

### Removed
- 用户友好错误提示层（`UserFriendlyError.vue` / `errorConverter.ts` /
  `error/user_friendly.rs`）：与「错误诚实化」直接冲突 —— 它把上游的 403
  「模型不在套餐内」统一文案成「API 密钥错误」，把用户引向改 Key 的错误方向；
  且三处均为无调用方的死代码

## [0.1.0] - 2024-01-01

### Added
- 初始版本发布
- 基础网关功能
- OpenAI Chat 协议支持
- Claude Messages 协议支持
- OpenAI Responses 协议支持
- 多提供商管理
- 日志记录功能
- 配置管理

---

## 版本说明

### 版本号格式

本项目使用语义化版本号：`主版本号.次版本号.修订号`

- **主版本号**: 不兼容的 API 变更
- **次版本号**: 向后兼容的功能性新增
- **修订号**: 向后兼容的问题修正

### 变更类型

- **Added**: 新功能
- **Changed**: 对现有功能的变更
- **Deprecated**: 已经不建议使用，即将移除的功能
- **Removed**: 已移除的功能
- **Fixed**: 任何 Bug 修复
- **Security**: 安全相关的变更
