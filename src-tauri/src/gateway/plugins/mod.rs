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

impl PromptCachePlugin {
    /// 请求体小于该字节数（约 1500~2000 tokens）时不启用缓存，
    /// 避免为短请求付出上游缓存写入的额外开销
    const MIN_CACHE_BODY_BYTES: usize = 6000;
    /// system 提示词超过该长度才值得缓存
    const MIN_CACHE_SYSTEM_CHARS: usize = 1000;

    /// 为 system 提示词注入 cache_control，返回是否发生修改
    fn inject_system_cache(body: &mut serde_json::Value) -> bool {
        let Some(system) = body.get_mut("system") else {
            return false;
        };

        // 普通字符串：转换为块数组后再注入 cache_control
        if let Some(system_str) = system.as_str() {
            if system_str.len() <= Self::MIN_CACHE_SYSTEM_CHARS {
                return false;
            }
            *system = json!([
                {
                    "type": "text",
                    "text": system_str,
                    "cache_control": { "type": "ephemeral" }
                }
            ]);
            return true;
        }

        // 已是块数组：在最后一个元素上注入 cache_control
        if let Some(system_array) = system.as_array_mut() {
            if let Some(obj) = system_array.last_mut().and_then(|b| b.as_object_mut()) {
                obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
                return true;
            }
        }

        false
    }

    /// 为倒数第 2 条消息（通常是上一轮 Assistant 回复）注入 cache_control
    fn inject_history_cache(body: &mut serde_json::Value) -> bool {
        let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
            return false;
        };
        if messages.len() < 3 {
            return false;
        }

        // 倒数第 2 条消息通常是上一轮 Assistant 回复
        let target = messages.len() - 2;
        let Some(content) = messages
            .get_mut(target)
            .and_then(|m| m.get_mut("content"))
        else {
            return false;
        };

        // 普通文本消息转为 block 结构注入
        if let Some(content_str) = content.as_str() {
            *content = json!([
                {
                    "type": "text",
                    "text": content_str,
                    "cache_control": { "type": "ephemeral" }
                }
            ]);
            return true;
        }

        // 已是 block 结构：在最后一个元素上注入
        if let Some(obj) = content
            .as_array_mut()
            .and_then(|blocks| blocks.last_mut())
            .and_then(|b| b.as_object_mut())
        {
            obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
            return true;
        }

