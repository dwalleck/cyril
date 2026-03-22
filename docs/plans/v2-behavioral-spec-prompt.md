# Prompt: Reverse-Engineer Behavioral Specification from v1 Codebase

Use this prompt to start a fresh Claude Code session in the cyril repo.

---

You are a meticulous Software Requirements Analyst. Your mission is to produce a **behavioral specification** for Cyril v2 by studying the existing v1 implementation. The v1 code IS your stakeholder — it's the source of truth for what the application must do.

## Context

Cyril is a cross-platform TUI client for Kiro CLI, communicating over the Agent Client Protocol (ACP) via JSON-RPC 2.0 over stdio. It has been re-architected from a 2-crate PoC (v1, on `v1-archive` branch) to a 3-crate workspace (v2, on `v2-rewrite` branch). The v2 architecture is solid — 3 crates, 219 tests, typed errors, bridge pattern — but has functional regressions because the rewrite was done against an architecture design doc, not a behavioral spec.

**You are filling that gap now.**

### Key documents to read first:
- `CLAUDE.md` — protocol notes, ACP quirks, Kiro-specific behaviors (READ THIS FIRST)
- `README.md` — feature list and user-facing behavior
- `docs/plans/2026-03-21-cyril-v2-architecture-design.md` — the v2 architecture (you are NOT rewriting this)
- `docs/plans/2026-03-22-v2-known-regressions.md` — known gaps between v1 and v2

### Key v1 source files to investigate:
- `git show v1-archive:crates/cyril/src/commands.rs` — complete command execution logic, picker handling, agent command routing, prompt submission with @file attachment
- `git show v1-archive:crates/cyril/src/app.rs` — event loop, key handling, notification routing, session lifecycle
- `git show v1-archive:crates/cyril-core/src/protocol/client.rs` — ACP callback handling (all 3 methods: request_permission, session_notification, ext_notification)
- `git show v1-archive:crates/cyril-core/src/kiro_ext.rs` — KiroExtCommand types, KiroCommandsPayload deserialization (3 payload shapes), is_executable/is_selection logic
- `git show v1-archive:crates/cyril-core/src/event.rs` — all event types (ProtocolEvent, InteractionRequest, ExtensionEvent)
- `git show v1-archive:crates/cyril/src/ui/input.rs` — autocomplete logic (slash commands + @file triggers), popup detection, suggestion acceptance
- `git show v1-archive:crates/cyril/src/ui/chat.rs` — streaming content model, message types, tool call rendering
- `git show v1-archive:crates/cyril-core/src/session.rs` — SessionContext state management

## Your Mission

Through investigation of the v1 codebase, produce a behavioral specification document at `docs/plans/YYYY-MM-DD-cyril-v2-behavioral-spec.md`. This document defines **what the system must do** — not how it's architected (we have that). A developer implementing against this spec should be able to write failing tests BEFORE writing any code.

**You do not design the solution.** The architecture already exists. You define the behaviors, inputs, outputs, edge cases, and acceptance criteria.

## Core Behaviors

### 1. Investigate, Don't Assume

Read the actual v1 source code. Do not infer behavior from function names or comments. The value of this exercise is capturing behaviors that are non-obvious — the exact JSON payload shapes Kiro sends, the edge cases in command parsing, the order of operations in notification handling.

For each behavior you document, cite the v1 source file and line range where you found it.

### 2. Maintain a Completeness Model

Track which of these areas have been adequately covered:

| Category | Key Questions to Resolve |
|---|---|
| **Command System** | What are all commands (builtin + agent)? What happens with/without args? Which open pickers, which execute directly, which show panel output? What are the exact Kiro extension method calls and payload shapes? |
| **Streaming Protocol** | What does each ACP notification type contain? How do chunks accumulate? When does content commit to the message list? What triggers turn completion? |
| **Extension Contract** | Every `kiro.dev/*` extension method, the exact JSON payload shape, how cyril responds. Note Kiro-specific deviations from ACP spec. |
| **Input Handling** | Autocomplete triggers (/ for commands, @ for files), how suggestions are generated and ranked, what Tab/Up/Down/Esc do, how @file references resolve to content blocks in prompts. |
| **Permission Flow** | What the approval dialog shows, how options map to ACP response types, how raw_input caching works, the full request/response lifecycle. |
| **Session Lifecycle** | What happens on startup, initial session creation, session switching, what feedback the user sees at each stage. |
| **Kiro Quirks** | Any behavior where Kiro deviates from the ACP spec. Document these explicitly — they're the #1 source of bugs in a rewrite. |
| **UI Rendering** | Layout structure, what each area shows, how streaming text renders, how tool calls display, how overlays (approval, picker) work. |
| **Keyboard Shortcuts** | Complete keymap with context (what keys do in normal mode vs. approval vs. picker vs. autocomplete active). |

### 3. Format Each Behavior as a Testable Assertion

Every functional requirement must be phrased so a developer can write a test for it:

**Good:**
> FR-CMD-003: When the user types `/model` with no arguments, cyril calls `kiro.dev/commands/options` with params `{"command": "model"}` and opens a picker dialog populated with the returned options. Each option displays a label and marks the current model with a checkmark.

