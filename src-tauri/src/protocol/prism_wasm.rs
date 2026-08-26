//! Prism WASM 协议转换引擎
//!
//! 通过 wasmtime 加载 prism.wasm（MoonBit classic `wasm` 目标编译产物），
//! 实现 Prism 字符串 ABI，对外提供 String → String 的纯函数协议转换。
//!
//! ABI 约定（与 prism 仓库 wrappers/ts/src/wasm.ts 一致）：
//! - 每个 String 参数以 i32 线性内存地址传入
//! - 地址 `ptr - 4` 处为 u32 长度（UTF-16 码元数）
//! - 字符串本体为 UTF-16LE，从 `ptr` 开始
//! - i32 返回值是同样的地址布局
//! - 首次调用前必须先执行 `_start` 初始化运行时
//!
//! 本模块为进程级单例（LazyLock + Mutex），转换是纯函数、无 IO、无状态，
//! 一次调用只持锁一次，适合网关主链路高频调用。

use std::sync::{LazyLock, Mutex, MutexGuard};

use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

/// 嵌入的 prism.wasm 字节（release 构建，310KB）
const PRISM_WASM: &[u8] = include_bytes!("../../prism.wasm");

/// Host 侧字符串参数暂存区：位于 MoonBit GC 堆（~0x1000）之下，固定地址写入。
const SCRATCH_START: u32 = 0x0400;
const SCRATCH_STRIDE: u32 = 512;

/// Prism WASM 实例（内部持有 wasmtime 运行时，非 Sync，由外层 Mutex 保护）
pub struct PrismWasm {
    store: Store<()>,
    instance: Instance,
    memory: Memory,
}

impl PrismWasm {
    /// 加载并初始化 prism.wasm
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| format!("加载 prism.wasm 模块失败: {e}"))?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| format!("实例化 prism.wasm 失败: {e}"))?;

        // 运行运行时初始化器（与 TS wrapper 相同）
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| format!("获取 _start 失败: {e}"))?;
        start
            .call(&mut store, ())
            .map_err(|e| format!("运行 _start 失败: {e}"))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| "prism.wasm 缺少 memory 导出".to_string())?;

        Ok(Self {
            store,
            instance,
            memory,
        })
    }

    /// 将 Rust 字符串按 UTF-16 ABI 写入线性内存 `ptr` 处。
    ///
    /// 如果 `ptr` 处的暂存区（SCRATCH_STRIDE 字节）放不下，则动态扩展线性内存
    /// 并将字符串写到当前线性内存末尾，确保远离 MoonBit GC 堆（GC 堆从 ~0x1000
    /// 向上增长，不会追到线性内存末尾）。
    fn write_string(&mut self, ptr: u32, s: &str) -> u32 {
        let units: Vec<u16> = s.encode_utf16().collect();
        let len = units.len() as u32;
        let needed = 4 + len * 2; // 长度头 + UTF-16LE 载荷

        // 判断是否需要使用大字符串策略
        let write_ptr = if needed <= SCRATCH_STRIDE {
            ptr
        } else {
            // 大字符串：扩展线性内存，写到末尾。
            // 取当前内存大小，扩展 needed 字节，写入新区域末尾。
            let current_pages = self.memory.size(&self.store);
            let current_bytes = (current_pages as usize) * 65536;
            let grow_pages = ((needed as usize + 65535) / 65536) as u64;
            if grow_pages > 0 {
                self.memory
                    .grow(&mut self.store, grow_pages)
                    .map_err(|e| format!("扩展线性内存失败: {e}"))
                    .unwrap();
            }
            // 写到新扩展区域的起始处（+4 留出长度头空间，避免头部落入旧内存）
            (current_bytes + 4) as u32
        };

        let data = self.memory.data_mut(&mut self.store);

        // 长度头：ptr - 4（u32 LE，UTF-16 码元数）
        let header = (write_ptr as usize) - 4;
        data[header..header + 4].copy_from_slice(&len.to_le_bytes());

        // UTF-16LE 载荷
        let base = write_ptr as usize;
        for (i, &u) in units.iter().enumerate() {
            let off = base + i * 2;
            data[off] = (u & 0xff) as u8;
            data[off + 1] = (u >> 8) as u8;
        }

        write_ptr
    }

    /// 从线性内存 `ptr` 处读取 UTF-16 ABI 字符串
    fn read_string(&self, ptr: i32) -> Result<String, String> {
        let data = self.memory.data(&self.store);
        let ptr = ptr as usize;
        if ptr < 4 {
            return Err("无效的返回指针".to_string());
        }
        let len = u32::from_le_bytes(
            data[ptr - 4..ptr]
                .try_into()
                .map_err(|_| "读取长度头失败".to_string())?,
        ) as usize;
        let mut units = Vec::with_capacity(len);
        for i in 0..len {
            let off = ptr + i * 2;
            if off + 1 >= data.len() {
                return Err("返回字符串越界".to_string());
            }
            units.push(u16::from_le_bytes([data[off], data[off + 1]]));
        }
        String::from_utf16(&units).map_err(|e| format!("UTF-16 解码失败: {e}"))
    }

    /// 调用三参导出函数（source, json/sse, target）→ String
    fn call_3(
        &mut self,
        name: &str,
        a: &str,
        b: &str,
        c: &str,
    ) -> Result<String, String> {
        let f: TypedFunc<(i32, i32, i32), i32> = self
            .instance
            .get_typed_func(&mut self.store, name)
            .map_err(|e| format!("获取导出函数 {name} 失败: {e}"))?;

        // write_string 返回实际写入地址（大字符串会写到线性内存末尾）
        let pa = self.write_string(SCRATCH_START, a);
        let pb = self.write_string(SCRATCH_START + SCRATCH_STRIDE, b);
        let pc = self.write_string(SCRATCH_START + 2 * SCRATCH_STRIDE, c);

        let ret = f
            .call(&mut self.store, (pa as i32, pb as i32, pc as i32))
            .map_err(|e| format!("调用 {name} 失败: {e}"))?;
        self.read_string(ret)
    }

    /// 调用无参导出函数 → String
    fn call_0(&mut self, name: &str) -> Result<String, String> {
        let f: TypedFunc<(), i32> = self
            .instance
            .get_typed_func(&mut self.store, name)
            .map_err(|e| format!("获取导出函数 {name} 失败: {e}"))?;
        let ret = f
            .call(&mut self.store, ())
            .map_err(|e| format!("调用 {name} 失败: {e}"))?;
        self.read_string(ret)
    }
}

