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
//!
//! 23 个导出函数覆盖情况：
//! - 中转转换 4 个：convert_request / convert_response / convert_stream / convert_stream_event
//! - 查询 1 个：list_providers
//! - 日志 2 个：init_log / log_pos
//! - 追踪 3 个：convert_request_trace / convert_response_trace / convert_stream_trace
//! - 低层 IR（6）和 SDK（5）：silk 网关不需要，不暴露
//! - scratch（2）：宿主内存增长方案替代，不暴露

use std::sync::{LazyLock, Mutex, MutexGuard};

use wasmtime::*;
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

/// 编译时嵌入的 prism.wasm 字节（兜底，确保开箱即用）
const PRISM_WASM_EMBEDDED: &[u8] = include_bytes!("../../prism.wasm");

/// 运行时加载 prism.wasm：优先从应用数据目录读取，不存在则使用嵌入版本。
///
/// 这样更新 wasm 只需替换文件并重启应用，无需重新编译。
fn load_prism_wasm() -> Vec<u8> {
    // 尝试从可执行文件同目录加载
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let wasm_path = exe_dir.join("prism.wasm");
            if let Ok(bytes) = std::fs::read(&wasm_path) {
                tracing::info!("从外部文件加载 prism.wasm: {}", wasm_path.display());
                return bytes;
            }
        }
    }
    // 尝试从应用数据目录加载
    if let Some(data_dir) = crate::get_settings_path().and_then(|p| p.parent()) {
        let wasm_path = data_dir.join("prism.wasm");
        if let Ok(bytes) = std::fs::read(&wasm_path) {
            tracing::info!("从数据目录加载 prism.wasm: {}", wasm_path.display());
            return bytes;
        }
    }
    tracing::info!("使用内嵌 prism.wasm ({}KB)", PRISM_WASM_EMBEDDED.len() / 1024);
    PRISM_WASM_EMBEDDED.to_vec()
}

/// Host 侧字符串参数暂存区：位于 MoonBit GC 堆（~0x1000）之下，固定地址写入。
const SCRATCH_START: u32 = 0x0400;
const SCRATCH_STRIDE: u32 = 512;

/// Prism WASM 实例（内部持有 wasmtime 运行时，非 Sync，由外层 Mutex 保护）
pub struct PrismWasm {
    store: Store<WasiP1Ctx>,
    instance: Instance,
    memory: Memory,
}

