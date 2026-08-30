use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::gateway::error::GatewayError;
use crate::protocol::prism_wasm;

/// 流式响应类型
pub enum StreamResponse {
    /// 非流式响应（一次性 body）
    Single {
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
    },
    /// SSE 流式响应
    Sse {
        status: StatusCode,
        headers: HeaderMap,
        stream: Box<dyn Stream<Item = Result<Bytes, GatewayError>> + Send + Unpin>,
    },
}

impl StreamResponse {
    /// 是否为流式响应
    pub fn is_streaming(&self) -> bool {
        matches!(self, StreamResponse::Sse { .. })
    }

    /// 获取状态码
    pub fn status(&self) -> StatusCode {
        match self {
            StreamResponse::Single { status, .. } | StreamResponse::Sse { status, .. } => *status,
        }
    }

    /// 获取响应头
    pub fn headers(&self) -> &HeaderMap {
        match self {
            StreamResponse::Single { headers, .. } | StreamResponse::Sse { headers, .. } => headers,
        }
    }

    /// 构建 axum Response
    pub fn into_response(self) -> Response {
        match self {
            StreamResponse::Single {
                status,
                headers,
                body,
            } => {
                let mut builder = Response::builder().status(status);
                if let Some(h) = builder.headers_mut() {
                    for (k, v) in &headers {
                        h.insert(k.clone(), v.clone());
                    }
                }
                builder
                    .body(Body::from(body))
                    .unwrap_or_else(|e| GatewayError::Internal(e.to_string()).into_response())
            }
            StreamResponse::Sse {
                status,
                headers,
                stream,
            } => {
                let mut builder = Response::builder().status(status);
                if let Some(h) = builder.headers_mut() {
                    for (k, v) in &headers {
                        if should_forward_sse_header(k) {
                            h.insert(k.clone(), v.clone());
                        }
                    }
                }
                builder
                    .body(Body::from_stream(stream))
                    .unwrap_or_else(|e| GatewayError::Internal(e.to_string()).into_response())
            }
        }
    }
}

/// SSE 流配置
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// 流超时（无数据推送的最大等待时间）
    pub stream_timeout: Duration,
    /// 心跳间隔（发送 keep-alive 注释的间隔）
    pub heartbeat_interval: Duration,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试初始退避时间
    pub initial_backoff: Duration,
    /// 最大退避时间
    pub max_backoff: Duration,
    /// 读取缓冲区大小（字节）
    pub read_buffer_size: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            stream_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(15),
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(8),
            read_buffer_size: 8192,
        }
    }
}

/// SSE 事件数据
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// 事件类型（event: xxx）
    pub event: Option<String>,
    /// 数据字段（data: xxx）
    pub data: Option<String>,
    /// 事件 ID
    pub id: Option<String>,
    /// 重试间隔（毫秒）
    pub retry: Option<u64>,
    /// 注释（:keep-alive 等）
    pub comment: Option<String>,
}

impl SseEvent {
    /// 序列化为 SSE 格式文本
    pub fn serialize(&self) -> String {
        let mut output = String::new();

        if let Some(ref comment) = self.comment {
            output.push_str(&format!(": {comment}\n"));
        }
        if let Some(ref id) = self.id {
            output.push_str(&format!("id: {id}\n"));
        }
        if let Some(ref event) = self.event {
            output.push_str(&format!("event: {event}\n"));
        }
        if let Some(ref retry) = self.retry {
            output.push_str(&format!("retry: {retry}\n"));
        }
        if let Some(ref data) = self.data {
            for line in data.lines() {
                output.push_str(&format!("data: {line}\n"));
            }
        }

        if !output.is_empty() {
            output.push('\n');
        }
        output
    }

