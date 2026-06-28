# 网关请求转换路径

```mermaid
flowchart LR
    UI[本地请求\n系统设置页] --> CMD[ Tauri Commands ]
    CMD --> KEY[(gateway_keys)]
    CMD --> SET[(gateway_settings)]

    APP[应用启动] --> DB[(SQLite)]
    APP --> LOG[日志写入任务]
    APP --> GW[HTTP Gateway]

    UI --> GW
    GW --> H[/health/]
    GW --> M[/v1/models/]
    GW --> P[Proxy Handler]

    M --> MM[(ModelMappingRepo\n本地启用模型)]

    P --> A[中转站鉴权]
    A -->|Bearer key| KEY
    A -->|hash lookup| DB

    A --> R[请求路由]
    R --> MP[(模型池映射\n模型负载均衡)]
    MP --> CP[(渠道映射\nkey 负载均衡)]
    CP --> PR[(ProviderRepo)]

    R --> N[协议转化]
    N --> T[请求转换]
    T --> D[发送请求]
    D --> U[上游 Provider]
    U --> S[协议转换]
    S --> L[写入日志]
    L --> LOG
    S --> RESP[返回本地]

    H --> RESP
    M --> RESP

    classDef db fill:#f8fafc,stroke:#64748b,stroke-width:1px;
    classDef proc fill:#eff6ff,stroke:#2563eb,stroke-width:1px;
    classDef io fill:#fefce8,stroke:#ca8a04,stroke-width:1px;

    class KEY,SET,DB,MM,MR,RR,PR db;
    class UI,APP,GW,H,M,P,A,R,N,T,D,U,S,L,RESP proc;
    class CMD,LOG io;
```

## 关键点

- `/v1/models` 读取的是本地启用的模型映射，不是上游目录。
- `/v1/*` 的实际鉴权是 `gateway_keys`，不是 `gateway_settings.auth_token_hash`。
- 请求主链是：本地请求 -> 中转站鉴权 -> 中转站接收 -> 模型池映射（模型负载均衡） -> 渠道映射（key 负载均衡） -> 协议转化 -> 发送请求 -> 获得响应 -> 协议转换 -> 返回本地。
- 请求优先按 body 里的 `model` 走模型映射，失败后再走路由规则兜底。
- 日志是异步写入 SQLite，不阻塞主请求。