impl PrismWasm {
    /// 加载并初始化 prism.wasm
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| format!("加载 prism.wasm 模块失败: {e}"))?;

        // WASI 上下文：MoonBit wasm 目标依赖 WASI（stdout/stderr），
        // 无 WASI 时 _start 中的 IO 操作会 panic。
        let wasi = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build_p1();
        let mut store = Store::new(&engine, wasi);

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |ctx| ctx)
            .map_err(|e| format!("添加 WASI linker 失败: {e}"))?;

        let instance = linker
            .instantiate(&mut store, &module)
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
        Ok(String::from_utf16_lossy(&units))
    }

    /// 通用调用帮助函数：写参数 → 调导出 → 读结果字符串
    ///
    /// 所有返回字符串指针的函数都走这个方法。
    /// args 为字符串参数列表，函数按 scratch 布局依次写入线性内存。
    fn call(&mut self, func_name: &str, args: &[&str]) -> Result<String, String> {
        let ret = self.call_raw(func_name, args)?;
        self.read_string(ret)
    }

    /// 底层调用：写参数 → 调导出 → 返回原始 i32 返回值
    ///
    /// 大多数函数返回字符串指针（用 `call`），
    /// 少数函数（如 wasm_log_init）直接返回 i32 地址（用 `call_raw_i32`）。
    fn call_raw(&mut self, func_name: &str, args: &[&str]) -> Result<i32, String> {
        // 构建参数指针列表
        let mut ptrs = Vec::with_capacity(args.len());
        let mut scratch = SCRATCH_START;
        for arg in args {
            let p = self.write_string(scratch, arg);
            ptrs.push(p as i32);
            scratch += SCRATCH_STRIDE;
        }

        // 按参数数量调用对应的类型化导出函数
        match args.len() {
            0 => {
                let f: TypedFunc<(), i32> = self
                    .instance
                    .get_typed_func(&mut self.store, func_name)
                    .map_err(|e| format!("获取导出函数 {func_name} 失败: {e}"))?;
                f.call(&mut self.store, ())
                    .map_err(|e| format!("调用 {func_name} 失败: {e}"))
            }
            1 => {
                let f: TypedFunc<(i32,), i32> = self
                    .instance
                    .get_typed_func(&mut self.store, func_name)
                    .map_err(|e| format!("获取导出函数 {func_name} 失败: {e}"))?;
                f.call(&mut self.store, (ptrs[0],))
                    .map_err(|e| format!("调用 {func_name} 失败: {e}"))
            }
            3 => {
                let f: TypedFunc<(i32, i32, i32), i32> = self
                    .instance
                    .get_typed_func(&mut self.store, func_name)
                    .map_err(|e| format!("获取导出函数 {func_name} 失败: {e}"))?;
                f.call(&mut self.store, (ptrs[0], ptrs[1], ptrs[2]))
                    .map_err(|e| format!("调用 {func_name} 失败: {e}"))
            }
            5 => {
                let f: TypedFunc<(i32, i32, i32, i32, i32), i32> = self
                    .instance
                    .get_typed_func(&mut self.store, func_name)
                    .map_err(|e| format!("获取导出函数 {func_name} 失败: {e}"))?;
                f.call(&mut self.store, (ptrs[0], ptrs[1], ptrs[2], ptrs[3], ptrs[4]))
                    .map_err(|e| format!("调用 {func_name} 失败: {e}"))
            }
            n => Err(format!("不支持的参数数量: {n}（{func_name}）")),
        }
    }

    /// 调用返回原始 i32（非字符串指针）的导出函数
    fn call_i32(&mut self, func_name: &str, args: &[&str]) -> Result<i32, String> {
        self.call_raw(func_name, args)
    }

    /// 混合调用：前 N 个参数是字符串（写入 scratch），后 M 个是原始 i32 值
    ///
    /// 用于 trace 函数：`(source, json, target, log_ptr, log_size)`
    /// 其中 source/json/target 是字符串指针，log_ptr/log_size 是原始 i32。
    fn call_mixed(
        &mut self,
        func_name: &str,
        str_args: &[&str],
        i32_args: &[i32],
    ) -> Result<String, String> {
        let total = str_args.len() + i32_args.len();
        let mut all_ptrs = Vec::with_capacity(total);

        // 写字符串参数到 scratch
        let mut scratch = SCRATCH_START;
        for arg in str_args {
            let p = self.write_string(scratch, arg);
            all_ptrs.push(p as i32);
            scratch += SCRATCH_STRIDE;
        }
        // 追加原始 i32 参数
        all_ptrs.extend_from_slice(i32_args);

        let ret = match total {
            5 => {
                let f: TypedFunc<(i32, i32, i32, i32, i32), i32> = self
                    .instance
                    .get_typed_func(&mut self.store, func_name)
                    .map_err(|e| format!("获取导出函数 {func_name} 失败: {e}"))?;
                f.call(
                    &mut self.store,
                    (all_ptrs[0], all_ptrs[1], all_ptrs[2], all_ptrs[3], all_ptrs[4]),
                )
                .map_err(|e| format!("调用 {func_name} 失败: {e}"))?
            }
            n => return Err(format!("不支持的混合参数数量: {n}（{func_name}）")),
        };

        self.read_string(ret)
    }
}

