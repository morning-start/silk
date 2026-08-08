//! SSE流式转换模块
//!
//! 包含SSE事件解析、流式转换器抽象和具体实现。

pub mod converter;
pub mod openai_chat;
pub mod openai_response;
pub mod claude_messages;

pub use converter::{
    SseEvent, SseParser, StreamConversionError, StreamConverter, StreamConverterRegistry,
};
