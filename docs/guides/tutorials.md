# Silk 使用教程

本教程将指导您完成 Silk 的常见使用场景。

## 目录

- [教程1：基础配置](#教程1基础配置)
- [教程2：多服务配置](#教程2多服务配置)
- [教程3：故障转移配置](#教程3故障转移配置)
- [教程4：高级使用](#教程4高级使用)

---

## 教程1：基础配置

### 目标

配置单个AI服务并成功调用。

### 前提条件

- 已安装 Silk
- 拥有至少一个AI服务的 API Key

### 步骤

#### 1. 启动应用

```bash
# Windows
双击桌面图标或从开始菜单启动

# macOS
open -a Silk

# Linux
./silk
```

#### 2. 完成引导向导

1. 选择"OpenAI"
2. 输入您的 API Key
3. 点击"开始使用"

#### 3. 验证配置

打开终端，测试API调用：

```bash
curl http://127.0.0.1:7600/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

成功响应：
```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "choices": [{
    "message": {
      "role": "assistant",
      "content": "Hello! How can I help you?"
    }
  }]
}
```

---

## 教程2：多服务配置

### 目标

配置多个AI服务，实现负载均衡和故障转移。

### 步骤

#### 1. 添加多个提供商

在"设置" → "提供商管理"中添加：

| 服务 | 模型 | 优先级 |
|------|------|--------|
| OpenAI | gpt-4 | 1 |
| Claude | claude-3-opus | 2 |
| DeepSeek | deepseek-chat | 3 |

#### 2. 配置故障转移

在"高级设置"中启用：
- ✅ 自动重试
- ✅ 失败切换服务
- 重试次数：3

#### 3. 测试故障转移

```bash
# 故意使用错误的API Key测试
curl http://127.0.0.1:7600/v1/chat/completions \
  -H "Authorization: Bearer INVALID_KEY" \
  -d '{"model": "gpt-4", "messages": [...]}'
```

观察日志，应该看到：
1. OpenAI 调用失败
2. 自动切换到 Claude
3. 成功返回响应

---

## 教程3：故障转移配置

### 目标

配置完善的故障转移机制，确保服务高可用。

### 配置策略

```
                ┌─────────────┐
                │   请求      │
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │  主服务     │
                │  (OpenAI)   │
                └──────┬──────┘
                       │ 失败
                ┌──────▼──────┐
                │  备用服务1  │
                │  (Claude)   │
                └──────┬──────┘
                       │ 失败
                ┌──────▼──────┐
                │  备用服务2  │
                │ (DeepSeek)  │
                └─────────────┘
```

### 配置步骤

#### 1. 设置优先级

在提供商配置中设置优先级（数字越小优先级越高）：

```
OpenAI:    优先级 1 (主服务)
Claude:    优先级 2 (备用1)
DeepSeek:  优先级 3 (备用2)
```

#### 2. 配置重试策略

```json
{
  "max_retries": 3,
  "retry_delay_ms": 1000,
  "backoff_multiplier": 2
}
```

#### 3. 设置超时

```json
{
  "timeout_ms": 30000,
  "connect_timeout_ms": 5000
}
```

---

## 教程4：高级使用

### 自定义请求头

某些服务需要自定义请求头。在提供商配置中添加：

```json
{
  "custom_headers": [
    {"name": "X-Custom-Header", "value": "custom-value"},
    {"name": "X-Region", "value": "us-east-1"}
  ]
}
```

### 协议转换

Silk 自动处理不同协议之间的转换：

| 输入格式 | 输出格式 | 自动转换 |
|----------|----------|----------|
| OpenAI Chat | Claude Messages | ✅ |
| OpenAI Chat | OpenAI Responses | ✅ |
| Claude Messages | OpenAI Chat | ✅ |

### 流式响应

所有请求默认支持流式响应：

```bash
curl http://127.0.0.1:7600/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }'
```

### 使用日志分析

1. 打开"日志"页面
2. 筛选条件：
   - 时间范围
   - 状态码
   - 提供商
3. 导出日志用于分析

---

## 最佳实践

### 1. API Key 安全

- 不要在代码中硬编码 API Key
- 使用环境变量或配置文件
- 定期轮换 API Key

### 2. 成本控制

- 设置每个服务的月度预算
- 监控使用量
- 使用较便宜的模型处理简单任务

### 3. 性能优化

- 选择地理位置近的服务
- 合理设置超时时间
- 使用流式响应提升用户体验

### 4. 监控告警

- 定期检查服务状态
- 设置错误率告警
- 监控响应时间

---

## 常见问题

查看 [FAQ](../faq/README.md) 获取更多帮助。