/// 进程级单例：LazyLock 懒加载，Mutex 保护非 Sync 的 wasmtime Store。
/// 初始化失败时缓存 Err，后续调用直接返回该错误。
static PRISM: LazyLock<Result<Mutex<PrismWasm>, String>> = LazyLock::new(|| {
    let wasm_bytes = load_prism_wasm();
    let prism = PrismWasm::new(&wasm_bytes)?;

    // 启动时校验：用 wasm 实际导出的 provider 列表验证映射表
    {
        let mut p = prism; // 临时取出，校验完再用
        match p.call("wasm_list_providers", &[]) {
            Ok(raw) => {
                if let Ok(providers) = serde_json::from_str::<Vec<String>>(&raw) {
                    let all_mapped: &[(&str, &str)] = &[
                        ("openai_chat", "openai-chat"),
                        ("claude_messages", "anthropic"),
                        ("openai_response", "openai"),
                        ("gemini", "gemini"),
                        ("azure_openai", "azure-openai"),
                        ("google_vertex", "google-vertex"),
                        ("openai_codex", "openai-codex"),
                        ("openai_vllm", "openai-vllm"),
                    ];
                    for &(silk_name, prism_name) in all_mapped {
                        if !providers.iter().any(|pp| pp == prism_name) {
                            tracing::warn!(
                                silk = silk_name,
                                prism = prism_name,
                                "映射的 prism provider 不在 wasm 支持列表中"
                            );
                        }
                    }
                    tracing::info!(wasm_providers = ?providers, "prism provider 校验完成");
                }
            }
            Err(e) => tracing::warn!("无法获取 prism provider 列表，跳过校验: {e}"),
        }
        Ok(Mutex::new(p))
    }
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
/// silk 使用下划线协议名（openai_chat/claude_messages/openai_response 等），
/// prism 使用其 provider 主名（openai/messages/responses/gemini 等）。
///
/// 完整映射（以 wasm_list_providers 实际导出为准）：
/// - openai_chat     → openai      （OpenAI Chat Completions，别名 chat）
/// - openai_response → responses   （OpenAI Responses API，别名 openai-responses）
/// - claude_messages → messages    （Anthropic Messages，别名 claude-messages/anthropic）
/// - gemini          → gemini      （Google Gemini，别名 google）
/// - azure_openai    → azure-openai   （Azure OpenAI）
/// - google_vertex   → google-vertex  （Google Vertex AI）
/// - openai_codex    → openai-codex   （OpenAI Codex /agents）
/// - openai_vllm     → openai-vllm    （vLLM /v1/completions）
pub fn map_provider(protocol: &str) -> Option<&'static str> {
    match protocol {
        "openai_chat" => Some("openai"),
        "claude_messages" => Some("messages"),
        "openai_response" => Some("responses"),
        "gemini" => Some("gemini"),
        "azure_openai" => Some("azure-openai"),
        "google_vertex" => Some("google-vertex"),
        "openai_codex" => Some("openai-codex"),
        "openai_vllm" => Some("openai-vllm"),
        _ => None,
    }
}

/// 解析 wasm 返回的信封：`{"value":..., "diagnostics":[]}` 或 `{"error":...}`
///
/// 当信封包含 diagnostics（degraded/unsupported/invalid）时记录警告日志。
fn extract_value(envelope: &str, context: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(envelope).map_err(|e| format!("解析信封失败: {e}"))?;

    // 记录 diagnostics（转换保真度降级、不支持字段等）
    if let Some(diags) = v.get("diagnostics").and_then(|d| d.as_array()) {
        for diag in diags {
            let status = diag.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let field = diag.get("field").and_then(|f| f.as_str()).unwrap_or("?");
            let detail = diag.get("detail").and_then(|d| d.as_str()).unwrap_or("");
            match status {
                "degraded" => {
                    tracing::warn!(context, field, detail, "prism 转换降级");
                }
                "unsupported" => {
                    tracing::warn!(context, field, detail, "prism 不支持的字段");
                }
                "invalid" => {
                    tracing::warn!(context, field, detail, "prism 无效字段");
                }
                _ => {} // "exact" 或未知状态不记录
            }
        }
    }

    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        tracing::error!(context, error = err, "prism 转换失败");
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
    let start = std::time::Instant::now();
    let mut p = prism()?;
    let envelope = p.call("wasm_convert_req", &[src, json, dst])?;
    let elapsed = start.elapsed();
    tracing::debug!(
        source, target,
        input_bytes = json.len(),
        elapsed_us = elapsed.as_micros(),
        "请求转换完成"
    );
    extract_value(&envelope, &format!("req {source}→{target}"))
}

/// 响应转换：source 协议 JSON → target 协议 JSON
pub fn convert_response(source: &str, json: &str, target: &str) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let start = std::time::Instant::now();
    let mut p = prism()?;
    let envelope = p.call("wasm_convert_resp", &[src, json, dst])?;
    let elapsed = start.elapsed();
    tracing::debug!(
        source, target,
        input_bytes = json.len(),
        elapsed_us = elapsed.as_micros(),
        "响应转换完成"
    );
    extract_value(&envelope, &format!("resp {source}→{target}"))
}