/// 进程级单例：LazyLock 懒加载，Mutex 保护非 Sync 的 wasmtime Store。
/// 初始化失败时缓存 Err，后续调用直接返回该错误。
static PRISM: LazyLock<Result<Mutex<PrismWasm>, String>> = LazyLock::new(|| {
    PrismWasm::new(PRISM_WASM).map(Mutex::new)
});

/// 获取 prism 实例锁
fn prism() -> Result<MutexGuard<'static, PrismWasm>, String> {
    match PRISM.as_ref() {
        Ok(m) => m.lock().map_err(|_| "prism 锁中毒".to_string()),
        Err(e) => Err(e.clone()),
    }
}

/// silk 协议名 → prism provider 名映射
///
/// silk 使用下划线协议名（openai_chat/claude_messages/openai_response），
/// prism 使用其 provider 主名（openai-chat/anthropic/openai）。
pub fn map_provider(protocol: &str) -> Option<&'static str> {
    match protocol {
        "openai_chat" => Some("openai-chat"),
        "claude_messages" => Some("anthropic"),
        "openai_response" => Some("openai"),
        _ => None,
    }
}

/// 解析 wasm 返回的信封：`{"value":..., "diagnostics":[]}` 或 `{"error":...}`
fn extract_value(envelope: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(envelope).map_err(|e| format!("解析信封失败: {e}"))?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    v.get("value")
        .and_then(|val| val.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "信封缺少 value".to_string())
}

/// 请求转换：source 协议 JSON → target 协议 JSON
pub fn convert_request(source: &str, json: &str, target: &str) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_3("wasm_convert_req", src, json, dst)?;
    extract_value(&envelope)
}

/// 响应转换：source 协议 JSON → target 协议 JSON
pub fn convert_response(source: &str, json: &str, target: &str) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_3("wasm_convert_resp", src, json, dst)?;
    extract_value(&envelope)
}

/// 流式转换：source 协议 SSE → target 协议 SSE
pub fn convert_stream(source: &str, sse: &str, target: &str) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_3("wasm_convert_stream", src, sse, dst)?;
    extract_value(&envelope)
}

/// 查询全部 provider 主名
pub fn list_providers() -> Result<Vec<String>, String> {
    let mut p = prism()?;
    let raw = p.call_0("wasm_list_providers")?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析 provider 列表失败: {e}"))?;
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .ok_or_else(|| "provider 列表格式错误".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_provider() {
        assert_eq!(map_provider("openai_chat"), Some("openai-chat"));
        assert_eq!(map_provider("claude_messages"), Some("anthropic"));
        assert_eq!(map_provider("openai_response"), Some("openai"));
        assert_eq!(map_provider("unknown"), None);
    }

    #[test]
    fn test_list_providers() {
        let providers = list_providers().expect("list providers");
        assert!(providers.contains(&"openai-chat".to_string()));
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"openai".to_string()));
    }

    #[test]
    fn test_convert_request_openai_chat_to_anthropic() {
        let req = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        let out = convert_request("openai_chat", &req.to_string(), "claude_messages")
            .expect("convert request");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["model"], "gpt-4");
        assert_eq!(v["messages"][0]["role"], "user");
        // content 被转为 Anthropic 块数组
        assert_eq!(v["messages"][0]["content"][0]["type"], "text");
        assert_eq!(v["messages"][0]["content"][0]["text"], "Hello");
    }

    #[test]
    fn test_convert_request_anthropic_to_openai_chat() {
        // 使用标准 Anthropic 块数组形状（prism 解码器要求）
        let req = serde_json::json!({
            "model": "claude-3-opus-20240229",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": [{"type": "text", "text": "Hello"}]}],
            "stream": true
        });
        let out = convert_request("claude_messages", &req.to_string(), "openai_chat")
            .expect("convert request");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_convert_response_openai_chat_to_anthropic() {
        let resp = serde_json::json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        });
        let out = convert_response("openai_chat", &resp.to_string(), "claude_messages")
            .expect("convert response");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["type"], "message");
        assert_eq!(v["content"][0]["text"], "Hi!");
    }

    #[test]
    fn test_convert_stream_openai_chat_to_anthropic() {
        let sse = [
            "data: {\"id\":\"1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}",
            "",
            "data: {\"id\":\"1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}",
            "",
            "data: [DONE]",
            "",
        ]
        .join("\n");
        let out = convert_stream("openai_chat", &sse, "claude_messages").expect("convert stream");
        assert!(out.contains("content_block_delta"));
        assert!(out.contains("Hello"));
    }

    #[test]
    fn test_convert_unsupported_protocol() {
        let req = "{}";
        assert!(convert_request("unknown_proto", req, "openai_chat").is_err());
        assert!(convert_request("openai_chat", req, "unknown_proto").is_err());
    }
}
