# 术语表

> 本文件是全局术语表，跨迭代共享，保持术语定义的一致性。

## 术语清单

| 术语 | 定义 | 状态 |
|------|------|------|
| Provider（服务商/渠道） | LLM 服务提供商的接入配置，包含 base_url、API Key、协议类型 | ACTIVE |
| Model Mapping（模型映射） | 虚拟模型名到实际上游模型的映射关系，支持多渠道路由 | ACTIVE |
| Channel（渠道路由） | 模型映射中的单个上游通道，关联 provider + 选定模型 | ACTIVE |
| Pipeline（管道） | 网关请求处理的 9 阶段中间件链 | ACTIVE |
| Failover（回退） | 请求失败时的 3 级自动切换策略（换 Key → 换 Provider → 502） | ACTIVE |
| Prism WASM | MoonBit 编译的协议转换器，运行在 wasmtime 中 | ACTIVE |
| Agent Profile | 针对不同 AI 编码代理（Claude Code / Codex 等）的预设配置 | ACTIVE |
| Gateway Key | 本地网关的鉴权密钥（SHA-256 哈希），隔离上游原始 Key | ACTIVE |
| PassThrough（透传） | 跳过协议转换，原样转发请求/响应的模式 | ACTIVE |
| Round Robin（轮询） | 多 Key/多 Provider 间的依次轮流选择策略 | ACTIVE |

## 状态说明

- **ACTIVE**：活跃，正在使用
- **DEPRECATED**：已弃用，不再使用
- **DRAFT**：草稿，待确认

## 版本历史

- **v1.0** (2026-08-27, iteration-001): 初始版本
