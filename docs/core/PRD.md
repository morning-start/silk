# Silk（丝路）产品需求文档

> 状态：Approved | 版本：v1.0 | 日期：2026-08-27 | 模式：light

## 一、产品定位

**Silk 是一款纯本地桌面客户端 AI 多模型统一中转站**，无网页、无服务端、不上云，轻量化常驻后台。

**关键词**：本地 / 个人 / 中转站

## 二、目标用户

- 个人开发者
- AI 重度使用者
- AI 使用初学者

## 三、核心价值

本地仅需一个统一接口，即可无缝调用全网所有大模型接口，彻底解决多模型协议不统一、客户端不兼容、切换繁琐的问题。

## 四、核心能力

### 1. 多协议双向归一互转

支持 8 种协议适配器（OpenAI Chat / Response / Codex / vLLM / Claude Messages / Gemini / Azure OpenAI / Google Vertex），通过 Prism WASM 实现双向转换。

### 2. 多账户轮询与模型池管理

多服务商渠道管理 + 4 种负载均衡策略（round_robin / weighted / failover / least_conn）+ 虚拟模型名映射到实际上游模型。

### 3. 本地统一单一入口

所有 AI 工具只连接 `http://127.0.0.1:1877`，一键切换所有模型，无需修改客户端配置。

## 五、技术架构

```
Vue3 + NaiveUI → Tauri IPC → commands/ → application/ services
    → gateway/ (Axum 9-stage pipeline) / protocol/ (adapters) / persistence/ (SQLx + SQLite)
```

详细架构见 [architecture.md](./architecture.md)。

## 六、功能清单

详见 [requirements.md](./requirements.md)（需求分层清单）。

## 七、迭代范围

详见 [scope.md](./scope.md)（范围说明书，已签署）。

## 八、风险评估

详见 [risks.md](./risks.md)（风险清单）。

## 九、产品差异化

| 对比 | Silk 优势 |
|------|----------|
| OneAPI / LiteLLM | 纯桌面客户端，无需部署 |
| Amux | Tauri 极致轻量，无 Electron 臃肿 |
| 各类小工具 | 唯一同时具备多协议互转 + 多账户轮询的本地 GUI 工具 |

## 十、Slogan

**Silk 丝路 — 贯通所有AI模型的本地数字通道**

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2026-08-27 | 初始版本，基于 fst-init 立项产出 |