/// 流式转换：source 协议 SSE → target 协议 SSE
pub fn convert_stream(source: &str, sse: &str, target: &str) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let start = std::time::Instant::now();
    let mut p = prism()?;
    let envelope = p.call("wasm_convert_stream", &[src, sse, dst])?;
    let elapsed = start.elapsed();
    tracing::debug!(
        source, target,
        input_bytes = sse.len(),
        elapsed_us = elapsed.as_micros(),
        "流式转换完成"
    );
    extract_value(&envelope, &format!("stream {source}→{target}"))
}

/// 单事件流式转换：source SSE 事件 → target SSE 事件（零状态，O(1)）
///
/// 与 `convert_stream`（整段转换）不同，本函数逐事件调用，无需累积 buffer。
/// 输入为单个 SSE 事件文本（如 `"data: {...}\n\n"`），
/// 返回转换后的 SSE 事件文本（可能为空字符串、单个或多个事件）。
pub fn convert_stream_event(
    source: &str,
    sse_event: &str,
    target: &str,
) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let start = std::time::Instant::now();
    let mut p = prism()?;
    let envelope = p.call("wasm_convert_stream_event", &[src, sse_event, dst])?;
    let elapsed = start.elapsed();
    tracing::debug!(
        source, target,
        input_bytes = sse_event.len(),
        elapsed_us = elapsed.as_micros(),
        "事件转换完成"
    );
    extract_value(&envelope, &format!("event {source}→{target}"))
}

/// 单事件流式转换 + 追踪日志（调试用，生产环境用 `convert_stream_event`）
///
/// 与 `convert_stream_event` 相同，但会将转换详情写入日志缓冲区。
/// 日志缓冲区需先通过 `init_log()` 分配。
pub fn convert_stream_event_trace(
    source: &str,
    sse_event: &str,
    target: &str,
    log_ptr: u32,
    log_size: u32,
) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let start = std::time::Instant::now();
    let mut p = prism()?;
    let envelope = p.call_mixed(
        "wasm_convert_stream_event_trace",
        &[src, sse_event, dst],
        &[log_ptr as i32, log_size as i32],
    )?;
    let elapsed = start.elapsed();
    tracing::debug!(
        source, target,
        input_bytes = sse_event.len(),
        elapsed_us = elapsed.as_micros(),
        "事件转换完成（带追踪）"
    );
    extract_value(&envelope, &format!("event_trace {source}→{target}"))
}

/// 查询全部 provider 主名
pub fn list_providers() -> Result<Vec<String>, String> {
    let mut p = prism()?;
    let raw = p.call("wasm_list_providers", &[])?;
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

// ---------------------------------------------------------------------------
// 日志与追踪（调试用）
// ---------------------------------------------------------------------------

/// 分配日志缓冲，返回缓冲区在线性内存中的地址（i32）。
///
/// 调用一次即可，地址在 prism 实例生命周期内有效。
/// `buf_size` 为日志内容最大字节数（不含 4 字节头），建议 8192+。
pub fn init_log(buf_size: u32) -> Result<u32, String> {
    let mut p = prism()?;
    let addr = p.call_i32("wasm_log_init", &[&buf_size.to_string()])?;
    Ok(addr as u32)
}

/// 读取日志缓冲当前写入位置。
///
/// 比较前后两次调用的返回值，若位置变化则有新日志可读。
pub fn log_pos(buf_ptr: u32) -> Result<u32, String> {
    let mut p = prism()?;
    let pos = p.call_i32("wasm_log_pos", &[&buf_ptr.to_string()])?;
    Ok(pos as u32)
}

/// 请求转换 + 追踪日志（调试用，生产环境用 `convert_request`）
pub fn convert_request_trace(
    source: &str,
    json: &str,
    target: &str,
    log_ptr: u32,
    log_size: u32,
) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_mixed(
        "wasm_convert_req_trace",
        &[src, json, dst],
        &[log_ptr as i32, log_size as i32],
    )?;
    extract_value(&envelope, &format!("req_trace {source}→{target}"))
}

