# Codebase Information

## Project Overview

**Name:** Cyril  
**Version:** 0.1.0-alpha.1  
**Status:** Alpha - functional but under active development  
**License:** MIT  
**Repository:** https://github.com/dwalleck/cyril

## Description

Cyril is a cross-platform TUI (Terminal User Interface) client for Kiro CLI that communicates via the Agent Client Protocol (ACP). It provides a terminal-based frontend with streaming markdown rendering, tool call visibility, and an approval workflow.

## Technology Stack

### Primary Language
- **Rust** (Edition 2021)

### Core Dependencies
- `agent-client-protocol` 0.9 - ACP protocol implementation
- `tokio` 1.x - Async runtime
- `ratatui` 0.29 - Terminal UI framework
- `crossterm` 0.28 - Cross-platform terminal manipulation
- `pulldown-cmark` 0.12 - Markdown parsing
- `syntect` 5.x - Syntax highlighting
- `serde_json` 1.x - JSON serialization
- `clap` 4.x - CLI argument parsing
- `anyhow` 1.x - Error handling
- `tracing` 0.1 - Logging and diagnostics

### Development Tools
- Cargo workspace with 2 crates
- Comprehensive test suite (50+ test functions)
- Git hooks for code formatting (rustfmt)
- LSP integration for development

## Codebase Statistics

- **Total Files:** 429
- **Lines of Code:** ~6,783
- **Size Category:** Large
- **Functions:** 290
- **Structs/Enums/Classes:** 57
- **Test Coverage:** Extensive (50+ test functions across modules)

## Workspace Structure

```
cyril/
├── crates/
│   ├── cyril/          # Binary crate - TUI application
│   └── cyril-core/     # Library crate - Protocol logic and platform abstraction
├── docs/               # Documentation and implementation plans
├── .kiro/              # Kiro CLI configuration, skills, and agents
├── .claude/            # Claude AI configuration
└── .agents/            # AI-generated documentation (this directory)
```

## Platform Support

- **Linux:** Native execution with direct subprocess communication
- **Windows:** WSL bridge with automatic path translation (C:\ ↔ /mnt/c/)

## Key Features

1. **Streaming TUI** - Real-time markdown rendering with syntax highlighting
2. **Cross-platform** - Native Linux support, WSL bridge for Windows
3. **Hook System** - JSON-configurable before/after hooks on file operations
4. **Slash Commands** - Autocomplete-enabled command system
5. **Tool Call Display** - Real-time visibility into agent actions
6. **Approval Prompts** - Review and approve file writes and command execution
7. **Session Management** - Create, load, and switch between sessions

## Build Information

**Build Command:**
```bash
cargo build --release
```

**Binary Location:**
- Linux: `target/release/cyril`
- Windows: `target/release/cyril.exe`

**Installation:**
```bash
cargo install cyril
```

## Prerequisites

- Rust toolchain (rustup)
- Kiro CLI installed and authenticated
- Windows only: WSL with kiro-cli installed inside

## Development Environment

- **LSP Support:** Configured via `.kiro/settings/lsp.json`
- **Code Formatting:** Automated via git hooks (rustfmt)
- **Skills System:** 15+ development skills in `.kiro/skills/`
- **Custom Agents:** 10+ specialized agents in `.kiro/agents/`