    /// 是否为流结束标记
    ///
    /// OpenAI 用 `[DONE]`，Anthropic 用 `message_stop`，Responses 用 `response.completed`。
    /// 同时检查 event: 字段和 data 中的 type 字段，兼容不同服务器格式。
    pub fn is_end(&self) -> bool {
        if self.data.as_deref() == Some("[DONE]") {
            return true;
        }
        // 检查 event: 字段（SSE 规范的事件类型）
        if let Some(ref event_name) = self.event {
            if event_name == "response.completed" || event_name == "message_stop" {
                return true;
            }
        }
        // 检查 data: 中的 type 字段
        if let Some(ref data) = self.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
                    if event_type == "message_stop" || event_type == "response.completed" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 从事件的 data 中解析 token 用量（data 非 JSON 或不含 usage 时返回 None）
    pub fn parse_usage(&self) -> Option<Usage> {
        let json = serde_json::from_str::<serde_json::Value>(self.data.as_deref()?).ok()?;
        self.extract_usage(&json)
    }

    /// 是否为 OpenAI 系流结束信号帧（choices[0].finish_reason 非空）。
    ///
    /// 用于增量转换：检测到 finish_reason 后流进入收尾阶段（随后是 usage 帧、
    /// [DONE]），停止增量 flush，避免 prism 对相同前缀输出不一致的
    /// response.completed（其内容依赖向后查找 Usage）破坏前缀 diff。
    pub fn has_finish_reason(&self) -> bool {
        let data = match self.data.as_deref() {
            Some(d) => d,
            None => return false,
        };
        let json: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return false,
        };
        json.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|ch| ch.get("finish_reason"))
            .and_then(|fr| fr.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    /// 推断事件类型
    pub fn infer_type(&self) -> StreamEventType {
        // 1. 检查是否为 [DONE] 标记
        if self.data.as_deref() == Some("[DONE]") {
            return StreamEventType::ResponseStop;
        }

        // 1.5 检查 event: 字段（SSE 规范的事件类型）
        if let Some(ref event_name) = self.event {
            match event_name.as_str() {
                "response.completed" | "message_stop" => return StreamEventType::ResponseStop,
                "response.created" | "message_start" => return StreamEventType::ResponseStart,
                "response.output_text.delta" | "content_block_delta" => return StreamEventType::ContentDelta,
                // content_block_stop / response.output_text.done 是块边界事件，不是流结束；
                // 归类为 ContentDelta 以避免 is_end() 误判导致流提前关闭。
                "response.output_text.done" | "content_block_stop" => return StreamEventType::ContentDelta,
                _ => {}
            }
        }

        // 2. 解析 JSON 推断类型
        if let Some(ref data) = self.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // OpenAI: {"choices":[{"delta":{"content":"..."}}]}
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    if let Some(first) = choices.first() {
                        // 检查 finish_reason（优先级高于 delta）
                        if let Some(finish_reason) = first.get("finish_reason") {
                            if finish_reason != "null" && !finish_reason.is_null() {
                                return StreamEventType::ResponseStop;
                            }
                        }
                        // 检查 delta
                        if first.get("delta").is_some() {
                            return StreamEventType::ContentDelta;
                        }
                    }
                }

                // Anthropic: {"type":"content_block_delta","delta":{"text":"..."}}
                if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        // Anthropic (messages)
                        "message_start" => return StreamEventType::ResponseStart,
                        "content_block_start" => return StreamEventType::ResponseStart,
                        "content_block_delta" => return StreamEventType::ContentDelta,
                        // content_block_stop 是块边界事件，不是流结束；
                        // 归类为 ContentDelta 以避免 is_end() 误判导致流提前关闭。
                        "content_block_stop" => return StreamEventType::ContentDelta,
                        "message_stop" => return StreamEventType::ResponseStop,
                        // Responses
                        "response.created" => return StreamEventType::ResponseStart,
                        "response.output_item.added" => return StreamEventType::ResponseStart,
                        "response.content_part.added" => return StreamEventType::ResponseStart,
                        "response.output_text.delta" => return StreamEventType::ContentDelta,
                        // response.output_text.done / reasoning_summary_text.done /
                        // function_call_arguments.done 都是块边界事件，不是流结束。
                        "response.output_text.done" => return StreamEventType::ContentDelta,
                        "response.reasoning_summary_text.delta" => return StreamEventType::ContentDelta,
                        "response.reasoning_summary_text.done" => return StreamEventType::ContentDelta,
                        "response.function_call_arguments.delta" => return StreamEventType::ContentDelta,
                        "response.function_call_arguments.done" => return StreamEventType::ContentDelta,
                        "response.completed" => return StreamEventType::ResponseStop,
                        _ => {}
                    }
                }

                // Usage 事件（OpenAI 和 Anthropic 都可能有）
                if json.get("usage").is_some() {
                    return StreamEventType::Usage;
                }
            }
        }

        // 默认为内容增量
        StreamEventType::ContentDelta
    }

    /// 转换为规范化流式事件
    pub fn to_stream_event(&self) -> StreamEvent {
        let event_type = self.infer_type();
        let mut event = StreamEvent {
            id: self.id.clone(),
            model: None,
            event_type: event_type.clone(),
            role: None,
            delta: None,
            stop_reason: None,
            usage: None,
            sequence: None,
            metadata: None,
        };

        // 解析 JSON 提取字段
        if let Some(ref data) = self.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // 提取 id（优先从 JSON 中提取，其次从 SSE id 字段）
                if event.id.is_none() {
                    event.id = json.get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }

                // 提取 model
                event.model = json.get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // 提取 role
                event.role = json.get("role")
                    .or_else(|| json.get("choices").and_then(|c| c.as_array()?.first()?.get("delta")?.get("role")))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // 提取 delta
                event.delta = self.extract_delta(&json, &event_type);

                // 提取 stop_reason
                event.stop_reason = json.get("finish_reason")
                    .or_else(|| json.get("choices").and_then(|c| c.as_array()?.first()?.get("finish_reason")))
                    .and_then(|v| v.as_str())
                    .filter(|s| *s != "null")
                    .map(|s| s.to_string());

                // 提取 usage
                event.usage = self.extract_usage(&json);
            }
        }

        event
    }

    /// 提取增量内容
    fn extract_delta(&self, json: &serde_json::Value, event_type: &StreamEventType) -> Option<ContentDelta> {
        match event_type {
            StreamEventType::ContentDelta => {
                // OpenAI: {"choices":[{"delta":{"content":"..."}}]}
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    if let Some(first) = choices.first() {
                        if let Some(delta) = first.get("delta") {
                            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                return Some(ContentDelta {
                                    text: Some(content.to_string()),
                                    tool_calls: None,
                                });
                            }
                        }
                    }
                }

                // Anthropic: {"type":"content_block_delta","delta":{"text":"..."}}
                if let Some(delta) = json.get("delta") {
                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                        return Some(ContentDelta {
                            text: Some(text.to_string()),
                            tool_calls: None,
                        });
                    }
                }

                None
            }
            _ => None,
        }
    }

    /// 从已解析的 JSON 中提取 usage 信息（兼容 OpenAI / Anthropic / Responses 字段名与嵌套位置）
    pub fn extract_usage(&self, json: &serde_json::Value) -> Option<Usage> {
        // 直接在顶层查找 usage
        let usage_json = json.get("usage")
            // Anthropic: {"message":{"usage":{...}}}
            .or_else(|| json.get("message").and_then(|m| m.get("usage")))
            // Responses: {"response":{"usage":{...}}}
            .or_else(|| json.get("response").and_then(|r| r.get("usage")))?;

        let input_tokens = usage_json.get("prompt_tokens")
            .or_else(|| usage_json.get("input_tokens"))
            .and_then(|v| v.as_i64());

        let output_tokens = usage_json.get("completion_tokens")
            .or_else(|| usage_json.get("output_tokens"))
            .and_then(|v| v.as_i64());

        let total_tokens = usage_json.get("total_tokens")
            .and_then(|v| v.as_i64());

        Some(Usage {
            input_tokens,
            output_tokens,
            total_tokens,
        })
    }
}

// ---------------------------------------------------------------------------
// 规范化流式事件（参考 OpenTrans StreamEvent）
// ---------------------------------------------------------------------------

/// 流式事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    /// 响应开始
    ResponseStart,
    /// 内容增量
    ContentDelta,
    /// 响应停止
    ResponseStop,
    /// Usage 信息
    Usage,
}

