use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::Stream;

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
    pub fn is_end(&self) -> bool {
        self.data.as_deref() == Some("[DONE]")
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

/// 流式 SSE 事件协议转换器（基于 prism.wasm 整段转换 + 增量差分）
///
/// prism 的 `wasm_convert_stream` 是整段转换（SSE 文本 → SSE 文本），
/// 本转换器通过累积事件文本 + 全量重转换 + 输出差分，实现逐事件增量转发：
/// - 每个事件序列化后追加到 `buffer`
/// - 每次调用对完整 buffer 调用 `prism_wasm::convert_stream`
/// - 只返回本次新增的输出（delta），保证流式低延迟
/// - 过滤无 `data:` 的空事件（prism 对 content_block_start 等会输出空占位）
///
/// 已实测两个方向（openai-chat ↔ anthropic）转换输出单调（追加只增不改），
/// 差分可靠；若出现非单调（异常），整段重发避免丢内容。
pub struct SseConverter {
    /// 是否需要转换（inbound != outbound 且协议可映射）
    enabled: bool,
    /// 源协议（上游，silk 协议名）
    source: String,
    /// 目标协议（下游，silk 协议名）
    target: String,
    /// 累积的上游 SSE 文本缓冲
    buffer: String,
    /// 上次完整转换输出（用于增量差分）
    last_output: String,
}

impl SseConverter {
    pub fn new(inbound: &str, outbound: &str) -> Self {
        let enabled = inbound != outbound
            && !inbound.is_empty()
            && !outbound.is_empty()
            && prism_wasm::map_provider(inbound).is_some()
            && prism_wasm::map_provider(outbound).is_some();
        Self {
            enabled,
            source: outbound.to_string(),
            target: inbound.to_string(),
            buffer: String::new(),
            last_output: String::new(),
        }
    }

    pub fn convert(&mut self, event: &SseEvent) -> Result<Bytes, String> {
        if !self.enabled {
            return Ok(Bytes::from(event.serialize()));
        }
        // 追加当前事件到缓冲
        self.buffer.push_str(&event.serialize());
        // 整段转换（prism 是整段 SSE 文本 → SSE 文本）
        let full = prism_wasm::convert_stream(&self.source, &self.buffer, &self.target)?;
        // 过滤空事件并剥离末尾 [DONE]（由 dispatch 统一发送流结束标记）
        let filtered = filter_empty_events(&full);
        // 增量差分：只返回新增部分（prism 转换输出单调，追加只增不改）
        let delta = if filtered.starts_with(&self.last_output) {
            filtered[self.last_output.len()..].to_string()
        } else {
            // 非单调（异常）：整段重发，避免丢内容
            filtered.clone()
        };
        self.last_output = filtered;
        Ok(Bytes::from(delta))
    }

    /// 流结束冲刷：追加 `[DONE]` 使 prism 输出收尾事件（如 anthropic message_stop）。
    ///
    /// 上游 openai-chat 的 `[DONE]` 由 dispatch 拦截并发送流结束标记，不会
    /// 经过 convert()；但 prism 需要看到 `[DONE]` 才会输出 message_stop 等
    /// 收尾事件，故在流结束时显式调用本方法冲刷最终增量。
    pub fn finish(&mut self) -> Result<Bytes, String> {
        if !self.enabled {
            return Ok(Bytes::new());
        }
        self.buffer.push_str("data: [DONE]\n\n");
        let full = prism_wasm::convert_stream(&self.source, &self.buffer, &self.target)?;
        let filtered = filter_empty_events(&full);
        let delta = if filtered.starts_with(&self.last_output) {
            filtered[self.last_output.len()..].to_string()
        } else {
            filtered.clone()
        };
        self.last_output = filtered;
        Ok(Bytes::from(delta))
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
    fn test_sse_converter_openai_chat_to_anthropic_incremental() {
        // openai-chat 上游 → anthropic 下游：逐事件增量差分，最终含 message_stop
        let mut converter = SseConverter::new("claude_messages", "openai_chat");
        let mut all = String::new();

        let ev1 = SseEvent {
            event: None,
            data: Some(r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out1 = converter.convert(&ev1).expect("convert 1");
        let s1 = String::from_utf8(out1.to_vec()).unwrap();
        assert!(s1.contains("content_block_delta"), "s1: {s1}");
        assert!(s1.contains("Hello"), "s1: {s1}");
        all.push_str(&s1);

        let ev2 = SseEvent {
            event: None,
            data: Some(r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out2 = converter.convert(&ev2).expect("convert 2");
        let s2 = String::from_utf8(out2.to_vec()).unwrap();
        all.push_str(&s2);

        // finish 冲刷：prism 看到 [DONE] 后输出 message_stop
        let out3 = converter.finish().expect("finish");
        let s3 = String::from_utf8(out3.to_vec()).unwrap();
        all.push_str(&s3);

        assert!(all.contains("content_block_delta"), "all: {all}");
        assert!(all.contains("Hello"), "all: {all}");
        assert!(all.contains("message_stop"), "all: {all}");
        // 不应包含重复的 [DONE]（由 dispatch 统一发送）
        assert!(!all.contains("[DONE]"), "all: {all}");
    }

    #[test]
    fn test_sse_converter_anthropic_to_openai_chat() {
        // anthropic 上游 → openai-chat 下游：message_start 上下文 + 增量差分
        let mut converter = SseConverter::new("openai_chat", "claude_messages");
        let mut all = String::new();

        let ev1 = SseEvent {
            event: Some("message_start".to_string()),
            data: Some(r#"{"type":"message_start","message":{"id":"m1","type":"message","role":"assistant","model":"claude-3","content":[],"usage":{"input_tokens":5,"output_tokens":1}}}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out1 = converter.convert(&ev1).expect("convert 1");
        all.push_str(&String::from_utf8(out1.to_vec()).unwrap());

        // content_block_start 是 delta 的前置上下文（prism 需要它）
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
        let s2 = String::from_utf8(out2.to_vec()).unwrap();
        all.push_str(&s2);
        assert!(s2.contains("Hello"), "s2: {s2}");

        let ev3 = SseEvent {
            event: Some("message_stop".to_string()),
            data: Some(r#"{"type":"message_stop"}"#.to_string()),
            id: None,
            retry: None,
            comment: None,
        };
        let out3 = converter.convert(&ev3).expect("convert 3");
        all.push_str(&String::from_utf8(out3.to_vec()).unwrap());

        assert!(all.contains("Hello"), "all: {all}");
        // [DONE] 由 dispatch 统一发送，转换器输出不应包含
        assert!(!all.contains("[DONE]"), "all: {all}");
    }

    #[test]
    fn test_sse_converter_same_protocol_passthrough() {
        let mut converter = SseConverter::new("openai_chat", "openai_chat");
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
}
