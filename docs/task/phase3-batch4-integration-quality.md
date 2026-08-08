# Phase 3 - 批次4：集成测试与质量保障

## 📋 批次概述

**批次目标**：进行集成测试、性能测试、安全测试、代码审查

**预计工期**：1周（5个工作日）

**依赖关系**：批次3完成

---

## 🎯 批次目标

1. 集成测试用例
2. 性能测试
3. 安全测试
4. 代码审查

---

## 📝 任务清单

### 任务4.1：集成测试用例

**文件路径**：`tests/integration/`

**任务描述**：
编写端到端流程测试、API集成测试

**实现步骤**：

1. 创建集成测试目录
```bash
mkdir -p tests/integration
```

2. 创建端到端测试
```rust
// tests/integration/e2e_test.rs

#[cfg(test)]
mod tests {
    use silk_lib::application::gateway_service;
    use silk_lib::persistence::ProviderRepo;
    use silk_lib::models::Provider;
    
    #[tokio::test]
    async fn test_gateway_lifecycle() {
        // 测试网关启动、停止、重启的完整生命周期
        let data_dir = unique_temp_dir();
        let pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        // 创建测试Provider
        let provider = Provider {
            id: "test-provider".to_string(),
            name: "Test Provider".to_string(),
            protocols: r#"["openai_chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: r#"[]"#.to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://example.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 0,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };
        
        ProviderRepo::create(&pool, &provider).await.expect("create provider");
        
        // 测试网关启动
        let (log_sender, _log_receiver) = tokio::sync::mpsc::channel(1);
        let gateway = gateway_service::load_gateway_context(pool.clone(), log_sender)
            .await
            .expect("load gateway context");
        
        let (settings_change_tx, _settings_change_rx) = tokio::sync::broadcast::channel(16);
        let state = silk_lib::AppState {
            gateway: std::sync::Arc::new(tokio::sync::RwLock::new(gateway)),
            gateway_server: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            lookup_cache: std::sync::Arc::new(tokio::sync::RwLock::new(silk_lib::LookupCache::default())),
            settings_change_tx,
        };
        
        // 启动网关
        let start_result = gateway_service::start(&state).await;
        assert!(start_result.is_ok());
        
        // 验证网关运行状态
        let status = gateway_service::status(&state).await.expect("get status");
        assert!(status.running);
        
        // 停止网关
        let stop_result = gateway_service::stop(&state).await;
        assert!(stop_result.is_ok());
        
        // 验证网关停止状态
        let status = gateway_service::status(&state).await.expect("get status");
        assert!(!status.running);
    }
    
    #[tokio::test]
    async fn test_provider_crud() {
        // 测试Provider的增删改查
        let data_dir = unique_temp_dir();
        let pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        // 创建Provider
        let provider = Provider {
            id: "test-provider".to_string(),
            name: "Test Provider".to_string(),
            protocols: r#"["openai_chat"]"#.to_string(),
            models: r#"["gpt-4"]"#.to_string(),
            keys: r#"[]"#.to_string(),
            key_strategy: "round_robin".to_string(),
            api_base_url: "https://example.com".to_string(),
            proxy_url: None,
            timeout_seconds: 30,
            max_retries: 0,
            status: "enabled".to_string(),
            health_status: None,
            last_health_check_at: None,
            metadata_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };
        
        // 创建
        let created = ProviderRepo::create(&pool, &provider).await.expect("create provider");
        assert_eq!(created.id, "test-provider");
        
        // 查询
        let found = ProviderRepo::get_by_id(&pool, "test-provider")
            .await
            .expect("get provider");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test Provider");
        
        // 更新
        let mut updated_provider = provider.clone();
        updated_provider.name = "Updated Provider".to_string();
        let updated = ProviderRepo::update(&pool, &updated_provider)
            .await
            .expect("update provider");
        assert_eq!(updated.name, "Updated Provider");
        
        // 删除
        let deleted = ProviderRepo::delete(&pool, "test-provider")
            .await
            .expect("delete provider");
        assert!(deleted);
        
        // 验证删除
        let found = ProviderRepo::get_by_id(&pool, "test-provider")
            .await
            .expect("get provider");
        assert!(found.is_none());
    }
    
    fn unique_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
```

