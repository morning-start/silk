use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;

use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::pipeline::StageError;
use crate::gateway::plugin::GatewayPlugin;

// ============================================================================
// 1. Prompt 缓存优化器 (Claude Prompt Caching 自动注入)
// ============================================================================

pub struct PromptCachePlugin;

#[async_trait]
impl GatewayPlugin for PromptCachePlugin {
    fn name(&self) -> &'static str {
        "prompt_cache_optimizer"
    }

    async fn before_upstream(
        &self,
        mut ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        let outbound = ctx.outbound_protocol.as_deref().unwrap_or("");
        
        // 仅对 Claude (anthropic messages 协议) 进行 Prompt 缓存优化
        if outbound != "claude_messages" {
            return Ok(ctx);
        }

        // 获取请求体长度，若总长度小于 6000 字节（大约 1500~2000 tokens），不开启缓存以防上游缓存写入开销
        if ctx.request_body.len() < 6000 {
            return Ok(ctx);
        }

        let mut body: serde_json::Value = match serde_json::from_slice(&ctx.request_body) {
            Ok(json) => json,
            Err(_) => return Ok(ctx), // 解析 JSON 失败，容错直接跳过
        };

        let mut modified = false;

        // 1. 注入 System Prompt 缓存
        if let Some(system) = body.get_mut("system") {
            // 如果 system 是普通字符串，将其转换为块数组并注入 cache_control
            if let Some(system_str) = system.as_str() {
                if system_str.len() > 1000 {
                    *system = json!([
                        {
                            "type": "text",
                            "text": system_str,
                            "cache_control": { "type": "ephemeral" }
                        }
                    ]);
                    modified = true;
                }
            } else if let Some(system_array) = system.as_array_mut() {
                // 如果已经是数组，在最后一个元素注入 cache_control
                if let Some(last_block) = system_array.last_mut() {
                    if let Some(obj) = last_block.as_object_mut() {
                        obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
                        modified = true;
                    }
                }
            }
        }

        // 2. 注入 Messages 历史缓存 (根据 Claude 推荐：对倒数第 2 条消息 — 通常是上一轮 Assistant 回复 — 注入缓存)
        if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            let msg_len = messages.len();
            if msg_len >= 3 {
                // 找到倒数第 2 条消息
                if let Some(target_msg) = messages.get_mut(msg_len - 2) {
                    if let Some(content) = target_msg.get_mut("content") {
                        if let Some(content_str) = content.as_str() {
                            // 普通文本消息转为 block 结构注入
                            *content = json!([
                                {
                                    "type": "text",
                                    "text": content_str,
                                    "cache_control": { "type": "ephemeral" }
                                }
                            ]);
                            modified = true;
                        } else if let Some(content_array) = content.as_array_mut() {
                            // 已经是 block 结构，在最后一个元素注入
                            if let Some(last_block) = content_array.last_mut() {
                                if let Some(obj) = last_block.as_object_mut() {
                                    obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
                                    modified = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if modified {
            if let Ok(new_body) = serde_json::to_vec(&body) {
                ctx.request_body = bytes::Bytes::from(new_body);
                tracing::debug!("PromptCachePlugin: 成功为 Claude 请求注入 Prompt Caching 控制头");
            }
        }

        Ok(ctx)
    }
}

// ============================================================================
// 2. 对话历史滑动窗口插件 (Sliding Window History Pruner)
// ============================================================================

pub struct SlidingWindowPlugin {
    pub max_window_rounds: usize,
}

impl Default for SlidingWindowPlugin {
    fn default() -> Self {
        Self {
            max_window_rounds: 12, // 默认保留最近 12 轮（24条历史消息）
        }
    }
}

#[async_trait]
impl GatewayPlugin for SlidingWindowPlugin {
    fn name(&self) -> &'static str {
        "sliding_window_pruner"
    }

    async fn before_upstream(
        &self,
        mut ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        let mut body: serde_json::Value = match serde_json::from_slice(&ctx.request_body) {
            Ok(json) => json,
            Err(_) => return Ok(ctx),
        };

        // 仅处理包含 messages 历史数组的请求
        let messages = match body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            Some(m) => m,
            None => return Ok(ctx),
        };

        let total_messages = messages.len();
        // 1 轮对话通常包含 1条 user 消息 + 1条 assistant 消息。
        // 保留的条数 = max_window_rounds * 2 + 1 (当前提问)
        let max_keep_messages = self.max_window_rounds * 2 + 1;

        if total_messages > max_keep_messages {
            let mut pruned_messages = Vec::new();
            
            // 1. 保留第 1 条消息（如果是 system 提示词，则极为关键）
            let mut start_idx = 0;
            if let Some(first_msg) = messages.first() {
                if first_msg.get("role").and_then(|r| r.as_str()) == Some("system") {
                    pruned_messages.push(first_msg.clone());
                    start_idx = 1;
                }
            }

            // 2. 计算需要截断丢弃的数量，并加入裁剪通告
            let discard_count = total_messages - max_keep_messages;
            let skip_until = start_idx + discard_count;

            // 注入裁剪说明占位符，告知模型上下文已被网关裁剪，防断层误解
            pruned_messages.push(json!({
                "role": "system",
                "content": format!("[Silk Gateway: Pruned {} older messages in the conversation history to save tokens]", discard_count)
            }));

            // 3. 保留最近的历史记录
            for idx in skip_until..total_messages {
                pruned_messages.push(messages[idx].clone());
            }

            tracing::info!(
                pruned = discard_count,
                remaining = pruned_messages.len(),
                "SlidingWindowPlugin: 对话历史超过上限，已裁剪历史消息"
            );

            *messages = pruned_messages;

            if let Ok(new_body) = serde_json::to_vec(&body) {
                ctx.request_body = bytes::Bytes::from(new_body);
            }
        }

        Ok(ctx)
    }
}

// ============================================================================
// 3. 终端与构建日志压缩器 (Terminal Log Pruner)
// ============================================================================

pub struct TerminalLogPrunerPlugin;

impl TerminalLogPrunerPlugin {
    fn compress_text(&self, text: &str) -> String {
        // 如果文本过短，不做任何耗时的正则与按行处理
        if text.len() < 3000 {
            return text.to_string();
        }

        let lines: Vec<&str> = text.lines().collect();
        // 行数较少，不进行折叠
        if lines.len() < 80 {
            return text.to_string();
        }

        // 简易启发式得分：检测是否是典型的控制台日志或长报错堆栈
        let mut log_line_score = 0;
        for line in &lines {
            let l = line.trim();
            if l.starts_with("at ")
                || l.starts_with("warning:")
                || l.starts_with("error:")
                || l.contains("node_modules")
                || l.contains("stdout")
                || l.contains("stderr")
                || l.starts_with("[info]")
                || l.starts_with("[warn]")
                || l.starts_with("[error]")
                || l.contains("Warning:")
                || l.contains("Error:")
            {
                log_line_score += 1;
            }
        }

        // 得分表明是日志，或者长度实在太大 (超过 150 行)
        if log_line_score > 5 || lines.len() > 150 {
            let first_keep = 25; // 保留开始的 25 行（报错入口）
            let last_keep = 25;  // 保留最后的 25 行（报错结果/总结）
            
            if lines.len() > (first_keep + last_keep + 10) {
                let mut result = String::new();
                for i in 0..first_keep {
                    result.push_str(lines[i]);
                    result.push('\n');
                }
                
                result.push_str(&format!(
                    "\n... [Silk Gateway: Truncated {} lines of verbose compilation/terminal logs to save tokens] ...\n\n",
                    lines.len() - first_keep - last_keep
                ));
                
                for i in (lines.len() - last_keep)..lines.len() {
                    result.push_str(lines[i]);
                    result.push('\n');
                }
                
                return result;
            }
        }

        text.to_string()
    }
}

#[async_trait]
impl GatewayPlugin for TerminalLogPrunerPlugin {
    fn name(&self) -> &'static str {
        "terminal_log_pruner"
    }

    async fn before_upstream(
        &self,
        mut ctx: RequestContext,
        _runtime: &GatewayContext,
    ) -> Result<RequestContext, StageError> {
        let mut body: serde_json::Value = match serde_json::from_slice(&ctx.request_body) {
            Ok(json) => json,
            Err(_) => return Ok(ctx),
        };

        let messages = match body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            Some(m) => m,
            None => return Ok(ctx),
        };

        let mut modified = false;

        for msg in messages {
            // 只压缩用户发送的消息，保留助理的历史回答
            if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
                continue;
            }

            if let Some(content) = msg.get_mut("content") {
                if let Some(content_str) = content.as_str() {
                    let compressed = self.compress_text(content_str);
                    if compressed.len() != content_str.len() {
                        *content = serde_json::Value::String(compressed);
                        modified = true;
                    }
                } else if let Some(content_array) = content.as_array_mut() {
                    // 处理 content 为 block 列表的场景（例如 Claude 消息块）
                    for block in content_array {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            if let Some(text_val) = block.get_mut("text") {
                                if let Some(text_str) = text_val.as_str() {
                                    let compressed = self.compress_text(text_str);
                                    if compressed.len() != text_str.len() {
                                        *text_val = serde_json::Value::String(compressed);
                                        modified = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if modified {
            if let Ok(new_body) = serde_json::to_vec(&body) {
                ctx.request_body = bytes::Bytes::from(new_body);
            }
        }

        Ok(ctx)
    }
}

// ============================================================================
// 插件注册与装载辅助函数
// ============================================================================

/// 获取默认装载的所有 Token 节省插件列表
pub fn default_token_saving_plugins() -> Vec<Arc<dyn GatewayPlugin>> {
    vec![
        Arc::new(PromptCachePlugin),
        Arc::new(SlidingWindowPlugin::default()),
        Arc::new(TerminalLogPrunerPlugin),
    ]
}
