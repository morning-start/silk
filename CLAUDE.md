# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**Silk (丝路)** — A pure-local desktop AI multi-model relay/gateway client. Provides a unified local HTTP endpoint at `http://127.0.0.1:xxxx` that bridges three LLM protocol paradigms: OpenAI Chat, Claude Messages, and OpenAI Response. All inbound protocols are bidirectionally converted and **unified to OpenAI Response format for outbound**. Also handles transparent passthrough for image/video AI APIs. All data stays local — no cloud, no server-side component.

See `docs/规划/Silk（丝路）项目完整需求&定位&功能总结.md` for the full product vision and feature list, and `docs/规划/规划使用库.md` for detailed tech selection rationale.

## Commands

```bash
bun install              # Install frontend deps
bun run dev               # Vite dev server (port 1420)
bun run build             # Type-check (vue-tsc) + production build
bun run tauri dev         # Full Tauri dev (frontend + Rust compile)
bun run tauri build       # Build production bundle (.msi/.dmg/.deb)

# Rust-only checks (run inside src-tauri/)
cargo check               # Fast Rust type-check
cargo clippy              # Rust linting
cargo test                # Run Rust tests
```

## Current State

**Backend core implemented** — the Rust backend has a working HTTP gateway with middleware pipeline, protocol adapters, and SQLite persistence:

### ✅ Implemented (Rust Backend)
- **Axum HTTP Gateway** at `src-tauri/src/gateway/` — HTTP server with 7-stage middleware pipeline
- **Middleware Pipeline** (`pipeline.rs`): extract → resolve_route → normalize_protocol → transform_request → dispatch_upstream → transform_response → persist_log
- **Protocol Adapters** at `src-tauri/src/protocol/` — OpenAI Chat, Claude Messages, OpenAI Response adapters with `ProviderAdapter` trait
- **SSE Streaming** (`stream_response.rs`) — Full SSE parsing, heartbeat, reconnect with Last-Event-ID
- **SQLite Persistence** at `src-tauri/src/persistence/` — Provider, RoutingRule, Group, Log, GatewaySettings repos
- **Provider Cache** — TTL-based in-memory cache (5 min)
- **Route Manager** — Host/Path/Method/ContentType matching with group load balancing
- **AES-GCM Encryption** — Secure API key storage
- **Async Log Writer** — Channel-based batch write to SQLite

### 🚧 In Progress
- `src/App.vue` still has boilerplate (no UI yet)
- NaiveUI, Pinia, Vue Router not installed yet
- No frontend views implemented

### 📦 Planned
- `src/views/` — Provider management, routing rules, log viewer, settings
- `src/stores/` — Pinia stores for providers, logs, settings
- `src/components/` — Reusable UI components

## Architecture

```
Vue3 + NaiveUI + Tailwind (UI Layer)
    ↕ Tauri IPC (invoke / events)
Rust Backend
    ├─ Axum HTTP Gateway (127.0.0.1:port)
    │   └─ 7-stage Middleware Pipeline
    ├─ Protocol Adapters (OpenAI/Claude/Response)
    ├─ Provider Cache (TTL 5min)
    └─ Route Manager (Group Load Balancing)
    ↕ async functions
Persistence (SQLite via SQLx)
    ├─ Provider Repo (AES-GCM encrypted keys)
    ├─ RoutingRule Repo
    ├─ Group Repo
    ├─ Log Repo (async batch write)
    └─ GatewaySettings Repo
    ↕
Tauri Native Layer (tray, window management, file I/O)
```

### Key Flow

```
External AI Tool → Axum Gateway
    ↓
extract::initialize() + read_body()    → Generate request_id, read body (2MB limit)
    ↓
resolve_route::run()                   → Match Host/Path/Method/ContentType → find Provider
    ↓
normalize_protocol::run()              → Set inbound/outbound protocol from route
    ↓
transform_request::run()               → Adapter converts to upstream format
    ↓
dispatch_upstream::run()               → Forward to upstream (retry + backoff + SSE reconnect)
    ↓
transform_response::run()              → Adapter converts response
    ↓
persist_log::run() → finalize()        → Async log write + build final response
```

### Module Structure

```
src-tauri/src/
├── lib.rs                    # Tauri entry point
├── main.rs                   # App entry
├── error.rs                  # Global error types
├── crypto/                   # AES-GCM encryption
├── gateway/
│   ├── mod.rs                # Server startup
│   ├── pipeline.rs           # 7-stage middleware pipeline
│   ├── context.rs            # RequestContext + GatewayContext
│   ├── error.rs              # GatewayError enum
│   ├── group_manager.rs      # Provider group load balancing
│   └── middleware/
│       ├── extract.rs        # Request extraction
│       ├── resolve_route.rs  # Route resolution
│       ├── normalize_protocol.rs
│       ├── transform_request.rs
│       ├── dispatch_upstream.rs
│       ├── transform_response.rs
│       ├── stream_response.rs  # SSE handling
│       ├── persist_log.rs
│       └── finalize.rs
├── protocol/
│   ├── adapter.rs            # ProviderAdapter trait + shared helpers
│   ├── registry.rs           # Adapter registry
│   └── adapters/
│       ├── openai_chat.rs
│       ├── claude.rs
│       └── openai_response.rs
├── persistence/              # SQLite repos
├── models/                   # Data models
└── commands/                 # Tauri IPC handlers
```

## Tech Stack Decisions (Locked)

| Choice | Why |
|--------|-----|
| Tauri 2 (not Electron) | 5–30MB bundle vs 150MB+, native performance |
| Axum (not Actix/Salvo) | Best Tokio/Tower ecosystem compatibility |
| NaiveUI (not Element Plus) | Better TS support, smaller, superior theming |
| SQLx + SQLite (not redb/sled) | Need SQL for log pagination/filtering |
| hyper-rustls (not native-tls) | Pure Rust TLS, no cross-platform OpenSSL issues |
| Bun (not npm/pnpm) | Faster installs, HMR, builds |
| Tokio full async | Single async runtime for all I/O |

## Conventions

- **Vue:** Always `<script setup lang="ts">` (Composition API)
- **Tauri 2:** Import from `@tauri-apps/api/core` for `invoke()` (not `@tauri-apps/api/tauri`)
- **TypeScript:** Strict mode enabled — `noUnusedLocals`, `noUnusedParameters` are on
- **Rust:** Use `thiserror` for error enums; `serde_with` for complex serialization
- **Package manager:** Bun, not npm
- **Bundle identifier:** `morning-start.silk`
- **Comments/documents:** Chinese is used in docs and UI text
- **Middleware pattern:** Each stage is a separate module in `middleware/` with a `run()` function
- **Protocol adapters:** Implement `ProviderAdapter` trait, register in `AdapterRegistry`
- **Error handling:** `GatewayError` enum with `status_code()` and `error_code()` methods

## API Endpoints

| Method | Path | Handler |
|--------|------|---------|
| GET | `/health` | Returns `{"status": "ok", "service": "silk-gateway"}` |
| * | `/*` | `GatewayPipeline` (full middleware pipeline) |

## Database Tables

- `providers` — AI service providers (API keys encrypted with AES-GCM)
- `routing_rules` — Host/Path/Method/ContentType matching rules
- `provider_groups` — Groups for load balancing
- `request_logs` — Full request/response logs (async batch write)
- `gateway_settings` — Server config (bind host/port, etc.)