3. 创建API集成测试
```rust
// tests/integration/api_test.rs

#[cfg(test)]
mod tests {
    use reqwest;
    use serde_json;
    
    #[tokio::test]
    async fn test_health_endpoint() {
        // 测试健康检查端点
        // 注意：需要先启动网关
        let client = reqwest::Client::new();
        let response = client.get("http://127.0.0.1:1877/health")
            .send()
            .await;
        
        // 这个测试需要网关运行，可能需要mock或启动测试服务器
        // assert!(response.is_ok());
    }
    
    #[tokio::test]
    async fn test_chat_completions_endpoint() {
        // 测试Chat Completions端点
        // 注意：需要先启动网关并配置Provider
        let client = reqwest::Client::new();
        let request_body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        
        let response = client.post("http://127.0.0.1:1877/v1/chat/completions")
            .json(&request_body)
            .send()
            .await;
        
        // 这个测试需要网关运行并配置Provider
        // assert!(response.is_ok());
    }
}
```

**验收标准**：
- [ ] 端到端测试覆盖核心流程
- [ ] API集成测试覆盖主要端点
- [ ] 测试用例通过
- [ ] 测试环境配置正确

---

### 任务4.2：性能测试

**文件路径**：`tests/performance/`

**任务描述**：
编写启动性能、响应时间、内存占用测试

**实现步骤**：

1. 创建性能测试目录
```bash
mkdir -p tests/performance
```

2. 创建启动性能测试
```rust
// tests/performance/startup_test.rs

#[cfg(test)]
mod tests {
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_database_initialization_performance() {
        // 测试数据库初始化性能
        let start = Instant::now();
        
        let data_dir = unique_temp_dir();
        let _pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        let duration = start.elapsed();
        
        // 数据库初始化应该在2秒内完成
        assert!(duration.as_secs() < 2, "数据库初始化耗时过长: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_gateway_startup_performance() {
        // 测试网关启动性能
        let data_dir = unique_temp_dir();
        let pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        let start = Instant::now();
        
        let (log_sender, _log_receiver) = tokio::sync::mpsc::channel(1);
        let gateway = silk_lib::application::gateway_service::load_gateway_context(pool.clone(), log_sender)
            .await
            .expect("load gateway context");
        
        let duration = start.elapsed();
        
        // 网关上下文加载应该在1秒内完成
        assert!(duration.as_secs() < 1, "网关上下文加载耗时过长: {:?}", duration);
    }
    
    fn unique_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-perf-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
```

3. 创建响应时间测试
```rust
// tests/performance/response_time_test.rs

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::protocol::converter::{ConverterRegistry, ProtocolConverter};
    use crate::protocol::converters::*;
    
    #[tokio::test]
    async fn test_protocol_conversion_performance() {
        // 测试协议转换性能
        let mut registry = ConverterRegistry::new();
        registry.register(std::sync::Arc::new(openai_chat::OpenAIChatConverter::new()));
        registry.register(std::sync::Arc::new(claude_messages::ClaudeMessagesConverter::new()));
        
        let converter = registry.find_converter("openai_chat", "claude_messages").unwrap();
        
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello, how are you?"}]
        });
        
        let body_bytes = serde_json::to_vec(&body).unwrap();
        
        // 预热
        for _ in 0..10 {
            let _ = converter.convert_request(&body_bytes, "openai_chat", "claude_messages").await;
        }
        
        // 测试性能
        let iterations = 100;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = converter.convert_request(&body_bytes, "openai_chat", "claude_messages").await;
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        // 单次转换应该在10ms内完成
        assert!(avg_duration.as_millis() < 10, "协议转换平均耗时过长: {:?}", avg_duration);
    }
}
```

4. 创建内存占用测试
```rust
// tests/performance/memory_test.rs

#[cfg(test)]
mod tests {
    #[test]
    fn test_memory_usage() {
        // 测试内存占用
        // 注意：这需要更复杂的内存监控工具
        // 这里只是示例
        
        let data_dir = unique_temp_dir();
        let pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        // 创建大量数据
        for i in 0..1000 {
            let provider = silk_lib::models::Provider {
                id: format!("provider-{}", i),
                name: format!("Provider {}", i),
                protocols: r#"["openai_chat"]"#.to_string(),
                models: r#"["gpt-4"]"#.to_string(),
                keys: r#"[]"#.to_string(),
                key_strategy: "round_robin".to_string(),
                api_base_url: "https://example.com".to_string(),
                proxy_url: None,
                timeout_seconds: 30,
                max_retries: 0,
                status: "enabled".to_string(),
                health_status: None,
                last_health_check_at: None,
                metadata_json: None,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            };
            
            silk_lib::persistence::ProviderRepo::create(&pool, &provider).await.expect("create provider");
        }
        
        // 验证内存占用在合理范围内
        // 实际实现需要使用内存监控工具
    }
    
    fn unique_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-mem-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
```

