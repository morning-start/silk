# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 启动画面 (Splash Screen)
- 引导向导 (Onboarding Wizard)
- 用户友好的错误提示
- 快捷操作面板
- 帮助系统
- 自动检测已安装的AI应用
- 预设AI服务提供商
- 一键快速配置
- 架构设计文档
- API 文档
- 用户手册
- 使用教程
- FAQ 文档

### Changed
- 协议转换器解耦，支持独立扩展
- 流转换器解耦，支持独立扩展
- 错误信息用户友好化

### Fixed
- 流转换器线程安全问题 (RefCell → Mutex)

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
