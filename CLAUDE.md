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

**Scaffold only** — the codebase is the default `create-tauri-app` template. No core features implemented yet:
- `src/App.vue` has the boilerplate greet demo
- `src-tauri/src/lib.rs` has the boilerplate greet command
- `index.html` still has the default "Tauri + Vue + Typescript" title
- No Tailwind, NaiveUI, Vue Router, or Pinia installed
- No axum, SQLx, or protocol conversion dependencies in Cargo.toml

## Architecture (Planned)

The project follows a **5-layer architecture**:

```
Vue3 + NaiveUI + Tailwind (UI)
    ↕ Tauri IPC (invoke / events)
Rust Backend (axum HTTP gateway + protocol conversion + media proxy)
    ↕ async functions
Persistence (SQLite via sqlx)
    ↕
Tauri native layer (tray, window management, file I/O)
```

**Key flow:** External AI tools → axum localhost gateway → protocol conversion (if LLM, unified to OpenAI Response) / passthrough (if media) → upstream provider → SSE streaming response back.

**Protocol design:** Three inbound protocols (OpenAI Chat, Claude Messages, OpenAI Response) are bidirectionally convertible. All models — including Qwen and native OpenAI — are unified to **OpenAI Response** as the single outbound standard format.

### Planned Module Structure (not yet created)

- `src/views/` — Provider management, routing rules, log viewer, settings
- `src/stores/` — Pinia stores for providers, logs, settings
- `src/components/` — Reusable UI components
- `src-tauri/src/gateway/` — axum HTTP server, routing, SSE handling
- `src-tauri/src/protocol/` — Three-protocol bidirectional conversion (OpenAI Chat / Claude Messages / OpenAI Response), unified outbound as OpenAI Response
- `src-tauri/src/proxy/` — Media API transparent forwarding
- `src-tauri/src/persistence/` — SQLite models, sqlx queries
- `src-tauri/src/commands/` — Tauri IPC command handlers

## Tech Stack Decisions (Locked)

| Choice | Why |
|--------|-----|
| Tauri 2 (not Electron) | 5–30MB bundle vs 150MB+, native performance |
| Axum (not Actix/Salvo) | Best Tokio/Tower ecosystem compatibility |
| NaiveUI (not Element Plus) | Better TS support, smaller, superior theming |
| SQLx + SQLite (not redb/sled) | Need SQL for log pagination/filtering |
| sse-reqwest-client (not reqwest-eventsource) | The latter is unmaintained |
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
