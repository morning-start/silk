use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::Stream;
use linguafranca::anthropic::convert::stream::{
    AnthropicMessagesToOpenResponsesStream, OpenResponsesToAnthropicMessagesStream,
};
use linguafranca::anthropic::stream::AnthropicStreamEvent;
use linguafranca::chat_completions_openai::convert::stream::{
    ChatCompletionsToOpenResponsesStream, OpenResponsesToChatCompletionsStream,
};
use linguafranca::chat_completions_openai::stream::ChatCompletionsStreamChunk;
use linguafranca::open_responses::stream::OpenResponsesStreamEvent;
use linguafranca::stream::StreamTransform;

use crate::gateway::error::GatewayError;

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

/// 流式 SSE 事件协议转换器
///
/// 转换架构（hub = OpenResponsesStreamEvent）：
///   openai_chat    ──ChatToHub──→  hub  ──HubToChat──→  openai_chat
///   claude_messages ──ClaudeToHub→  hub  ──HubToClaude→ claude_messages
///   openai_response                  hub                openai_response
///
/// - 出站为 openai_response：数据已是 hub，跳过 outbound→hub
/// - 入站为 openai_response：hub 已是最终格式，跳过 hub→inbound
pub struct SseConverter {
    /// outbound → hub 转换器（None 表示出站已是 hub 格式）
    pub outbound_to_hub: Option<OutboundToHubConverter>,
    /// hub → inbound 转换器（None 表示入站已是 hub 格式）
    pub hub_to_inbound: Option<HubToInboundConverter>,
}

pub enum OutboundToHubConverter {
    ChatCompletionsToOpenResponses(ChatCompletionsToOpenResponsesStream),
    AnthropicToOpenResponses(AnthropicMessagesToOpenResponsesStream),
}

pub enum HubToInboundConverter {
    OpenResponsesToChatCompletions(OpenResponsesToChatCompletionsStream),
    OpenResponsesToAnthropic(OpenResponsesToAnthropicMessagesStream),
}

impl SseConverter {
    pub fn new(inbound: &str, outbound: &str) -> Self {
        if inbound == outbound || inbound.is_empty() || outbound.is_empty() {
            return Self { outbound_to_hub: None, hub_to_inbound: None };
        }
        Self {
            outbound_to_hub: match outbound {
                "openai_chat" => Some(OutboundToHubConverter::ChatCompletionsToOpenResponses(
                    ChatCompletionsToOpenResponsesStream::new(),
                )),
                "claude_messages" => Some(OutboundToHubConverter::AnthropicToOpenResponses(
                    AnthropicMessagesToOpenResponsesStream::new(),
                )),
                // openai_response: 已是 hub 格式，无需转换
                _ => None,
            },
            hub_to_inbound: match inbound {
                "openai_chat" => Some(HubToInboundConverter::OpenResponsesToChatCompletions(
                    OpenResponsesToChatCompletionsStream::new(),
                )),
                "claude_messages" => Some(HubToInboundConverter::OpenResponsesToAnthropic(
                    OpenResponsesToAnthropicMessagesStream::new(),
                )),
                // openai_response: hub 已是最终格式，无需转换
                _ => None,
            },
        }
    }

    pub fn convert(&mut self, event: &SseEvent) -> Result<Bytes, String> {
        // 无需协议转换时直接透传原始事件（inbound == outbound）
        if self.outbound_to_hub.is_none() && self.hub_to_inbound.is_none() {
            return Ok(Bytes::from(event.serialize()));
        }

        let data = match &event.data {
            Some(d) => d,
            None => return Ok(Bytes::from(event.serialize())),
        };
        let json: serde_json::Value =
            serde_json::from_str(data).map_err(|e| format!("解析 SSE data JSON 失败: {e}"))?;

        // Step 1: 解析出站事件 → hub 事件
        let hub_events = if self.outbound_to_hub.is_some() {
            self.outbound_to_hub_events(&json)?
        } else {
            // 出站已是 hub 格式（openai_response），直接反序列化
            let ev: OpenResponsesStreamEvent = serde_json::from_value(json)
                .map_err(|e| format!("解析 OpenResponses SSE event 失败: {e}"))?;
            vec![ev]
        };

        // Step 2: hub 事件 → 入站格式
        if self.hub_to_inbound.is_some() {
            let mut output = Vec::new();
            for hub_event in &hub_events {
                let bytes = self.hub_to_inbound_events(hub_event)?;
                output.extend(bytes);
            }
            Ok(Bytes::from(output))
        } else {
            // 入站已是 hub 格式（openai_response），直接序列化为 SSE
            serialize_open_responses_events(&hub_events)
        }
    }