**验收标准**：
- [ ] 启动性能测试通过
- [ ] 响应时间测试通过
- [ ] 内存占用测试通过
- [ ] 性能指标达标

---

### 任务4.3：安全测试

**文件路径**：`tests/security/`

**任务描述**：
编写输入验证、权限检查、漏洞扫描测试

**实现步骤**：

1. 创建安全测试目录
```bash
mkdir -p tests/security
```

2. 创建输入验证测试
```rust
// tests/security/input_validation_test.rs

#[cfg(test)]
mod tests {
    use crate::gateway::context::RequestContext;
    use crate::gateway::error::GatewayError;
    use axum::http::{HeaderMap, Method};
    use std::time::Instant;
    
    #[test]
    fn test_sql_injection_prevention() {
        // 测试SQL注入防护
        // 注意：使用参数化查询，SQL注入应该被防止
        
        let malicious_input = "'; DROP TABLE providers; --";
        
        // 尝试创建包含恶意输入的Provider
        // 实际实现需要测试数据库层
    }
    
    #[test]
    fn test_xss_prevention() {
        // 测试XSS防护
        let malicious_input = "<script>alert('xss')</script>";
        
        // 尝试创建包含恶意输入的Provider
        // 实际实现需要测试输出编码
    }
    
    #[test]
    fn test_path_traversal_prevention() {
        // 测试路径遍历防护
        let malicious_path = "../../../etc/passwd";
        
        // 尝试访问恶意路径
        // 实际实现需要测试文件访问权限
    }
    
    #[test]
    fn test_request_size_limit() {
        // 测试请求大小限制
        let large_body = "x".repeat(10 * 1024 * 1024); // 10MB
        
        // 尝试发送大请求
        // 实际实现需要测试请求大小限制
    }
    
    #[test]
    fn test_header_injection_prevention() {
        // 测试HTTP头注入防护
        let malicious_header = "value\r\nX-Injected: true";
        
        // 尝试注入恶意HTTP头
        // 实际实现需要测试HTTP头验证
    }
}
```

3. 创建权限检查测试
```rust
// tests/security/permission_test.rs

#[cfg(test)]
mod tests {
    #[test]
    fn test_file_permission() {
        // 测试文件权限
        // 注意：在Windows上可能需要不同的测试方法
        
        let data_dir = unique_temp_dir();
        let db_path = data_dir.join("silk.db");
        
        // 创建数据库文件
        let _pool = silk_lib::init_database(&data_dir).await.expect("db init");
        
        // 检查文件权限
        // 实际实现需要检查文件ACL
    }
    
    #[test]
    fn test_api_key_protection() {
        // 测试API密钥保护
        // 注意：API密钥应该加密存储
        
        let api_key = "sk-test-key-12345";
        
        // 验证API密钥不以明文存储
        // 实际实现需要检查数据库中的存储格式
    }
    
    fn unique_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("silk-security-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
```

**验收标准**：
- [ ] 输入验证测试通过
- [ ] 权限检查测试通过
- [ ] 漏洞扫描通过
- [ ] 安全指标达标

---

### 任务4.4：代码审查

**文件路径**：无（审查过程）

**任务描述**：
进行代码质量检查、架构一致性检查

**实现步骤**：