/// 增量内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDelta {
    /// 文本内容
    pub text: Option<String>,
    /// 工具调用
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 调用 ID
    pub id: String,
    /// 函数名称
    pub name: String,
    /// 函数参数（JSON 字符串）
    pub arguments: String,
}

/// Token 用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// 输入 tokens
    pub input_tokens: Option<i64>,
    /// 输出 tokens
    pub output_tokens: Option<i64>,
    /// 总 tokens
    pub total_tokens: Option<i64>,
}

/// 规范化流式事件（参考 OpenTrans StreamEvent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// 事件 ID
    pub id: Option<String>,
    /// 模型名称
    pub model: Option<String>,
    /// 事件类型
    pub event_type: StreamEventType,
    /// 角色（assistant/user）
    pub role: Option<String>,
    /// 增量内容
    pub delta: Option<ContentDelta>,
    /// 停止原因
    pub stop_reason: Option<String>,
    /// Token 用量
    pub usage: Option<Usage>,
    /// 事件序号
    pub sequence: Option<u64>,
    /// 扩展元数据
    pub metadata: Option<serde_json::Value>,
}

impl StreamEvent {
    /// 提取 usage 信息
    pub fn extract_usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// 提取 prompt tokens
    pub fn prompt_tokens(&self) -> Option<i64> {
        self.usage.as_ref()?.input_tokens
    }

    /// 提取 completion tokens
    pub fn completion_tokens(&self) -> Option<i64> {
        self.usage.as_ref()?.output_tokens
    }
}

/// SSE 解析器：将字节流解析为 SseEvent
pub struct SseParser {
    buffer: bytes::BytesMut,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: bytes::BytesMut::new(),
        }
    }

    /// 喂入数据块，返回解析出的事件
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);

        // 防止 buffer 无限增长：超过 1MB 时截断
        const MAX_BUFFER_SIZE: usize = 1024 * 1024;
        if self.buffer.len() > MAX_BUFFER_SIZE {
            let split_idx = self.buffer.len() - MAX_BUFFER_SIZE / 2;
            let _ = self.buffer.split_to(split_idx);
        }

        let mut events = Vec::new();

        while let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            let raw_bytes = self.buffer.split_to(pos);
            let _ = self.buffer.split_to(2); // Skip the \n\n

            let raw_str = String::from_utf8_lossy(&raw_bytes);
            if let Some(event) = Self::parse_event(&raw_str) {
                events.push(event);
            }
        }

        events
    }

    fn parse_event(raw: &str) -> Option<SseEvent> {
        let mut event = SseEvent {
            event: None,
            data: None,
            id: None,
            retry: None,
            comment: None,
        };

        let mut has_data = false;

        for line in raw.lines() {
            if line.starts_with(':') {
                // SSE 规范：注释以 : 开头，可选空格
                let comment = line.strip_prefix(": ").or_else(|| line.strip_prefix(':'));
                event.comment = comment.map(|s| s.to_string());
            } else if let Some(rest) = line.strip_prefix("event:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                event.event = Some(rest.to_string());
                has_data = true;
            } else if let Some(rest) = line.strip_prefix("data:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE 规范：多个 data: 字段用 \n 拼接
                event.data = match event.data {
                    Some(existing) => Some(format!("{existing}\n{rest}")),
                    None => Some(rest.to_string()),
                };
                has_data = true;
            } else if let Some(rest) = line.strip_prefix("id:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                // SSE 规范：id 字段包含 null 字符时应忽略
                if !rest.contains('\0') {
                    event.id = Some(rest.to_string());
                    has_data = true;
                }
            } else if let Some(rest) = line.strip_prefix("retry:")
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
            {
                event.retry = rest.parse().ok();
                has_data = true;
            }
        }

        if has_data {
            Some(event)
        } else {
            None
        }
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 判断是否为 SSE 响应
pub fn is_sse_response(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or(false)
}

fn should_forward_sse_header(name: &HeaderName) -> bool {
    !matches!(
        name,
        &axum::http::header::CONTENT_LENGTH
            | &axum::http::header::CONTENT_ENCODING
            | &axum::http::header::TRANSFER_ENCODING
            | &axum::http::header::CONNECTION
    )
}

/// 创建 SSE 心跳注释
pub fn heartbeat_comment() -> Bytes {
    Bytes::from(": keep-alive\n\n")
}

/// 创建流结束标记
pub fn stream_end_marker() -> Bytes {
    Bytes::from("data: [DONE]\n\n")
}

/// 流状态追踪
#[derive(Debug)]
pub struct StreamState {
    pub bytes_received: u64,
    pub events_received: u64,
    pub last_data_at: std::time::Instant,
    pub ended: bool,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            bytes_received: 0,
            events_received: 0,
            last_data_at: std::time::Instant::now(),
            ended: false,
        }
    }

    pub fn record_data(&mut self, bytes: usize) {
        self.bytes_received += bytes as u64;
        self.last_data_at = std::time::Instant::now();
    }

    pub fn record_event(&mut self) {
        self.events_received += 1;
    }

    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        self.last_data_at.elapsed() > timeout
    }
}

impl Default for StreamState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SSE 协议转换器（从 dispatch_upstream 拆分）
// ---------------------------------------------------------------------------

