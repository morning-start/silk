# 网关请求转换路径

```mermaid
flowchart TD
    REQ[外部请求\nHTTP Client] --> EXT[extract\n读取请求体 2MB 限制]
    EXT --> AUTH[authenticate\n网关鉴权\nBearer key → gateway_keys]
    AUTH --> RL[rate_limit\nIP 级限流检查]
    RL --> RR[resolve_route\n路由解析\n匹配 routing_rules / 模型映射]

    RR -->|路径 /v1/models| MM[(ModelMappingRepo\n本地启用模型)]
    MM --> RSP[提前构建响应]

    RR -->|匹配到路由| B_RT[before_route\n插件钩子]
    B_RT -->|检查缓存等| SC[select_channel\n选择渠道 & Key\nKey 负载均衡]

    RR --> PR[(ProviderRepo\n渠道配置)]
    RR --> GR[(GatewaySettings\n路由规则)]

    SC --> TR[transform_request\n请求协议转换\nOpenAI/Claude → 上游格式]
    TR --> B_UP[before_upstream\n插件钩子\n注入缓存/日志压缩/窗口截断]
    B_UP --> DU[dispatch_upstream\n发送上游请求\n含重试+退避+SSE管理]
    DU --> UP[上游 Provider\nOpenAI / Claude / 其他]
    UP --> TRESP[transform_response\n响应协议转换\n上游格式 → OpenAI Response]
    TRESP --> A_UP[after_upstream\n插件钩子]
    A_UP --> PL[persist_log\n异步写入 SQLite]
    RSP --> PL
    PL --> FIN[finalize\n构建最终 HTTP 响应]

    AUTH --> GK[(gateway_keys\n网关鉴权 Key)]
    SC --> CP[渠道内 Key 轮询\n失败自动切换]

    subgraph 三级失败回退
        DU -->|Level 1 重试耗尽| SC
        SC -->|Level 2 换 Key| TR
        RR -->|Level 3 换渠道| SC
    end

    classDef db fill:#f8fafc,stroke:#64748b,stroke-width:1px;
    classDef proc fill:#eff6ff,stroke:#2563eb,stroke-width:1px;
    classDef io fill:#fefce8,stroke:#ca8a04,stroke-width:1px;

    class GK,MM,PR,GR db;
    class REQ,EXT,AUTH,RL,RR,SC,TR,DU,TRESP,PL,FIN,RSP,UP,B_RT,B_UP,A_UP proc;
    class CP io;
```

## 关键点

- `/v1/models` 读取的是本地启用的模型映射，不是上游目录。
- `/v1/*` 的实际鉴权是 `gateway_keys`，不是 `gateway_settings.auth_token_hash`。
- 请求主链已引入插件钩子系统：
  `extract → authenticate → rate_limit → resolve_route → [before_route] → select_channel → transform_request → [before_upstream] → dispatch_upstream → transform_response → [after_upstream] → persist_log → finalize`。
- 默认开启的 3 个内置原生插件：
  *   **`PromptCachePlugin`**：自动为 Claude 注入 `cache_control` 标记。
  *   **`SlidingWindowPlugin`**：滑动窗口，自动裁剪历史对话。
  *   **`TerminalLogPrunerPlugin`**：控制台/终端大段编译输出日志压缩折叠。
- 路由决策优先级（由高到低）：
  1. **模型映射优先**：先读取请求体 `model` 字段，在 `model_mappings` 表中查找启用映射。命中后按映射配置的负载均衡策略选定 Provider 渠道，支持自动回退。
  2. **路由规则降级**：模型映射未命中时，再按 `RoutingRule` 匹配（Host + Path + Method + ContentType）。
  3. **路径兜底**：路由规则也未命中时，按请求路径自动推断协议，选择任意启用的 Provider。
- 日志是异步写入 SQLite，不阻塞主请求。
- 三级失败回退：单次请求重试 → 换 Key → 换渠道。