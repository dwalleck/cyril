# Codebase Information

> Generated: 2026-04-11 | Codebase: Cyril

## Project Identity

- **Name:** Cyril
- **Repository:** https://github.com/dwalleck/cyril
- **Language:** Rust (Edition 2024, rust-version 1.94.0)
- **License:** MIT
- **Status:** Alpha — functional, under active development

## What Cyril Does

Cyril is a cross-platform TUI client for [Kiro CLI](https://kiro.dev) that communicates via the [Agent Client Protocol (ACP)](https://agentclientprotocol.com). It provides streaming markdown rendering, tool call visibility, approval workflows, and multi-session subagent management — all from the terminal.

- **Linux:** runs `kiro-cli acp` directly as a subprocess
- **Windows:** bridges to `kiro-cli` inside WSL with automatic `C:\` ↔ `/mnt/c/` path translation

## Workspace Structure

Three-crate Cargo workspace:

| Crate | Role | Description |
|-------|------|-------------|
| `cyril` | Binary | TUI application — event loop, terminal I/O, rendering orchestration |
| `cyril-core` | Library | Protocol bridge, ACP client, types, commands, session management |
| `cyril-ui` | Library | UI state machine, widgets, rendering, file completion, syntax highlighting |

## Key Metrics

| Metric | Value |
|--------|-------|
| Source files (`.rs`) | 48 |
| Test functions (`#[test]`) | 431 |
| `cyril` crate | binary, event loop + main |
| `cyril-core` crate | protocol, types, commands |
| `cyril-ui` crate | state, widgets, rendering |

## Build Configuration

- **Workspace resolver:** 2
- **Lints:** `unsafe_code = "forbid"`, `unwrap_used = "deny"`, `expect_used = "warn"`
- **Release profile:** LTO fat, codegen-units 1, symbols stripped
- **Dev profile:** incremental, opt-level 0
- **Test profile:** opt-level 1

## Configuration

User config loaded from `~/.config/cyril/config.toml` (TOML). Falls back to defaults if missing or invalid.

| Section | Key | Default | Purpose |
|---------|-----|---------|---------|
| `ui` | `max_messages` | 500 | Chat history limit |
| `ui` | `highlight_cache_size` | 20 | Syntax highlight LRU entries |
| `ui` | `stream_buffer_timeout_ms` | 150 | Streaming flush timeout |
| `ui` | `mouse_capture` | true | Enable mouse on startup |
| `agent` | `agent_name` | `"kiro-cli"` | Agent binary name |
| `agent` | `extra_args` | `[]` | Extra subprocess args |