1. 创建代码审查检查清单
```markdown
# 代码审查检查清单

## 1. 代码质量

### 1.1 命名规范
- [ ] 变量名清晰易懂
- [ ] 函数名描述功能
- [ ] 常量名全大写
- [ ] 模块名符合规范

### 1.2 代码风格
- [ ] 代码格式一致
- [ ] 缩进正确
- [ ] 空行合理
- [ ] 注释清晰

### 1.3 代码复杂度
- [ ] 函数长度适中（<50行）
- [ ] 嵌套层级适中（<3层）
- [ ] 圈复杂度适中（<10）

## 2. 架构一致性

### 2.1 分层边界
- [ ] GUI层无业务逻辑
- [ ] Application层无HTTP处理
- [ ] Gateway层无协议转换
- [ ] Protocol层无网络通信

### 2.2 依赖关系
- [ ] 无循环依赖
- [ ] 依赖方向正确
- [ ] 接口隔离

### 2.3 模块职责
- [ ] 单一职责
- [ ] 高内聚低耦合
- [ ] 接口清晰

## 3. 错误处理

### 3.1 错误类型
- [ ] 使用自定义错误类型
- [ ] 错误信息清晰
- [ ] 错误码规范

### 3.2 错误传播
- [ ] 错误正确传播
- [ ] 不吞没错误
- [ ] 错误日志记录

### 3.3 用户友好
- [ ] 错误信息用户友好
- [ ] 不暴露技术细节
- [ ] 提供解决建议

## 4. 安全性

### 4.1 输入验证
- [ ] 验证所有输入
- [ ] 防止SQL注入
- [ ] 防止XSS攻击
- [ ] 防止路径遍历

### 4.2 数据保护
- [ ] API密钥加密存储
- [ ] 敏感信息不日志
- [ ] 文件权限正确

### 4.3 网络安全
- [ ] 仅监听本地地址
- [ ] 可选远程访问
- [ ] HTTPS支持

## 5. 性能

### 5.1 内存使用
- [ ] 无内存泄漏
- [ ] 及时释放资源
- [ ] 避免不必要的克隆

### 5.2 并发处理
- [ ] 正确使用异步
- [ ] 避免阻塞操作
- [ ] 正确处理锁

### 5.3 响应时间
- [ ] 响应时间达标
- [ ] 无性能瓶颈
- [ ] 优化热点代码

## 6. 测试

### 6.1 测试覆盖
- [ ] 单元测试覆盖
- [ ] 集成测试覆盖
- [ ] 测试覆盖率达80%

### 6.2 测试质量
- [ ] 测试用例完整
- [ ] 边界条件测试
- [ ] 错误场景测试

### 6.3 测试维护
- [ ] 测试代码清晰
- [ ] 测试独立性
- [ ] 测试可重复

## 7. 文档

### 7.1 代码注释
- [ ] 关键逻辑有注释
- [ ] 注释清晰准确
- [ ] 注释及时更新

### 7.2 API文档
- [ ] API文档完整
- [ ] 参数说明清晰
- [ ] 示例代码正确

### 7.3 架构文档
- [ ] 架构文档完整
- [ ] 设计决策记录
- [ ] 变更日志更新

## 8. 依赖管理

### 8.1 依赖版本
- [ ] 依赖版本锁定
- [ ] 无已知漏洞
- [ ] 定期更新

### 8.2 依赖数量
- [ ] 依赖数量合理
- [ ] 无冗余依赖
- [ ] 依赖冲突解决

## 9. 构建与部署

### 9.1 构建配置
- [ ] 构建配置正确
- [ ] 构建脚本清晰
- [ ] 构建产物完整

### 9.2 部署配置
- [ ] 部署文档完整
- [ ] 部署脚本可用
- [ ] 环境配置清晰

## 10. 其他

### 10.1 代码所有权
- [ ] 代码所有权清晰
- [ ] 责任人明确
- [ ] 变更历史完整

### 10.2 代码审查
- [ ] 代码审查流程
- [ ] 审查意见记录
- [ ] 问题跟踪解决
```

2. 进行代码审查
```bash
# 运行代码检查工具
cargo clippy
cargo fmt --check

# 运行测试
cargo test

# 检查依赖
cargo audit
```

**验收标准**：
- [ ] 代码审查完成
- [ ] 所有检查项通过
- [ ] 问题记录并解决
- [ ] 审查报告完成

---

## 📦 批次交付物

1. `tests/integration/` - 集成测试
2. `tests/performance/` - 性能测试
3. `tests/security/` - 安全测试
4. 代码审查报告

---

## ✅ 批次验收标准

- [ ] 集成测试通过
- [ ] 性能测试通过
- [ ] 安全测试通过
- [ ] 代码审查完成
- [ ] 质量报告完成

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务4.1 | 1.5天 | 第1天 | 第2天上午 |
| 任务4.2 | 1天 | 第2天下午 | 第3天 |
| 任务4.3 | 1天 | 第4天 | 第4天 |
| 任务4.4 | 1天 | 第5天 | 第5天 |
