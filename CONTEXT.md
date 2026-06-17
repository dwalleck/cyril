# Cyril

Cyril is a polished terminal interface for the Agent Client Protocol (ACP) ecosystem: one TUI that drives any registered ACP agent, with composable proxy stages adding behaviors no agent ships natively. This file is the project's glossary — the canonical name for each domain concept. It is not a spec; implementation lives in code, direction lives in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Language

### Agents & engines

**Vendor**:
An agent provider selectable from the ACP registry — Kiro, Claude, Codex, Goose, and others. The unit the agent picker and registry reason about.
_Avoid_: provider, backend (when you mean the vendor), agent (reserve "agent" for the running process)

**Engine**:
A Kiro-internal agent implementation — **v2** (the Rust engine, `kiro.dev/*` / `_kiro.dev/*` wire dialect) or **KAS** (the TypeScript/LangGraph engine, `_kiro/*` dialect). Engine is an axis *within* the Kiro vendor: both engines share the `kiro-cli` binary, the `~/.kiro` auth store and session storage, and Kiro's slash-command/mode heritage, differing mainly in wire dialect and lifecycle. Cyril binds one engine at agent-subprocess spawn (startup): the bridge runs one `kiro-cli acp [--agent-engine kas]` process and holds one engine for its life, so every session on that process shares it. Switching engines means a new subprocess.
_Avoid_: mode (means something else in Kiro — vibe/spec), version (v2/v3 are engines, not release versions), variant

**v2**:
The Kiro engine cyril drives today (`kiro-cli acp`, default). Rust, `sacp`-based, `kiro.dev/*` dialect.
_Avoid_: rust engine, classic, legacy

**KAS** (Kiro Agent Server):
The Kiro TypeScript/LangGraph engine, embedded as of kiro-cli 2.7.1, reached over `kiro-cli acp --agent-engine kas`. `_kiro/*` dialect; host supplies auth; can call fs/terminal callbacks; uses the `agent-subtask` subagent model.
_Avoid_: v3 (it's the user-facing TUI alias `--v3`, but the engine is KAS), TypeScript engine
