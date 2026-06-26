# Silk 后端请求编排中间层设计

## 1. 背景

当前 `src-tauri/src/gateway/mod.rs` 已经承担了过多职责：HTTP 接入、路由解析、上游转发、响应读取、日志组装都在同一条流程里完成。随着后续继续加入协议转换、重试、限流、缓存、鉴权和审计，这个入口会迅速变成高耦合的“总控文件”。

本设计的目标是引入一层显式的请求编排管线，把每个横切能力拆成独立中间件，让主流程只负责组装和收口。

## 2. 目标

- 将网关处理流程拆成可组合的中间件链。
- 明确每一步的输入、输出和副作用边界。
- 为后续协议转换、重试、限流、缓存、鉴权预留插槽。
- 降低 `gateway/mod.rs` 的复杂度，避免继续膨胀。
- 让日志、错误、响应构造在统一出口收口。

## 3. 非目标

- 不在这一阶段实现完整的 OpenAI / Claude 协议转换逻辑。
- 不在这一阶段实现缓存、限流、鉴权的具体策略，只预留接口。
- 不重构 UI 层，也不改变数据库 schema。
- 不引入复杂的动态插件系统，先采用静态中间件注册。

## 4. 总体方案

采用 `RequestContext + Middleware + Pipeline` 三层结构：

- `RequestContext` 保存一次请求从进入网关到结束所需的全部状态。
- `Middleware` 只负责处理一个明确步骤，并返回更新后的上下文。
- `Pipeline` 负责按顺序执行中间件，并在任意一步失败时统一中断和收口。

主入口只做三件事：

1. 将 axum 的 `Request<Body>` 转成 `RequestContext`。
2. 将上下文交给 pipeline。
3. 将 pipeline 的最终结果转成 axum response。

## 5. 目录结构

建议将 `src-tauri/src/gateway/` 重组为：

```text
gateway/
  mod.rs
  context.rs
  pipeline.rs
  error.rs
  middleware/
    mod.rs
    extract.rs
    resolve_route.rs
    normalize_protocol.rs
    transform_request.rs
    dispatch_upstream.rs
    transform_response.rs
    persist_log.rs
    finalize.rs
```

说明：

- `mod.rs` 保留模块导出、server 启动和 router 组装。
- `context.rs` 定义请求上下文和运行时上下文。
- `pipeline.rs` 定义中间件执行模型。
- `middleware/` 按职责拆分步骤。
- `error.rs` 统一网关错误类型与 axum 适配。

## 6. 核心抽象

### 6.1 GatewayContext

`GatewayContext` 继续作为运行时依赖容器，但职责收敛为“只存共享资源”，例如：

- SQLite pool
- Gateway settings
- RouteManager
- 客户端工厂或共享 HTTP client 配置

它不再承担具体请求状态。

### 6.2 RequestContext

`RequestContext` 是单次请求的状态载体，建议包含：

- `request_id`
- `started_at`
- 原始 `method`、`uri`、`headers`、`body`
- 解析出的 `host`、`path`
- 选中的 `route`
- 选中的 `provider`
- 协议判定结果
- 标准化后的中间请求
- 上游响应
- 最终响应
- 日志所需字段
- 失败信息

原则：

- 上下文是“可补充、可覆盖”的。
- 中间件不得直接返回裸响应，除非发生提前终止。
- 重要中间结果必须写回上下文，便于后续日志和调试。

### 6.3 Middleware

中间件建议采用同步接口风格上的异步版本：

```rust
async fn run(ctx: RequestContext) -> Result<RequestContext, GatewayError>;
```

约束：

- 每个中间件只做一件事。
- 中间件可以读取上下文、补充上下文、记录副作用。
- 中间件不要直接依赖其他中间件的内部实现。

### 6.4 Pipeline

`Pipeline` 负责按序执行中间件，并支持一个“总是执行”的收口阶段：

1. `extract`
2. `resolve_route`
3. `normalize_protocol`
4. `transform_request`
5. `dispatch_upstream`
6. `transform_response`
7. `finalize`

总是执行的阶段：

- `persist_log`

执行策略：

- 任一步失败，立即进入统一错误收口。
- `persist_log` 必须放在 `finally` 风格的收口阶段中执行，不能依赖主链“刚好走到最后”。
- `finalize` 只负责把结果变成 axum response，不再执行业务逻辑。

## 7. 请求流转

### 7.1 extract

职责：

- 读取 `method / uri / headers / body`
- 生成 `request_id`
- 记录开始时间
- 做请求体大小上限检查

输出：

- 填充基础请求上下文
- 不做路由判断和协议判断

### 7.2 resolve_route

职责：

- 从 `host / method / path / content_type` 计算匹配路由
- 读取对应 provider
- 将路由和 provider 写回上下文

输出：

- 成功则上下文进入下一步
- 失败则返回“未找到路由”或“未找到 provider”错误

### 7.3 normalize_protocol

职责：