/// 流式 SSE 事件协议转换器（基于 prism.wasm 单事件转换）
///
/// 使用 prism 的 `wasm_convert_stream_event` 逐事件转换，无状态、无累积：
/// - 每个 SSE 事件独立转换（O(1) 调用，无 buffer 膨胀）
/// - 返回值可能是空、单个或多个 SSE 事件（由 prism 映射逻辑决定）
/// - 自动过滤空事件和 `[DONE]`（由 dispatch 统一发送流结束标记）
pub struct SseConverter {
    /// 是否需要转换（inbound != outbound 且协议可映射）
    enabled: bool,
    /// 源协议（上游，silk 协议名）
    source: String,
    /// 目标协议（下游，silk 协议名）
    target: String,
    /// 已转换事件计数
    event_count: u64,
    /// 转换失败计数
    error_count: u64,
    /// 原始 SSE 文本累积缓冲（用于批量转换）
    accumulated: String,
    /// 上次增量转换时已下发的输出字节数（前缀 diff 游标）
    last_output_len: usize,
    /// 上次增量转换时的输入累积字节数（阈值判断游标）
    last_flushed_input_len: usize,
    /// 上游已发送 finish_reason（流进入收尾阶段）：此后停止增量 flush，
    /// 剩余事件（usage/[DONE]）累积到 finish() 一次性转换。
    /// 原因：prism 的 response.completed 依赖向后查找 Usage，若在 usage 帧到达前
    /// 增量转换，会对相同前缀输入输出不同的 completed（无 usage → 带 usage），
    /// 破坏前缀 diff 的确定性，客户端会收到从 "usage" 中间切断的乱码事件，
    /// AI SDK 校验失败中止流，OpenCode 自动重试（表现为"思考两遍、回答两遍"）。
    ending_seen: bool,
}

/// 增量转换的输入累积阈值：原始 SSE 累积超过该字节数即触发一次全量转换并下发新增部分。
/// 数值越大实时性越差（客户端等更久才看到内容），越小转换越频繁（CPU 开销略增）。
const INCREMENTAL_FLUSH_THRESHOLD: usize = 1024;

impl SseConverter {
    pub fn new(inbound: &str, outbound: &str) -> Self {
        let inbound_mapped = prism_wasm::map_provider(inbound);
        let outbound_mapped = prism_wasm::map_provider(outbound);
        let enabled = inbound != outbound
            && !inbound.is_empty()
            && !outbound.is_empty()
            && inbound_mapped.is_some()
            && outbound_mapped.is_some();

        tracing::debug!(
            inbound,
            outbound,
            inbound_mapped = ?inbound_mapped,
            outbound_mapped = ?outbound_mapped,
            enabled,
            "SseConverter 初始化"
        );

        if enabled {
            tracing::info!(
                source = outbound,
                target = inbound,
                "SSE 流式协议转换已启用"
            );
        }
        Self {
            enabled,
            source: outbound.to_string(),
            target: inbound.to_string(),
            event_count: 0,
            error_count: 0,
            accumulated: String::new(),
            last_output_len: 0,
            last_flushed_input_len: 0,
            ending_seen: false,
        }
    }

    /// 规范化：SseEvent → StreamEvent
    pub fn normalize(&self, event: &SseEvent) -> StreamEvent {
        event.to_stream_event()
    }

    /// 序列化：StreamEvent → Bytes（当前未使用，保留作为未来 API）
    pub fn marshal(&self, _event: &StreamEvent) -> Result<Bytes, String> {
        // TODO: 实现 StreamEvent 到目标协议的序列化
        // 当前保留作为未来 API，convert() 方法仍使用原有逻辑
        Err("marshal() 方法尚未实现".to_string())
    }

    /// 逐事件转换：累积原始 SSE 文本，达到阈值时增量转换并下发新增部分。
    ///
    /// 背景：prism 的逐事件转换（convert_stream_event）是无状态的，Responses 格式的
    /// BlockStart 每次都被重新生成，产生重复的 response.output_item.added；因此改为
    /// 累积原始 SSE、用有状态的 convert_stream 批量转换。
    ///
    /// 实时性：convert_stream 是确定性纯函数（相同输入 → 相同输出），输入按前缀累积，
    /// 输出也按前缀增长。利用这一性质，每累积满阈值就全量转换一次，用前缀 diff
    /// （过滤后输出中 last_output_len 之后的部分）只下发新增事件，实现流式实时下发，
    /// 而不是等到流结束才一次性全量输出。
    pub fn convert(&mut self, event: &SseEvent) -> Result<Bytes, String> {
        if !self.enabled {
            return Ok(Bytes::from(event.serialize()));
        }
        // 结束事件（如 [DONE]）不累积：收尾事件由 finish() 统一生成，
        // 避免 prism 将其转换为重复的 message_stop / [DONE]。
        if event.is_end() {
            return Ok(Bytes::new());
        }
        self.accumulated.push_str(&event.serialize());
        self.event_count += 1;

        // 检测上游 finish_reason（OpenAI 系流结束信号），标记进入收尾阶段。
        // 此后（usage / 收尾帧）不再增量 flush，统一由 finish() 一次性转换：
        // prism 的 response.completed 会向后查找 Usage，若在此前增量转换，
        // 相同前缀输入会输出不同的 completed，破坏前缀 diff 确定性。
        if !self.ending_seen && event.has_finish_reason() {
            self.ending_seen = true;
            tracing::debug!("检测到上游 finish_reason，停止增量转换，收尾交给 finish()");
        }

        // 累积超过阈值且未进入收尾阶段：全量转换，前缀 diff 只返回新增部分
        if !self.ending_seen
            && self.accumulated.len() - self.last_flushed_input_len >= INCREMENTAL_FLUSH_THRESHOLD
        {
            self.flush_incremental()
        } else {
            Ok(Bytes::new())
        }
    }

    /// 增量转换：对全部累积 SSE 做一次确定性全量转换，只返回上次未下发的新增部分。
    ///
    /// 因为 prism 转换是纯函数，输入前缀 → 输出前缀，所以本次输出是上次输出的
    /// 前缀扩展，`last_output_len` 之后的字节就是新事件，可直接下发。
    fn flush_incremental(&mut self) -> Result<Bytes, String> {
        if !self.enabled || self.accumulated.is_empty() {
            return Ok(Bytes::new());
        }
        let input_len = self.accumulated.len();
        let full = prism_wasm::convert_stream(&self.source, &self.accumulated, &self.target)?;
        let filtered = filter_empty_events(&full);
        let new_output = if filtered.len() > self.last_output_len {
            filtered[self.last_output_len..].to_string()
        } else {
            String::new()
        };
        self.last_output_len = filtered.len();
        self.last_flushed_input_len = input_len;
        tracing::debug!(
            new_bytes = new_output.len(),
            total_output = filtered.len(),
            input_bytes = input_len,
            "增量转换下发"
        );
        Ok(Bytes::from(new_output))
    }

