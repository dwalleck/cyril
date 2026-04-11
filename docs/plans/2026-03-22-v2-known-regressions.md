# Cyril v2 Known Regressions

**Date:** 2026-03-22
**Branch:** `v2-rewrite` (v1 preserved on `v1-archive`)
**Status:** Architecture complete, functional regressions remain

## Current State

- 38 commits, 219 tests passing, release binary builds
- All 9 implementation phases complete (workspace through integration)
- Core architecture is sound: 3-crate structure, bridge pattern, TuiState trait, typed errors
- The protocol layer handles all standard ACP notifications correctly
- The issues below are all in the **command system** and **extension handling** — the wiring between Kiro's extension commands and the UI

## Regressions from v1

### 1. Double slashes in autocomplete (`//model`)

**Symptom:** Autocomplete shows `//model` instead of `/model`.

**Root cause:** Kiro's `kiro.dev/commands/available` sends command names WITH a `/` prefix (e.g., `"/model"`). The `update_autocomplete()` method in `state.rs` prepends another `/` when building suggestions.

**V1 behavior:** `AgentCommand.display_name()` returned `format!("/{}", cmd.name)`, but `cmd.name` was stored WITHOUT the `/` prefix. The `kiro.dev/commands/available` payload sends names with `/` — v1's `KiroCommandsPayload` deserializer handled this correctly.

**Fix:** In `convert.rs` `to_ext_notification` for `kiro.dev/commands/available`, strip the leading `/` from command names before creating `CommandInfo`. OR adjust `update_autocomplete` to not add `/` prefix when the name already has one.

### 2. Only builtin commands appear in autocomplete

**Symptom:** Only /help, /clear, /quit, /new, /load show up. Agent commands like /model, /agent, /compact, /tools are missing.

**Root cause:** The `kiro.dev/commands/available` extension notification is now parsed (fixed earlier), but the command names may not be flowing correctly into `UiState.command_names`. The `register_agent_commands` in `CommandRegistry` and the `set_command_names` flow in `App::handle_notification` need verification.

**V1 behavior:** `matching_suggestions()` iterated both `COMMANDS` (builtins) and `agent_commands` (from Kiro). All appeared together in the autocomplete dropdown with descriptions.

### 3. `/model` lists models in chat instead of opening picker

**Symptom:** Running `/model` without args dumps model options as text in chat instead of opening a selection picker.

**Root cause:** The v2 `ModelCommand` builtin sends `ExtMethod` to `kiro.dev/commands/options` and returns `ShowPicker`. But the ExtMethod response is fire-and-forget in the bridge — the response is discarded. The picker options never come back.

**V1 behavior:** `set_model()` called `kiro.dev/commands/options` with `{"command": "model"}`, parsed the response into picker options, and opened a `PickerState`. This was a request/response pattern, not fire-and-forget.

**Fix needed:** `BridgeCommand::ExtMethod` needs a way to return the response to the caller. Options:
- Add a oneshot response channel to `ExtMethod` variant
- Add a new `BridgeCommand::ExtMethodWithResponse` variant
- Make `BridgeSender::send_ext_method()` return a `Result<Value>`

### 4. Agent "selection" commands don't open pickers

**Symptom:** Commands like `/agent`, `/chat`, `/prompts` that should open selection pickers don't work correctly.

**Root cause:** V1 had `KiroExtCommand.meta.input_type == "selection"` to distinguish picker commands from execute commands. V2's `CommandInfo` has `has_options: bool` but the execution path doesn't differentiate — all agent commands go through `AgentProxyCommand` which calls `ExtMethod`.

**V1 behavior:** Selection commands queried `kiro.dev/commands/options` for the option list, then opened a picker. Non-selection commands called `kiro.dev/commands/execute` directly.

### 5. @file references don't attach file contents to prompts

**Symptom:** Typing `@src/main.rs explain this` sends only the text, not the file contents.

**Root cause:** V1's `send_prompt()` parsed `@file` references from the input text, read the file contents via `FileCompleter`, and appended them as additional `ContentBlock::Text` entries in the ACP prompt. V2's `submit_input()` in `app.rs` sends only the raw text.

**V1 behavior:** `file_completer::parse_file_references()` found all `@path` patterns, `completer.read_file()` loaded each one, and they were appended as `<file path="...">contents</file>` blocks.

### 6. Missing Kiro extension: `kiro.dev/session/update` (tool_call_chunk)

**Symptom:** Tool call progress titles don't update during execution.

**Root cause:** `to_ext_notification` returns an error for `kiro.dev/session/update`. V1 parsed the `tool_call_chunk` variant to update tool call titles in real-time.

**Impact:** Low — standard ACP `ToolCallUpdate` provides similar information. But v1 used the extension for more responsive title updates.

## Architecture Notes for the Fix Session

The fixes all center on the **command execution pipeline** and **extension method request/response pattern**. The core architecture (bridge, types, rendering) is solid.

Key files to focus on:
- `crates/cyril-core/src/protocol/convert.rs` — extension notification parsing (bug 1, 2)
- `crates/cyril-core/src/protocol/bridge.rs` — need ext_method response channel (bug 3, 4)
- `crates/cyril-core/src/commands/builtin.rs` — command execution (bug 3, 4)
- `crates/cyril/src/app.rs` — prompt submission with @file contents (bug 5)
- `crates/cyril-ui/src/state.rs` — autocomplete slash handling (bug 1)

Reference: `git show v1-archive:crates/cyril/src/commands.rs` has the full v1 command execution logic including picker handling, model selection, and agent command routing.

Reference: `git show v1-archive:crates/cyril-core/src/kiro_ext.rs` has the `KiroExtCommand` and `KiroCommandsPayload` types with comprehensive deserialization tests.

## Test Coverage Needed

Before fixing, write failing tests for:
1. Command name normalization (strip leading `/` from Kiro names)
2. Autocomplete includes both builtin and agent commands
3. Selection commands open pickers (mock bridge returns options)
4. @file references parsed from input text
5. @file contents attached as content blocks in prompt
6. `kiro.dev/commands/available` payload parsing with all 3 shapes (wrapped, ACP-style, bare array)