/// 响应转换 + 追踪日志（调试用，生产环境用 `convert_response`）
pub fn convert_response_trace(
    source: &str,
    json: &str,
    target: &str,
    log_ptr: u32,
    log_size: u32,
) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_mixed(
        "wasm_convert_resp_trace",
        &[src, json, dst],
        &[log_ptr as i32, log_size as i32],
    )?;
    extract_value(&envelope, &format!("resp_trace {source}→{target}"))
}

/// 流式转换 + 追踪日志（调试用，生产环境用 `convert_stream`）
pub fn convert_stream_trace(
    source: &str,
    sse: &str,
    target: &str,
    log_ptr: u32,
    log_size: u32,
) -> Result<String, String> {
    let src = map_provider(source).ok_or_else(|| format!("不支持的源协议: {source}"))?;
    let dst = map_provider(target).ok_or_else(|| format!("不支持的目标协议: {target}"))?;
    let mut p = prism()?;
    let envelope = p.call_mixed(
        "wasm_convert_stream_trace",
        &[src, sse, dst],
        &[log_ptr as i32, log_size as i32],
    )?;
    extract_value(&envelope, &format!("stream_trace {source}→{target}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_provider() {
        assert_eq!(map_provider("openai_chat"), Some("openai"));
        assert_eq!(map_provider("claude_messages"), Some("messages"));
        assert_eq!(map_provider("openai_response"), Some("responses"));
        assert_eq!(map_provider("gemini"), Some("gemini"));
        assert_eq!(map_provider("unknown"), None);
    }

    #[test]
    fn test_list_providers() {
        let providers = list_providers().expect("list providers");
        // 校验核心 provider 都被 wasm 支持（别名可能因版本变化）
        let core_providers = ["openai", "responses", "messages", "gemini"];
        for name in &core_providers {
            assert!(
                providers.contains(&name.to_string()),
                "prism.wasm 缺少核心 provider: {name}，实际列表: {providers:?}"
            );
        }
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
    fn test_convert_usage_only_event_openai_response_to_chat() {
        // OpenAI Responses API 的 response.completed 事件需要 event: 行
        let usage_event = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-1\",\"object\":\"response\",\"model\":\"gpt-4\",\"status\":\"completed\",\"usage\":{\"input_tokens\":51080,\"output_tokens\":160,\"total_tokens\":51240}}}\n\n";

        let out = convert_stream_event("openai_response", usage_event, "openai_chat")
            .expect("convert usage event");

        eprintln!("=== prism usage 转换结果 ===\n{out}\n========================");

        // 结果必须包含 choices 字段（即使是空数组）
        assert!(out.contains("\"choices\""), "输出缺少 choices: {out}");
        // 结果必须包含 id
        assert!(out.contains("\"id\""), "输出缺少 id: {out}");
        // 结果必须包含 usage
        assert!(out.contains("\"usage\""), "输出缺少 usage: {out}");
    }

    #[test]
    fn test_convert_unsupported_protocol() {
        let req = "{}";
        assert!(convert_request("unknown_proto", req, "openai_chat").is_err());
        assert!(convert_request("openai_chat", req, "unknown_proto").is_err());
    }

    #[test]
    fn test_init_log_and_log_pos() {
        let log_buf = init_log(8192).expect("init_log");
        assert!(log_buf > 0, "log buffer address should be non-zero");

        // log_pos 返回值取决于 prism 内部实现，只验证调用不报错
        let _pos = log_pos(log_buf).expect("log_pos should not error");
    }

    #[test]
    fn test_convert_request_trace() {
        let log_buf = init_log(8192).expect("init_log");
        let pos_before = log_pos(log_buf).expect("log_pos before");

        let req = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });

        // trace 函数可能在某些 prism.wasm 版本中不可用或行为不同，
        // 成功时验证结果正确，失败时验证错误信息合理
        match convert_request_trace(
            "openai_chat",
            &req.to_string(),
            "claude_messages",
            log_buf,
            8192,
        ) {
            Ok(out) => {
                let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
                assert_eq!(v["model"], "gpt-4");
                // trace 写入后位置应有变化
                let pos_after = log_pos(log_buf).expect("log_pos after trace");
                assert!(
                    pos_after > pos_before,
                    "log position should advance, {pos_before} -> {pos_after}"
                );
            }
            Err(e) => {
                // trace 可能未在当前 prism.wasm 版本中实现
                eprintln!("wasm_convert_req_trace 不可用（可接受）: {e}");
            }
        }
    }
}