- 判断入站协议类型
- 决定是否需要转换为内部标准请求

说明：

- 这一层先做协议识别和统一标记。
- 真正的转换逻辑可以在下一阶段扩展，不要求本阶段完成全部字段映射。

### 7.4 transform_request

职责：

- 将不同入站协议转换为内部标准请求模型
- 清理不需要透传的字段
- 准备上游请求所需的最终 body、headers、query 等数据

原则：

- 这里的输出必须是统一结构，避免后面的 dispatch 再理解多种协议。

### 7.5 dispatch_upstream

职责：

- 构造 reqwest 请求
- 发往上游 provider
- 捕获状态码、headers、body
- 记录耗时和响应大小

输出：

- 将上游结果写回上下文

### 7.6 transform_response

职责：

- 将上游响应转换回目标入站协议所需格式
- 如果是透明透传场景，则保持原样

原则：

- 转换逻辑只关心响应，不再回头改写上游请求。

### 7.7 persist_log

职责：

- 组装请求日志
- 写入 SQLite
- 记录成功/失败、耗时、token、provider、route、body 摘要等信息

要求：

- 尽量采用“最终记录”思路，保证请求结束后有一致的日志结构。
- 如果日志写入失败，不能影响主响应优先返回，但需要向错误系统暴露可观测信息。
- 该步骤必须在成功和失败两种路径下都执行。
- 实现方式可以是 `Pipeline::run()` 之后的 `after` / `finally` 钩子，而不是普通主链中间件。

### 7.8 finalize

职责：

- 根据上下文中的结果构造最终 axum `Response`
- 对成功/失败响应统一出口

原则：

- 不再执行业务判断。
- 不再访问数据库。
- 不再发起外部 HTTP 请求。

## 8. 错误模型

建议将错误分为以下几类：

- `BadRequest`：请求格式错误、body 读取失败、方法非法
- `NotFound`：路由不存在、provider 不存在
- `Upstream`：上游请求失败、超时、网络错误
- `Transform`：协议转换失败
- `Database`：持久化失败
- `Internal`：未预期错误

统一原则：

- 中间件只返回结构化错误，不直接拼接 HTTP 文本。
- 错误到 HTTP 状态码的映射只在最终错误适配层处理。
- 日志中记录原始错误，但对外响应保持稳定格式。

## 9. 状态与依赖

### 9.1 全局共享状态

保留在 `GatewayContext` 中：

- `SqlitePool`
- `GatewaySettings`
- `RouteManager`
- HTTP client 配置工厂

### 9.2 单请求状态

保留在 `RequestContext` 中：

- 请求原始数据
- 解析后的路由结果
- 标准化请求
- 上游响应
- 日志数据
- 错误数据

### 9.3 依赖方向

依赖必须单向流动：

`GatewayContext -> Middleware -> RequestContext -> Final Response`

禁止：

- 中间件互相直接调用私有实现
- 请求上下文反向依赖全局上下文的业务逻辑

## 10. 迁移步骤

### 阶段 1：抽壳

- 将 `gateway/mod.rs` 中的请求处理逻辑拆出到 `context.rs`、`pipeline.rs`、`error.rs`。
- 保持对外行为不变。

### 阶段 2：拆中间件

- 先拆 `extract`、`resolve_route`、`dispatch_upstream`、`persist_log`。
- 让主流程先具备中间件形态。

### 阶段 3：补协议层

- 引入 `normalize_protocol`、`transform_request`、`transform_response`。
- 将协议差异从转发逻辑中移走。

### 阶段 4：预留横切能力

- 增加缓存、限流、重试、鉴权中间件的空位或默认实现。
- 后续仅需插入 pipeline，不改入口结构。

## 11. 测试策略

### 单元测试

- `RequestContext` 初始化是否完整。
- `RouteManager` 是否命中正确规则。
- 错误类型到状态码的映射是否稳定。
- 中间件顺序是否符合预期。

### 集成测试

- 构造一条完整请求，验证能走通 `extract -> resolve_route -> dispatch -> log -> finalize`。
- 模拟路由不存在、provider 不存在、上游超时、日志失败等场景。

### 回归重点

- 请求体大小限制。
- 路由匹配优先级。
- 日志写入失败不影响主响应。
- 统一出口不会吞掉错误细节。

## 12. 风险

- 抽象层增加后，初期代码量会比当前单文件方案更多。
- 如果中间件边界定义不严，仍会出现“薄壳大函数”。
- 协议转换和转发耦合过深时，可能需要再引入更细的标准请求模型。

## 13. 结论

本设计选择显式 middleware pipeline，优先解决“网关入口耦合过高”的问题。它不是为了追求抽象本身，而是为了让后续的协议转换、缓存、限流、鉴权、审计可以稳定挂到同一条链路上。

如果按此方案实现，`gateway/mod.rs` 将退化为装配层，核心复杂度会被移入一组职责清晰、可单测的中间件模块。