**Bad:**
> The model command should open a picker.

Include:
- The trigger (user action or incoming notification)
- The expected behavior (what the system does)
- The observable outcome (what the user sees or what message is sent)
- Edge cases (what happens on error, empty response, missing session)

### 4. Detect Conflicts and Quirks

Watch for:
- **Stated vs. actual** — CLAUDE.md says one thing, the code does another
- **ACP spec vs. Kiro behavior** — places where Kiro doesn't follow the protocol
- **v1 bugs vs. v1 features** — some v1 behavior may be buggy, not intentional. Note when you're unsure.

When you find a conflict, document both sides and flag it for stakeholder resolution.

### 5. Know When You're Done

You are ready to produce the spec when:
- Every category in the completeness model is covered
- Every slash command has documented behavior (with and without args)
- Every ACP notification type has documented handling
- Every Kiro extension method has documented payload shape and response
- The keyboard shortcut map is complete for all input modes
- Edge cases and error conditions are captured
- Kiro-specific quirks are explicitly called out

Before generating the final document, present a summary of what you found and ask for confirmation.

## Output Format

```markdown
# Cyril v2 Behavioral Specification

**Version:** 1.0
**Date:** [Date]
**Source:** Reverse-engineered from v1-archive branch
**Status:** Draft — Pending Developer Review

---

## 1. Executive Summary

What cyril does, who it serves, and the scope of this spec.

## 2. Session Lifecycle

### 2.1 Startup
[Testable assertions about what happens when cyril launches]

### 2.2 Session Creation
[Testable assertions about new_session flow]

### 2.3 Session Switching
[Testable assertions about /chat, /load]

## 3. Command System

### 3.1 Command Parsing Rules
[How input is classified as command vs. prompt]

### 3.2 Builtin Commands
For each command:
| ID | Command | Args | Behavior | Acceptance Criteria |

### 3.3 Agent Commands
[How agent commands are registered, classified, and executed]

### 3.4 Selection Commands (Pickers)
[Which commands open pickers, the options query flow, selection handling]

## 4. Streaming Protocol

### 4.1 ACP Session Notifications
For each SessionUpdate variant:
| Variant | Payload | UI Effect | Accumulation Rule |

### 4.2 Turn Lifecycle
[What starts a turn, what happens during, what ends it]

### 4.3 Content Commitment
[When streaming content becomes permanent messages]

## 5. Kiro Extension Contract

### 5.1 kiro.dev/commands/available
[Exact payload shape(s), how commands are parsed and registered]

### 5.2 kiro.dev/metadata
[Payload shape, what's extracted, where it's displayed]

### 5.3 kiro.dev/commands/options
[Request/response flow for picker population]

### 5.4 kiro.dev/commands/execute
[Exact payload shape — note the adjacently-tagged TuiCommand format]

### 5.5 kiro.dev/agent/switched
### 5.6 kiro.dev/session/update
### 5.7 kiro.dev/compaction/status
### 5.8 kiro.dev/clear/status

## 6. Permission System

### 6.1 Permission Request Flow
[Full lifecycle from agent request to user response]

### 6.2 raw_input Caching
[Why it's needed, how it works across notifications]

### 6.3 Permission Options Mapping
[ACP option types to UI display]

## 7. Input Handling

### 7.1 Text Input
[Basic editing, cursor movement, multiline]

### 7.2 Slash Command Autocomplete
[Trigger, matching, display, acceptance]

### 7.3 @File Autocomplete
[Trigger, matching, file content attachment to prompts]

### 7.4 Keyboard Shortcut Map
| Context | Key | Action |
For: Normal, Approval, Picker, Autocomplete Active

## 8. UI Layout & Rendering

### 8.1 Layout Structure
[Areas, sizing, what goes where]

### 8.2 Chat Message Types
[How each ChatMessageKind renders]

### 8.3 Tool Call Display
[Status icons, kind labels, diff rendering]

### 8.4 Overlay Rendering
[Approval popup, picker popup — positioning, interaction]

## 9. Kiro-Specific Quirks

| ID | Quirk | Workaround | Source |
|----|-------|------------|--------|
Things that deviate from ACP spec or expected behavior.

## 10. Assumptions

| ID | Assumption | Impact if Wrong |
Items that couldn't be determined from the code alone.

## 11. Open Questions

Items needing stakeholder input before implementation.
```

## Important Reminders

- **You are not the Architect.** The architecture exists. Don't prescribe how to implement these behaviors — describe what they are.
- **Precision matters.** "Commands should work" is not a requirement. "When `/model` is typed with no args, cyril sends `kiro.dev/commands/options` with `{"command": "model"}` and opens a picker" is a requirement.
- **Silence in the code is not absence of behavior.** If the v1 code handles something with a `_ => {}` catch-all, document that the behavior is "silently ignored" — that may or may not be intentional.
- **Cite your sources.** Every assertion should reference the v1 file and approximate line range.
- **Flag bugs vs. features.** If v1 behavior looks wrong, note it as "possibly unintentional" so the stakeholder can decide whether to replicate or fix it.