    /// 流结束冲刷：先下发累积的增量内容，再为目标协议生成正确的收尾事件。
    ///
    /// 不同协议的流结束信号不同：
    /// - Anthropic Messages: `message_delta`(stop_reason) + `message_stop`
    /// - OpenAI Chat: `finish_reason:stop` chunk + `[DONE]`
    /// - OpenAI Responses: `response.completed`
    ///
    /// 收尾事件直接生成，不依赖 prism 转换（prism 可能无法正确处理
    /// 跨协议的结束信号，如 responses→messages 的 response.completed）。
    pub fn finish(&mut self) -> Result<Bytes, String> {
        if !self.enabled {
            return Ok(Bytes::new());
        }
        tracing::info!(
            source = %self.source,
            target = %self.target,
            events_converted = self.event_count,
            errors = self.error_count,
            "SSE 流式转换完成"
        );

        // 先冲刷累积的增量内容（正文事件），确保消息内容已下发
        let mut output: Vec<u8> = Vec::new();
        if !self.accumulated.is_empty() {
            match self.flush_incremental() {
                Ok(bytes) => output.extend_from_slice(&bytes),
                Err(e) => tracing::warn!(error = %e, "增量转换冲刷失败"),
            }
        }

        // 为目标协议生成收尾事件（不经过 prism）
        match self.target.as_str() {
            "messages" => {
                // Anthropic Messages 需要 message_delta + message_stop
                // message_delta 携带 stop_reason 和 usage，是客户端判断流结束的关键信号
                output.extend_from_slice(
                    concat!(
                        "event: message_delta\n",
                        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":0}}\n\n",
                        "event: message_stop\n",
                        "data: {\"type\":\"message_stop\"}\n\n",
                    )
                    .as_bytes(),
                );
            }
            "openai" => {
                // OpenAI Chat 需要 finish_reason:stop chunk（[DONE] 由 dispatch 统一发送）
                let done_event = "data: [DONE]\n\n";
                match prism_wasm::convert_stream_event(&self.source, done_event, &self.target) {
                    Ok(converted) => {
                        let filtered = filter_empty_events(&converted);
                        if !filtered.is_empty() {
                            output.extend_from_slice(filtered.as_bytes());
                        } else {
                            // prism 无输出时用硬编码兜底
                            output.extend_from_slice(
                                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"
                                    .as_bytes(),
                            );
                        }
                    }
                    Err(_) => {
                        output.extend_from_slice(
                            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"
                                .as_bytes(),
                        );
                    }
                }
            }
            "responses" => {
                // 增量模式：累积内容（含 finish_reason → response.completed）已在
                // 上方 flush_incremental 中下发，无需额外收尾事件
            }
            _ => {
                // 未知协议：尝试用 prism 冲刷
                let done_event = "data: [DONE]\n\n";
                match prism_wasm::convert_stream_event(&self.source, done_event, &self.target) {
                    Ok(converted) => {
                        let filtered = filter_empty_events(&converted);
                        if !filtered.is_empty() {
                            output.extend_from_slice(filtered.as_bytes());
                        }
                    }
                    Err(_) => {}
                }
            }
        };

        if output.is_empty() {
            return Ok(Bytes::new());
        }
        tracing::debug!(
            target = %self.target,
            closing_preview = %String::from_utf8_lossy(&output).chars().take(300).collect::<String>(),
            "生成目标协议收尾事件"
        );
        Ok(Bytes::from(output))
    }
}

/// 过滤无 `data:` 行的空 SSE 事件，并剥离末尾 `data: [DONE]`。
///
/// prism 对 content_block_start 等事件会输出空占位（`\n\n`），需过滤；
/// `[DONE]` 由 dispatch 统一发送流结束标记，避免重复。

fn filter_empty_events(sse: &str) -> String {
    let blocks: Vec<&str> = sse
        .split("\n\n")
        .filter(|b| b.contains("data:") && !b.trim_end().ends_with("data: [DONE]"))
        .collect();
    if blocks.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", blocks.join("\n\n"))
    }
}

// ---------------------------------------------------------------------------
// 非流式响应聚合（SSE → 单个 JSON）
// ---------------------------------------------------------------------------