    fn outbound_to_hub_events(
        &mut self,
        json: &serde_json::Value,
    ) -> Result<Vec<OpenResponsesStreamEvent>, String> {
        match self.outbound_to_hub.as_mut().unwrap() {
            OutboundToHubConverter::ChatCompletionsToOpenResponses(c) => {
                let chunk: ChatCompletionsStreamChunk = serde_json::from_value(json.clone())
                    .map_err(|e| format!("解析 OpenAI Chat SSE chunk 失败: {e}"))?;
                c.transform(chunk)
                    .map_err(|e| format!("OpenAI Chat → OpenResponses 转换失败: {e}"))
            }
            OutboundToHubConverter::AnthropicToOpenResponses(c) => {
                let event: AnthropicStreamEvent = serde_json::from_value(json.clone())
                    .map_err(|e| format!("解析 Anthropic SSE event 失败: {e}"))?;
                c.transform(event)
                    .map_err(|e| format!("Anthropic → OpenResponses 转换失败: {e}"))
            }
        }
    }

    fn hub_to_inbound_events(
        &mut self,
        hub_event: &OpenResponsesStreamEvent,
    ) -> Result<Vec<u8>, String> {
        match self.hub_to_inbound.as_mut().unwrap() {
            HubToInboundConverter::OpenResponsesToChatCompletions(c) => {
                let out: Vec<ChatCompletionsStreamChunk> = c
                    .transform(hub_event.clone())
                    .map_err(|e| format!("OpenResponses → OpenAI Chat 转换失败: {e}"))?;
                let mut bytes = Vec::new();
                for chunk in out {
                    let s = serde_json::to_string(&chunk)
                        .map_err(|e| format!("序列化 OpenAI Chat chunk 失败: {e}"))?;
                    bytes.extend_from_slice(b"data: ");
                    bytes.extend_from_slice(s.as_bytes());
                    bytes.extend_from_slice(b"\n\n");
                }
                Ok(bytes)
            }
            HubToInboundConverter::OpenResponsesToAnthropic(c) => {
                let out: Vec<AnthropicStreamEvent> = c
                    .transform(hub_event.clone())
                    .map_err(|e| format!("OpenResponses → Anthropic 转换失败: {e}"))?;
                let mut bytes = Vec::new();
                for event in out {
                    let json_str = serde_json::to_string(&event)
                        .map_err(|e| format!("序列化 Anthropic event 失败: {e}"))?;
                    if let Some(et) = extract_anthropic_event_type(&event) {
                        bytes.extend_from_slice(b"event: ");
                        bytes.extend_from_slice(et.as_bytes());
                        bytes.extend_from_slice(b"\n");
                    }
                    bytes.extend_from_slice(b"data: ");
                    bytes.extend_from_slice(json_str.as_bytes());
                    bytes.extend_from_slice(b"\n\n");
                }
                Ok(bytes)
            }
        }
    }
}

/// 提取 AnthropicStreamEvent 的 type 字段值（用于 SSE event: 行）
pub fn extract_anthropic_event_type(event: &AnthropicStreamEvent) -> Option<String> {
    let json = serde_json::to_value(event).ok()?;
    json.get("type").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 将 OpenResponsesStreamEvent 列表序列化为 SSE 文本
pub fn serialize_open_responses_events(events: &[OpenResponsesStreamEvent]) -> Result<Bytes, String> {
    let mut bytes = Vec::new();
    for event in events {
        let json_val = serde_json::to_value(event)
            .map_err(|e| format!("序列化 OpenResponses event 失败: {e}"))?;
        let et = json_val
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let json_str = serde_json::to_string(&json_val)
            .map_err(|e| format!("JSON to string 失败: {e}"))?;
        if let Some(et) = et {
            bytes.extend_from_slice(b"event: ");
            bytes.extend_from_slice(et.as_bytes());
            bytes.extend_from_slice(b"\n");
        }
        bytes.extend_from_slice(b"data: ");
        bytes.extend_from_slice(json_str.as_bytes());
        bytes.extend_from_slice(b"\n\n");
    }
    Ok(Bytes::from(bytes))
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
}
