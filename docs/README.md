# Silk 文档系统

> 双文档体系（flowstate 0.3.0）| 模式：light | 更新日期：2026-08-28

## 文档结构

```
docs/
├── README.md               # 本文件
│
├── core/                   # 核心正式文档（立项/迭代必选）
│   ├── PRD.md              # 产品需求文档
│   ├── requirements.md     # 需求分层清单
│   ├── scope.md            # 范围说明书
│   ├── risks.md            # 风险清单
│   ├── glossary.md         # 术语表
│   └── architecture.md     # 架构文档
│
├── adr/                    # 架构决策记录
│   ├── ADR-0001-architecture.md
│   └── ADR-0002-single-user-local-only.md
│
├── reference/              # 参考文档
│   ├── api-guide.md        # API 使用指南
│   ├── gateway-api.md      # 网关 API 文档
│   ├── dev-guide.md        # 开发者指南
│   ├── architecture-detail.md  # 详细架构（历史版本）
│   ├── request-flow.md     # 请求处理流程
│   └── code-review.md      # 代码审查规范
│
├── guides/                 # 用户指南
│   ├── user-manual.md      # 用户手册
│   ├── tutorials.md        # 教程
│   ├── api.md              # API 文档
│   └── faq.md              # 常见问题
│
```

## 双文档体系

| 层 | 位置 | 提交 git? | 说明 |
|----|------|-----------|------|
| **正式文档** | `docs/` | ✅ | PRD、ADR、需求、范围、风险、术语定稿 |
| **工作区** | `.agent-workplace/` | ❌ | 迭代过程稿、调研、状态、草稿 |

## 核心文档清单

| 文档 | 文件 | 状态 |
|------|------|------|
| PRD | `core/PRD.md` | ✅ v1.0 |
| 需求分层 | `core/requirements.md` | ✅ v1.0 |
| 范围说明书 | `core/scope.md` | ✅ 已签署 |
| 风险清单 | `core/risks.md` | ✅ v1.0 |
| 架构文档 | `core/architecture.md` | ✅ v1.0 |
| 术语表 | `core/glossary.md` | ✅ v1.0 |
| ADR | `adr/` | ✅ 2 条记录 |

## 参考文档

| 文档 | 说明 |
|------|------|
| [API 使用指南](./reference/api-guide.md) | API 调用示例 |
| [网关 API 文档](./reference/gateway-api.md) | HTTP 网关接口说明 |
| [开发者指南](./reference/dev-guide.md) | 开发环境搭建 |
| [详细架构](./reference/architecture-detail.md) | 详细架构说明（历史版本） |
| [请求流程](./reference/request-flow.md) | 请求处理流程 |
| [代码审查](./reference/code-review.md) | 代码审查标准 |