/// 将收集的 SSE 事件聚合为 OpenAI chat completion JSON 响应。
///
/// 用于非流式客户端：网关强制上游走流式，但客户端期望单个 JSON 响应。
/// 此函数将多个 SSE chunk 拼接为一个完整的 chat.completion 对象。
pub fn aggregate_sse_to_json(events: &[SseEvent]) -> Bytes {
    let mut id = String::new();
    let mut model = String::new();
    let mut content = String::new();
    let mut finish_reason: Option<String> = None;
    let mut usage: Option<serde_json::Value> = None;

    for event in events {
        let data = match &event.data {
            Some(d) if !d.is_empty() && d != "[DONE]" => d,
            _ => continue,
        };

        let json: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 提取元数据（取第一个有效值）
        if id.is_empty() {
            if let Some(v) = json.get("id").and_then(|v| v.as_str()) {
                id = v.to_string();
            }
        }
        if model.is_empty() {
            if let Some(v) = json.get("model").and_then(|v| v.as_str()) {
                model = v.to_string();
            }
        }

        // OpenAI chat chunk: choices[0].delta.content
        if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
            if let Some(first) = choices.first() {
                // 拼接增量内容
                if let Some(delta) = first.get("delta") {
                    if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                        content.push_str(c);
                    }
                }
                // 取最后出现的 finish_reason
                if let Some(fr) = first.get("finish_reason").and_then(|v| v.as_str()) {
                    if fr != "null" {
                        finish_reason = Some(fr.to_string());
                    }
                }
            }
        }

        // 提取 usage（取最后出现的）
        if let Some(u) = json.get("usage") {
            usage = Some(u.clone());
        }
    }

    // 构建 OpenAI chat completion 响应
    let mut result = serde_json::json!({
        "id": if id.is_empty() { "chatcmpl-aggregated" } else { &id },
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": if model.is_empty() { "unknown" } else { &model },
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content,
            },
            "finish_reason": finish_reason.as_deref().unwrap_or("stop"),
        }],
    });

    if let Some(u) = usage {
        result["usage"] = u;
    }

    Bytes::from(result.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_serialize() {
        let event = SseEvent {
            event: Some("message".to_string()),
            data: Some("hello world".to_string()),
            id: Some("123".to_string()),
            retry: Some(3000),
            comment: None,
        };
        let serialized = event.serialize();
        assert!(serialized.contains("event: message"));
        assert!(serialized.contains("data: hello world"));
        assert!(serialized.contains("id: 123"));
        assert!(serialized.contains("retry: 3000"));
    }

    #[test]
    fn test_sse_parser_basic() {
        let mut parser = SseParser::new();
        let input = "event: message\ndata: hello\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("message".to_string()));
        assert_eq!(events[0].data, Some("hello".to_string()));
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();
        let input = "data: line1\ndata: line2\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, Some("line1\nline2".to_string()));
    }

    #[test]
    fn test_sse_parser_multiple_events() {
        let mut parser = SseParser::new();
        let input = "data: first\n\ndata: second\n\n";
        let events = parser.feed(input.as_bytes());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, Some("first".to_string()));
        assert_eq!(events[1].data, Some("second".to_string()));
    }

    #[test]
    fn test_sse_parser_incremental() {
        let mut parser = SseParser::new();
        let events1 = parser.feed(b"data: hello");
        assert_eq!(events1.len(), 0);
        let events2 = parser.feed(b"\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, Some("hello".to_string()));
    }

    #[test]
    fn test_is_sse_response() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "text/event-stream".parse().unwrap(),
        );
        assert!(is_sse_response(&headers));

        let mut headers2 = HeaderMap::new();
        headers2.insert(
            axum::http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        assert!(!is_sse_response(&headers2));
    }

    #[test]
    fn test_stream_state_timeout() {
        let state = StreamState::new();
        assert!(!state.is_timed_out(Duration::from_secs(1)));
    }

    #[test]
    fn test_heartbeat_comment() {
        let hb = heartbeat_comment();
        assert_eq!(hb, Bytes::from(": keep-alive\n\n"));
    }

    #[test]
    fn test_stream_end_marker() {
        let end = stream_end_marker();
        assert_eq!(end, Bytes::from("data: [DONE]\n\n"));
    }

    #[test]
    fn test_sse_converter_openai_to_messages_incremental() {
        // openai 上游 → messages 下游：增量模式（小数据累积到 finish() 才统一输出）
        let mut converter = SseConverter::new("messages", "openai");
        let mut all = String::new();

        let ev1 = SseEvent {
            event: None,
            data: Some(r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        // 单条小事件不触发增量阈值，convert 返回空（累积）
        let out1 = converter.convert(&ev1).expect("convert 1");
        all.push_str(&String::from_utf8(out1.to_vec()).unwrap());

        let ev2 = SseEvent {
            event: None,
            data: Some(r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out2 = converter.convert(&ev2).expect("convert 2");
        all.push_str(&String::from_utf8(out2.to_vec()).unwrap());

        // finish 冲刷：prism 看到 [DONE] 后输出 message_stop
        let out3 = converter.finish().expect("finish");
        let s3 = String::from_utf8(out3.to_vec()).unwrap();
        all.push_str(&s3);

        assert!(all.contains("Hello"), "all: {all}");
        // finish 应产生收尾事件（如 message_stop）
        assert!(all.contains("message_stop"), "all: {all}");
        // 不应包含重复的 [DONE]（由 dispatch 统一发送）
        assert!(!all.contains("[DONE]"), "all: {all}");
    }

    #[test]
    fn test_sse_converter_messages_to_openai() {
        // messages 上游 → openai 下游：增量模式（小数据累积到 finish() 才统一输出）
        let mut converter = SseConverter::new("openai", "messages");
        let mut all = String::new();

        // message_start 作为上下文事件，转换后可能无可见输出
        let ev1 = SseEvent {
            event: Some("message_start".to_string()),
            data: Some(r#"{"type":"message_start","message":{"id":"m1","type":"message","role":"assistant","model":"claude-3","content":[],"usage":{"input_tokens":5,"output_tokens":1}}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let _ = converter.convert(&ev1).expect("convert 1");

        // content_block_start
        let ev_start = SseEvent {
            event: Some("content_block_start".to_string()),
            data: Some(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let _ = converter.convert(&ev_start).expect("convert start");

        let ev2 = SseEvent {
            event: Some("content_block_delta".to_string()),
            data: Some(r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out2 = converter.convert(&ev2).expect("convert 2");
        all.push_str(&String::from_utf8(out2.to_vec()).unwrap());

        let ev3 = SseEvent {
            event: Some("message_stop".to_string()),
            data: Some(r#"{"type":"message_stop"}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out3 = converter.convert(&ev3).expect("convert 3");
        all.push_str(&String::from_utf8(out3.to_vec()).unwrap());

        // 小数据在 finish() 统一输出，断言最终内容而非中间过程
        let finish_out = converter.finish().expect("finish");
        all.push_str(&String::from_utf8_lossy(&finish_out));

        assert!(all.contains("Hello"), "all: {all}");
        // [DONE] 由 dispatch 统一发送，转换器输出不应包含
        assert!(!all.contains("[DONE]"), "all: {all}");
    }

    #[test]
    fn test_sse_converter_responses_to_messages_finish() {
        // responses 上游 → messages 下游：finish 应直接生成 message_delta + message_stop
        let mut converter = SseConverter::new("messages", "responses");
        // 模拟一个内容事件
        let ev = SseEvent {
            event: Some("response.output_text.delta".to_string()),
            data: Some(r#"{"type":"response.output_text.delta","output_index":1,"content_index":0,"delta":"Hello"}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        // 单条小事件不触发增量阈值，convert 返回空（累积）
        let _ = converter.convert(&ev).expect("convert");

        // finish 应生成 message_delta + message_stop
        let finish_out = converter.finish().expect("finish");
        let finish_str = String::from_utf8_lossy(&finish_out);
        assert!(finish_str.contains("message_delta"), "缺少 message_delta: {finish_str}");
        assert!(finish_str.contains("message_stop"), "缺少 message_stop: {finish_str}");
        assert!(finish_str.contains("stop_reason"), "缺少 stop_reason: {finish_str}");
        // 不应包含 [DONE]（Messages 客户端不识别）
        assert!(!finish_str.contains("[DONE]"), "不应包含 [DONE]: {finish_str}");
    }

    /// 构造一条 openai chat completion SSE 事件（与上游 Agnes 格式一致）
    fn openai_chunk(delta: &str, finish_reason: Option<&str>) -> SseEvent {
        let delta_json = if let Some(fr) = finish_reason {
            format!(r#"{{"index":0,"delta":{{}},"finish_reason":"{fr}"}}"#)
        } else if delta.contains("reasoning_content") {
            format!(r#"{{"index":0,"delta":{delta},"finish_reason":null}}"#)
        } else {
            format!(r#"{{"index":0,"delta":{delta},"finish_reason":null}}"#)
        };
        SseEvent {
            event: None,
            data: Some(format!(
                r#"{{"id":"chunk-1","object":"chat.completion.chunk","created":1700000000,"model":"agnes-2.5-flash","choices":[{delta_json}]}}"#
            )),
            id: None,
            retry: None,
            comment: None,
        }
    }

    #[test]
    fn test_sse_converter_incremental_prefix_consistency() {
        // 增量转换的核心假设：多次 convert + finish 的拼接输出
        // 必须与一次性全量 convert_stream 输出完全一致（前缀 diff 确定性）。
        // 回归场景：真实日志中 finish() 时 flush_bytes=0，客户端在文本中途截断。
        let mut events: Vec<SseEvent> = vec![];
        // role 引导
        events.push(openai_chunk(r#"{"role":"assistant","content":""}"#, None));
        // 大量 reasoning_content（超过 INCREMENTAL_FLUSH_THRESHOLD=1024，触发多次增量 flush）
        for i in 0..20 {
            events.push(openai_chunk(&format!(r#"{{"reasoning_content":"思考片段{i}内容内容内容"}}"#), None));
        }
        // 文本内容
        for i in 0..10 {
            events.push(openai_chunk(&format!(r#"{{"content":"正文文本片段{i}内容"}}"#), None));
        }
        // 收尾
        events.push(openai_chunk(r#"{}"#, Some("stop")));
        events.push(SseEvent {
            event: None,
            data: Some(r#"{"id":"chunk-1","object":"chat.completion.chunk","created":1700000000,"model":"agnes-2.5-flash","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":30,"total_tokens":40}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        });
        events.push(SseEvent {
            event: None,
            data: Some("[DONE]".to_string()),
            id: None,
            retry: None,
            comment: None,
        });

        // 一次性全量转换（基准）
        let full_sse: String = events
            .iter()
            .map(|e| e.serialize())
            .collect::<Vec<_>>()
            .join("");
        let baseline_raw =
            prism_wasm::convert_stream("openai", &full_sse, "responses").expect("baseline");
        eprintln!("=== prism 原始输出 ===\n{baseline_raw}\n=============================");
        let baseline = filter_empty_events(&baseline_raw);
        assert!(baseline.contains("response.completed"), "基准应含 completed");

        // 增量转换：逐个事件 convert + finish
        let mut converter = SseConverter::new("responses", "openai");
        let mut incremental = String::new();
        for ev in &events {
            if let Ok(bytes) = converter.convert(ev) {
                incremental.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        let finish = converter.finish().expect("finish");
        incremental.push_str(&String::from_utf8_lossy(&finish));

        eprintln!("=== 增量拼接输出 ===\n{incremental}\n====================");
        eprintln!(
            "incremental len={} baseline len={}",
            incremental.len(),
            baseline.len()
        );
        // 打印首个差异位置及前后文
        let inc_bytes = incremental.as_bytes();
        let base_bytes = baseline.as_bytes();
        let diff_len = inc_bytes.len().min(base_bytes.len());
        let mut diff_at = None;
        for i in 0..diff_len {
            if inc_bytes[i] != base_bytes[i] {
                diff_at = Some(i);
                break;
            }
        }
        if let Some(i) = diff_at {
            let start = i.saturating_sub(30);
            eprintln!(
                "首个差异 @字节 {i}\n  增量: {:?}\n  基准: {:?}",
                String::from_utf8_lossy(&inc_bytes[start..(i + 30).min(inc_bytes.len())]),
                String::from_utf8_lossy(&base_bytes[start..(i + 30).min(base_bytes.len())]),
            );
        } else if inc_bytes.len() != base_bytes.len() {
            eprintln!(
                "前缀相同但长度不同：增量 {} vs 基准 {}（差异在尾部）",
                inc_bytes.len(),
                base_bytes.len()
            );
            eprintln!(
                "  增量尾部: {:?}",
                String::from_utf8_lossy(&inc_bytes[inc_bytes.len().saturating_sub(60)..])
            );
            eprintln!(
                "  基准尾部: {:?}",
                String::from_utf8_lossy(&base_bytes[base_bytes.len().saturating_sub(60)..])
            );
        }

        // 增量拼接必须与基准完全一致（前缀 diff 确定性）
        assert_eq!(
            incremental, baseline,
            "增量拼接与全量基准不一致：增量输出可能丢失/重复内容"
        );
        // 文本内容必须完整
        for i in 0..10 {
            let frag = format!("正文文本片段{i}内容");
            assert!(incremental.contains(&frag), "缺少文本片段 {frag}");
        }
        assert!(incremental.contains("response.completed"), "增量输出缺少 completed");
    }

    #[test]
    fn test_sse_converter_same_protocol_passthrough() {
        let mut converter = SseConverter::new("openai", "openai");
        let ev = SseEvent {
            event: None,
            data: Some(r#"{"choices":[{"delta":{"content":"x"}}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out = converter.convert(&ev).expect("passthrough");
        let s = String::from_utf8(out.to_vec()).unwrap();
        assert!(s.contains("data: "), "s: {s}");
        // finish 在未启用时返回空
        let fin = converter.finish().expect("finish");
        assert!(fin.is_empty());
    }

    // -----------------------------------------------------------------------
    // 规范化流式事件测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_stream_event_type_serialization() {
        // 测试 StreamEventType 序列化/反序列化
        let event_type = StreamEventType::ContentDelta;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, "\"content_delta\"");

        let deserialized: StreamEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, StreamEventType::ContentDelta);
    }

    #[test]
    fn test_stream_event_serialization() {
        // 测试 StreamEvent 序列化/反序列化
        let event = StreamEvent {
            id: Some("123".to_string()),
            model: Some("gpt-4".to_string()),
            event_type: StreamEventType::ContentDelta,
            role: Some("assistant".to_string()),
            delta: Some(ContentDelta {
                text: Some("Hello".to_string()),
                tool_calls: None,
            }),
            stop_reason: None,
            usage: None,
            sequence: Some(1),
            metadata: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"id\":\"123\""));
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"event_type\":\"content_delta\""));
        assert!(json.contains("\"text\":\"Hello\""));

        let deserialized: StreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, Some("123".to_string()));
        assert_eq!(deserialized.model, Some("gpt-4".to_string()));
        assert_eq!(deserialized.event_type, StreamEventType::ContentDelta);
    }

    #[test]
    fn test_usage_serialization() {
        // 测试 Usage 序列化/反序列化
        let usage = Usage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: Some(150),
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"output_tokens\":50"));
        assert!(json.contains("\"total_tokens\":150"));

        let deserialized: Usage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.input_tokens, Some(100));
        assert_eq!(deserialized.output_tokens, Some(50));
        assert_eq!(deserialized.total_tokens, Some(150));
    }

    #[test]
    fn test_sse_event_infer_type_openai() {
        // 测试 OpenAI 格式的事件类型推断
        let event = SseEvent {
            event: None,
            data: Some(r#"{"choices":[{"delta":{"content":"Hello"}}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ContentDelta);

        let event = SseEvent {
            event: None,
            data: Some(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ResponseStop);
    }

    #[test]
    fn test_sse_event_infer_type_anthropic() {
        // 测试 Anthropic 格式的事件类型推断
        let event = SseEvent {
            event: Some("message_start".to_string()),
            data: Some(r#"{"type":"message_start","message":{}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ResponseStart);

        let event = SseEvent {
            event: Some("content_block_delta".to_string()),
            data: Some(r#"{"type":"content_block_delta","delta":{"text":"Hello"}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ContentDelta);

        // content_block_stop 是块边界事件，不应归类为 ResponseStop
        let event = SseEvent {
            event: Some("content_block_stop".to_string()),
            data: Some(r#"{"type":"content_block_stop","index":0}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ContentDelta);

        let event = SseEvent {
            event: Some("message_stop".to_string()),
            data: Some(r#"{"type":"message_stop"}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ResponseStop);
    }

    #[test]
    fn test_sse_event_infer_type_done() {
        // 测试 [DONE] 事件类型推断
        let event = SseEvent {
            event: None,
            data: Some("[DONE]".to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::ResponseStop);
    }

    #[test]
    fn test_sse_event_infer_type_usage() {
        // 测试 Usage 事件类型推断
        let event = SseEvent {
            event: None,
            data: Some(r#"{"usage":{"prompt_tokens":100,"completion_tokens":50}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        assert_eq!(event.infer_type(), StreamEventType::Usage);
    }

    #[test]
    fn test_sse_event_to_stream_event_openai() {
        // 测试 OpenAI 格式的事件转换
        let event = SseEvent {
            event: None,
            data: Some(r#"{"id":"123","model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let stream_event = event.to_stream_event();
        assert_eq!(stream_event.id, Some("123".to_string()));
        assert_eq!(stream_event.model, Some("gpt-4".to_string()));
        assert_eq!(stream_event.event_type, StreamEventType::ContentDelta);
        assert!(stream_event.delta.is_some());
        assert_eq!(stream_event.delta.unwrap().text, Some("Hello".to_string()));
    }

    #[test]
    fn test_sse_event_to_stream_event_anthropic() {
        // 测试 Anthropic 格式的事件转换
        let event = SseEvent {
            event: Some("content_block_delta".to_string()),
            data: Some(r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let stream_event = event.to_stream_event();
        assert_eq!(stream_event.event_type, StreamEventType::ContentDelta);
        assert!(stream_event.delta.is_some());
        assert_eq!(stream_event.delta.unwrap().text, Some("Hello".to_string()));
    }

    #[test]
    fn test_sse_event_to_stream_event_usage() {
        // 测试 Usage 事件转换
        let event = SseEvent {
            event: None,
            data: Some(r#"{"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };

        let stream_event = event.to_stream_event();
        assert_eq!(stream_event.event_type, StreamEventType::Usage);
        assert!(stream_event.usage.is_some());
        let usage = stream_event.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.total_tokens, Some(150));
    }

    #[test]
    fn test_stream_event_extract_usage() {
        // 测试 StreamEvent 的 usage 提取方法
        let event = StreamEvent {
            id: None,
            model: None,
            event_type: StreamEventType::Usage,
            role: None,
            delta: None,
            stop_reason: None,
            usage: Some(Usage {
                input_tokens: Some(100),
                output_tokens: Some(50),
                total_tokens: Some(150),
            }),
            sequence: None,
            metadata: None,
        };

        assert!(event.extract_usage().is_some());
        assert_eq!(event.prompt_tokens(), Some(100));
        assert_eq!(event.completion_tokens(), Some(50));
    }

    #[test]
    fn test_stream_event_extract_usage_none() {
        // 测试 StreamEvent 没有 usage 时的行为
        let event = StreamEvent {
            id: None,
            model: None,
            event_type: StreamEventType::ContentDelta,
            role: None,
            delta: None,
            stop_reason: None,
            usage: None,
            sequence: None,
            metadata: None,
        };

        assert!(event.extract_usage().is_none());
        assert!(event.prompt_tokens().is_none());
        assert!(event.completion_tokens().is_none());
    }
}