        false
    }
}

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
        // 仅对 Claude (anthropic messages 协议) 进行 Prompt 缓存优化
        if ctx.outbound_protocol.as_deref() != Some("claude_messages") {
            return Ok(ctx);
        }
        if ctx.request_body.len() < Self::MIN_CACHE_BODY_BYTES {
            return Ok(ctx);
        }

        let mut modified = false;
        let _ = ctx.edit_body(|body| {
            // 注意用 `|` 而非 `||`：两个注入点都要执行，不能短路
            modified = Self::inject_system_cache(body) | Self::inject_history_cache(body);
        });

        if modified {
            tracing::debug!("PromptCachePlugin: 成功为 Claude 请求注入 Prompt Caching 控制头");
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

impl SlidingWindowPlugin {
    /// 允许保留的最大消息条数
    ///
    /// 1 轮对话通常包含 1 条 user 消息 + 1 条 assistant 消息，
    /// 再加 1 条当前提问。
    fn max_keep_messages(&self) -> usize {
        self.max_window_rounds * 2 + 1
    }

    /// 裁剪历史消息：保留首条 system 提示与最近若干条，中间插入一条裁剪通告
    ///
    /// 仅在消息数超过 `max_keep` 时调用。
    fn prune_messages(messages: &[serde_json::Value], max_keep: usize) -> Vec<serde_json::Value> {
        let discard_count = messages.len() - max_keep;

        // 首条为 system 提示时始终保留：裁剪通告不能取代系统提示
        let keep_system = messages
            .first()
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            == Some("system");
        let history_start = usize::from(keep_system);

        let mut pruned = Vec::with_capacity(max_keep + 2);
        if keep_system {
            pruned.push(messages[0].clone());
        }

        // 注入裁剪说明，告知模型上下文已被网关裁剪，防断层误解
        pruned.push(json!({
            "role": "system",
            "content": format!("[Silk Gateway: Pruned {discard_count} older messages in the conversation history to save tokens]")
        }));

        pruned.extend(messages.iter().skip(history_start + discard_count).cloned());
        pruned
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
        let max_keep = self.max_keep_messages();

        let _ = ctx.edit_body(|body| {
            // 仅处理包含 messages 历史数组的请求
            let Some(messages) = body.get("messages").and_then(|m| m.as_array()) else {
                return;
            };
            if messages.len() <= max_keep {
                return;
            }

            let pruned = Self::prune_messages(messages, max_keep);
            tracing::info!(
                pruned = messages.len() - max_keep,
                remaining = pruned.len(),
                "SlidingWindowPlugin: 对话历史超过上限，已裁剪历史消息"
            );
            body["messages"] = serde_json::Value::Array(pruned);
        });

        Ok(ctx)
    }
}

// ============================================================================
// 3. 终端与构建日志压缩器 (Terminal Log Pruner)
// ============================================================================

pub struct TerminalLogPrunerPlugin;

impl TerminalLogPrunerPlugin {
    /// 文本短于此长度时不做任何处理（避免为短文本付出按行扫描的开销）
    const MIN_COMPRESS_CHARS: usize = 3000;
    /// 行数少于此值时不做折叠
    const MIN_COMPRESS_LINES: usize = 80;
    /// 超过该行数则无论是否像日志都强制折叠
    const FORCE_COMPRESS_LINES: usize = 150;
    /// 判定为日志所需的最小"像日志的行"数量
    const LOG_LINE_SCORE_THRESHOLD: usize = 5;
    /// 折叠时保留的首 / 尾行数
    const EDGE_KEEP_LINES: usize = 25;
    /// 总行数需超过该值才值得折叠（否则折叠后省不下多少）
    const MIN_LINES_TO_COLLAPSE: usize = Self::EDGE_KEEP_LINES * 2 + 10;

    fn compress_text(text: &str) -> String {
        // 文本过短或行数过少：不做任何处理
        if text.len() < Self::MIN_COMPRESS_CHARS {
            return text.to_string();
        }
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < Self::MIN_COMPRESS_LINES {
            return text.to_string();
        }

        // 判定为日志，或行数实在太大则强制折叠
        let is_log = Self::looks_like_terminal_log(&lines) || lines.len() > Self::FORCE_COMPRESS_LINES;
        if !is_log || lines.len() <= Self::MIN_LINES_TO_COLLAPSE {
            return text.to_string();
        }

        Self::collapse_middle(&lines, Self::EDGE_KEEP_LINES)
    }

    /// 简易启发式：检测文本是否是典型的控制台日志或长报错堆栈
    fn looks_like_terminal_log(lines: &[&str]) -> bool {
        lines
            .iter()
            .filter(|line| Self::is_log_line(line.trim()))
            .count()
            > Self::LOG_LINE_SCORE_THRESHOLD
    }

    fn is_log_line(line: &str) -> bool {
        line.starts_with("at ")
            || line.starts_with("warning:")
            || line.starts_with("error:")
            || line.starts_with("[info]")
            || line.starts_with("[warn]")
            || line.starts_with("[error]")
            || line.contains("node_modules")
            || line.contains("stdout")
            || line.contains("stderr")
            || line.contains("Warning:")
            || line.contains("Error:")
    }

    /// 保留首尾各 `edge_keep` 行，中间用一行折叠提示替代
    fn collapse_middle(lines: &[&str], edge_keep: usize) -> String {
        let mut result = String::new();

        for line in lines.iter().take(edge_keep) {
            result.push_str(line);
            result.push('\n');
        }

        result.push_str(&format!(
            "\n... [Silk Gateway: Truncated {} lines of verbose compilation/terminal logs to save tokens] ...\n\n",
            lines.len() - edge_keep * 2
        ));

        for line in lines.iter().skip(lines.len() - edge_keep) {
            result.push_str(line);
            result.push('\n');
        }

        result
    }

    /// 压缩单条消息的内容（支持纯文本与 block 数组两种形态），返回是否发生修改
    fn compress_message_content(content: &mut serde_json::Value) -> bool {
        match content {
            serde_json::Value::String(text) => {
                let compressed = Self::compress_text(text);
                if compressed.len() == text.len() {
                    return false;
                }
                *text = compressed;
                true
            }
            // Claude 等协议的 content 是 block 列表，需逐块处理其中的 text
            serde_json::Value::Array(blocks) => {
                let mut modified = false;
                for block in blocks.iter_mut() {
                    if block.get("type").and_then(|t| t.as_str()) != Some("text") {
                        continue;
                    }
                    let Some(text_value) = block.get_mut("text") else {
                        continue;
                    };
                    let Some(original) = text_value.as_str() else {
                        continue;
                    };

                    let compressed = Self::compress_text(original);
                    if compressed.len() == original.len() {
                        continue;
                    }
                    *text_value = serde_json::Value::String(compressed);
                    modified = true;
                }
                modified
            }
            _ => false,
        }
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
        let _ = ctx.edit_body(|body| {
            let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
                return;
            };

            for msg in messages.iter_mut() {
                // 只压缩用户发送的消息，保留助理的历史回答
                if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
                    continue;
                }
                if let Some(content) = msg.get_mut("content") {
                    Self::compress_message_content(content);
                }
            }
        });

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
