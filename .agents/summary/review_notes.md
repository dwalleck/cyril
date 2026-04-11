# Review Notes

> Generated: 2026-04-11 | Codebase: Cyril

## Consistency Check

### Cross-Document Consistency ✅

| Check | Status | Notes |
|-------|--------|-------|
| Crate count (3) | ✅ Consistent | All docs reference three-crate workspace |
| Crate names | ✅ Consistent | `cyril`, `cyril-core`, `cyril-ui` throughout |
| Edition/Rust version | ✅ Consistent | Edition 2024, Rust 1.94.0 in codebase_info.md and verified against Cargo.toml |
| Bridge architecture | ✅ Consistent | BridgeHandle/BridgeSender described consistently in architecture.md, components.md, interfaces.md, workflows.md |
| RoutedNotification routing | ✅ Consistent | Global vs scoped routing described identically across architecture.md and workflows.md |
| Command system | ✅ Consistent | Command trait, CommandRegistry, CommandResult described consistently |
| TuiState trait | ✅ Consistent | Read-only renderer pattern described in architecture.md, interfaces.md, components.md |
| Dependency versions | ✅ Consistent | All versions match workspace Cargo.toml |
| Type names | ✅ Consistent | All type references match actual source code |

### Terminology Consistency ✅

| Term | Usage | Status |
|------|-------|--------|
| "Bridge" | Always refers to BridgeHandle/BridgeSender channel pair | ✅ |
| "Notification" | Always refers to `Notification` enum variant | ✅ |
| "RoutedNotification" | Always includes session_id routing explanation | ✅ |
| "Subagent" | Consistently used (not "sub-agent" or "sub agent") | ✅ |
| "ACP" | Always expanded on first use per document | ✅ |

## Completeness Check

### Well-Documented Areas ✅

- Bridge architecture and channel design
- Notification routing (global vs scoped)
- Command system (registry, trait, builtins, agent commands, subagent commands)
- Tool call lifecycle (start → update → permission → complete)
- Subagent system (tracker, UI state, crew panel)
- All type definitions with field-level detail
- Dependency versions and feature flags
- Configuration options
- Rendering pipeline and adaptive frame rate

### Gaps and Limitations

| Area | Gap | Severity | Recommendation |
|------|-----|----------|----------------|
| `convert.rs` internals | Largest file (2097 LOC) documented at function level but individual conversion logic not exhaustively detailed | Low | The function signatures and test names in the codebase overview provide sufficient navigation. Deep-dive only if modifying conversion logic. |
| `app.rs` full event handling | `handle_notification()` routing logic documented at pattern level but not every match arm | Low | The workflows.md sequence diagrams cover the important paths. |
| Integration tests | `tests/event_routing.rs` mentioned but test scenarios not enumerated | Low | Tests are self-documenting via descriptive names. |
| `examples/test_bridge.rs` | Example file not documented | Low | Development utility, not part of the public API. |
| Error recovery patterns | `draw()` has panic-safe fallback, but other error recovery patterns not cataloged | Medium | Consider documenting error propagation strategy in a future update. |
| `.kiro/` and `.claude/` config | Directory contents mentioned but not detailed | Low | These are external tool configurations, not Cyril source code. |
| Windows/WSL path translation | `path.rs` documented at module level but individual translation functions not detailed | Low | Well-tested (roundtrip tests visible in codebase overview). |
| `docs/` directory | Contains `kiro-acp-protocol.md` and large JS reference files — not analyzed | Low | These are reference materials, not Cyril source. |
| Streaming buffer semantics | `StreamBuffer` documented but boundary detection algorithm not detailed | Low | Small file (149 LOC), easy to read directly. |

### Documentation vs Old AGENTS.md

The previous AGENTS.md (dated 2026-03-20) had several inaccuracies relative to the current codebase:

| Old AGENTS.md Claim | Current Reality |
|---------------------|-----------------|
| "Two-crate workspace" | Three crates: `cyril`, `cyril-core`, `cyril-ui` |
| "Edition 2021" | Edition 2024 |
| "~6,783 LOC" | ~16,174 LOC |
| "~50 test functions" | 431 test functions |
| No mention of subagent system | Full subagent support (tracker, UI, commands, crew panel) |
| No mention of command registry | Trait-based command registry with dynamic agent command registration |
| No mention of bridge architecture | Bridge pattern with RoutedNotification routing |
| ASCII art diagrams | Now using Mermaid diagrams |
| Included volatile LOC counts per file | New docs avoid per-file LOC counts |
| Included generic Rust best practices | New docs focus on repo-specific patterns |
| Hook system documented as active | Hook system is display-only (Kiro runs hooks, Cyril shows them) |

## Recommendations

1. **Keep `index.md` as primary context file** — it contains enough metadata for AI assistants to route queries without loading all files.
2. **Re-run documentation generation** after significant architectural changes (new crates, new major features).
3. **Add Custom Instructions section** to AGENTS.md for human-maintained operational knowledge.
4. **Consider documenting** the error propagation strategy (anyhow vs thiserror boundary) more explicitly if it becomes a source of confusion.
5. **The `convert.rs` file** is the most likely source of bugs when the ACP protocol changes — consider adding a protocol version compatibility note.
