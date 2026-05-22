# Cyril ACP coverage gap vs Kiro 2.4.1 tui.js

What cyril needs to add to match the wire surface that Kiro's bundled `tui.js` (2.4.1) is prepared to handle. Built by comparing tui.js's full known method/variant inventory against cyril's actual dispatch in `crates/cyril-core/src/protocol/`.

Source: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.4.1.js` (sha256 `0a320921…`, 12.10 MB, snapshotted 2026-05-21 after a `kiro-cli chat --tui` launch on the upgraded local install). Method inventory extracted by grepping all `"_kiro.dev/..."` substrings and `sessionUpdate==="..."` case discriminators.

## Method coverage matrix

### `_kiro.dev/*` extension methods

| Method | tui.js | cyril | Notes |
|---|:---:|:---:|---|
| `commands/available`        | ✓ | ✓ | |
| `commands/execute`          | ✓ | ✓ | implicit via response |
| `commands/options`          | ✓ | ✓ | implicit via response |
| `metadata`                  | ✓ | ✓ | gained `effort` field in 2.4.1; cyril may need to deserialize it |
| `agent/switched`            | ✓ | ✓ | |
| `agent/not_found`           | ✓ | ✓ | |
| `agent/config_error`        | ✓ | ✓ | binary dropped this in 2.3.0 but tui.js still handles defensively; cyril does too |
| `clear/status`              | ✓ | ✓ | |
| `compaction/status`         | ✓ | ✓ | |
| `error/rate_limit`          | ✓ | ✓ | |
| `mcp/oauth_request`         | ✓ | ✓ | |
| `mcp/server_initialized`    | ✓ | ✓ | |
| `mcp/server_init_failure`   | ✓ | ✓ | |
| `subagent/list_update`      | ✓ | ✓ | |
| `session/inbox_notification`| ✓ | ✓ | |
| `session/update`            | ✓ | ✓ | (lightweight `tool_call_chunk` carrier) |
| `session/activity`          | ✓ | ✓ | dispatched together with `session/list_update` in cyril |
| `session/list_update`       | ✓ | ✓ | |
| `model/not_found`           | — | ✓ | cyril has handler; not in tui.js inventory (cyril-side parity guard) |
| `account/usage`             | ✓ | **—** | likely usage-data push; cyril gets usage via `commands/execute` response today |
| `docs/cli/terminal`         | ✓ | **—** | terminal docs lookup; purpose unclear without exercise |
| `mcp/governance_disabled`   | ✓ | **—** | **added in 2.3.0** ([[reference-kiro-2-3-0-diff]]) |
| `session/list`              | ✓ | **—** | list past sessions; was removed from binary in 2.3.0 but tui.js still has handler |
| `session/terminate`         | ✓ | **—** | inbound termination notification (cyril has *outbound* `TerminateSession` command but no inbound handler) |
| `settings/list`             | ✓ | **—** | **added in 2.3.0** |
| `settings/set`              | ✓ | **—** | pairs with `settings/list` |

**7 extension methods tui.js handles that cyril doesn't dispatch.** Two were added in 2.3.0 (`mcp/governance_disabled`, `settings/list`+`settings/set`); the others are pre-existing tui.js handlers we never wired.

### Standard ACP session/update variants

| Variant | tui.js | cyril | Notes |
|---|:---:|:---:|---|
| `agent_message_chunk`         | ✓ | ✓ | |
| `agent_thought_chunk`         | ✓ | ✓ | cyril has it; **never observed on the wire** even at Opus xHigh + max effort. Likely Kiro doesn't expose thinking via ACP today |
| `user_message_chunk`          | ✓ | ✓ | |
| `tool_call`                   | ✓ | ✓ | **schema drift**: see below |
| `tool_call_update`            | ✓ | ✓ | **schema drift**: see below |
| `tool_call_chunk`             | ✓ | ✓ | matches doc |
| `plan`                        | ✓ | ✓ | never observed; probably needs implement-style prompt |
| `current_mode_update`         | ✓ | ✓ | |
| `config_option_update`        | ✓ | ✓ | |
| `available_commands_update`   | ✓ | ✓ | |
| `agent_switched`              | ✓ | ✓ | also handled via `_kiro.dev/agent/switched` |
| `retry_warning`               | ✓ | **—** | tui.js case-matches it; cyril has no variant |
| `auth_error`                  | ✓ | **—** | tui.js case-matches it; cyril has no variant |
| `approval_request`            | ✓ | partial | cyril routes `session/request_permission` to `PermissionRequest` channel — but `_meta.trustOptions[]` field is **completely unhandled** |

**2 unhandled `session/update` variants** (`retry_warning`, `auth_error`) + **1 partially-handled** (`approval_request` / `session/request_permission` missing `_meta.trustOptions[]`).

### Client → server methods (cyril sends to agent)

Cyril's `BridgeCommand` variants vs tui.js's outgoing method inventory:

| Method | tui.js sends | cyril sends | Notes |
|---|:---:|:---:|---|
| `initialize`                  | ✓ | ✓ | |
| `session/new`                 | ✓ | ✓ | `NewSession` |
| `session/prompt`              | ✓ | ✓ | `SendPrompt` |
| `session/cancel`              | ✓ | ✓ | `CancelRequest` |
| `session/set_mode`            | ✓ | ✓ | `SetMode` |
| `session/set_model`           | ✓ | ✓ | `SetModel` (Kiro returns "Method not found" — handled via `commands/execute model` instead) |
| `session/load`                | ✓ | ✓ | `LoadSession` |
| `session/spawn`               | ✓ | ✓ | `SpawnSession` |
| `session/terminate`           | ✓ | ✓ | `TerminateSession` |
| `_kiro.dev/commands/execute`  | ✓ | ✓ | `ExecuteCommand` |
| `_kiro.dev/commands/options`  | ✓ | ✓ | `QueryCommandOptions` |
| `authenticate`                | ✓ | **—** | tui.js can call it; cyril doesn't have a variant |
| `session/set_config_option`   | ✓ | **—** | Kiro returns "Method not found" anyway; doc-known dead surface |
| `session/list`                | ✓ | **—** | Kiro v1.29.0+ feature; not exercised |
| `session/attach`              | ✓ | **—** | new in v1.29.0; not exercised |
| `session/fork`                | ✓ | **—** | **the `/rewind` command uses this** — implementing /rewind needs this |
| `session/resume`              | ✓ | **—** | resume a previous session |
| `session/close`               | ✓ | **—** | close a session (different from terminate?) |
| `message/send`                | ✓ | **—** | new in v1.29.0; alternate prompt path |

**8 outbound methods cyril doesn't send.** Of these, `session/set_config_option` is dead in Kiro anyway. `session/fork` is the one with concrete near-term value because it backs the `/rewind` command.

## Schema drift on already-handled variants

From the non-trivial-prompt capture ([[reference-kiro-2-4-1-diff]] — see "Deep wire-surface findings"). Cyril's deserializer accepts the basic shape but is missing newer fields:

### `session/request_permission`

| Field path | Doc says | Wire (2.4.1) | Cyril |
|---|---|---|---|
| `params.options[]` | `optionId`, `name`, `kind` | same | ✓ |
| `params.toolCall` | `toolCallId`, `title` | same | ✓ |
| `params._meta.trustOptions[]` | undocumented | **present** for shell/grep/out-of-workspace; `label`, `display`, `setting_key`, `patterns[]` | **—** |

**Action:** add `trust_options: Vec<TrustOption>` to cyril's `PermissionRequest`, where `TrustOption { label, display, setting_key, patterns: Vec<String> }`. Then a separate UX layer can implement "always allow this pattern" persisted by `setting_key`.

### `session/update.tool_call`

| Field path | Doc says | Wire (2.4.1) |
|---|---|---|
| `name` | string | **absent** (renamed to `kind`) |
| `status` | enum | **absent on initial tool_call** (only on tool_call_update) |
| `kind` | undocumented | `read`, `search`, `execute`, etc. |
| `locations[]` | undocumented | `[{path: "/abs/path"}, …]` |
| `rawInput` | `{path}` (read) | tool-kind-dependent — `command`, `query`, `max_matches_per_file`, `__tool_use_purpose`, etc. |

**Action:** verify `convert/kiro.rs` to_tool_call() reads `kind` not `name`, surfaces `locations[]`, and tolerates the `__tool_use_purpose` field. The `acp` crate's `ToolCall` type may already have `kind` (it's part of ACP); the gap is at the Kiro-side parsing.

### `session/update.tool_call_update`

| Field path | Doc says | Wire (2.4.1) |
|---|---|---|
| `output` | scalar string | **absent** |
| `rawOutput.items[]` | undocumented | tagged union, two variants observed: `Text` (file content), `Json` (shell exec, web search, structured payloads) |

**Action:** cyril's `ToolCall::raw_output` is `Option<serde_json::Value>` so the parser already accepts the tagged-union shape — **the gap is rendering**. Today's display path likely treats `raw_output` as a single blob and renders the same way for everything. The tagged union splits into `Text` (file content → syntax-highlighted code panel) versus `Json` (shell exec → terminal-style stdout/stderr; web search → result list; etc.). Add a thin enum wrapper at the display boundary or pattern-match `items[].Text` vs `items[].Json.*` in the rendering code. No serde changes needed at the type layer.

### `_kiro.dev/metadata`

| Field | Doc says | Wire (2.4.1) |
|---|---|---|
| `sessionId` | required | required |
| `contextUsagePercentage` | required post-turn | optional everywhere; bare `{sessionId}` keep-alives observed |
| `meteringUsage[]` | "typically one entry" | up to 29 entries (one per backend request) for non-trivial turns |
| `turnDurationMs` | post-turn only | post-turn only |
| `effort` | undocumented | **present under thinking models** (Opus 4.7); absent under haiku. Values: `low`/`medium`/`high`/`xhigh`/`max` |

**Action:** add `effort: Option<String>` to cyril's `MetadataUpdated` and toolbar surface. Make all other fields tolerant of absence — bare `{sessionId}` is a valid shape. Sum `meteringUsage[]` rather than taking `[0]`.

## Prioritized action list

**Tier 1 — observable wire deltas that affect display correctness today:**

1. `_meta.trustOptions[]` deserializer + UX surface. Highest product value; pure on-the-wire data that cyril discards.
2. `rawOutput.items[]` tagged-union *rendering* (not deserialization — cyril stores it as `serde_json::Value` which round-trips anything). Display code needs to switch on `Text` vs `Json` variants; today's render path likely doesn't differentiate, so web-search/shell results render the same as file content.
3. `effort: Option<String>` on `MetadataUpdated` + toolbar render. Necessary for showing the active effort level once `/effort` picker is wired.
4. `meteringUsage[]` accumulator (sum array, don't take `[0]`). For non-trivial turns with multiple backend requests this is a real cost-display bug.

**Tier 2 — methods that exist in the wire surface but never observed:**

5. `session/fork` outbound — backs `/rewind`. The slash command will surface as a panel-input UI; the handler needs to emit a `session/fork` request.
6. `_kiro.dev/mcp/governance_disabled` notification — added in 2.3.0. Cyril silently drops it today. Likely small UI surface ("MCP governance has been disabled").
7. `_kiro.dev/settings/list` + `settings/set` — added in 2.3.0. Settings query/mutation extension. No client today exercises it; impact depends on whether it surfaces user-visible config.
8. `retry_warning` and `auth_error` session_update variants. Both have tui.js handlers; missing in cyril.

**Tier 3 — defensive / completeness:**

9. `_kiro.dev/session/terminate` inbound — cyril has outbound but should handle inbound when the agent terminates a subagent session.
10. `_kiro.dev/account/usage` notification — cyril gets usage data via `commands/execute` response today; this push variant is a redundant path but worth dispatching to the same handler.
11. `authenticate` outbound — unclear what triggers it; cyril probably never needs it (kiro-cli is pre-authenticated via `kiro-cli login`).
12. `session/{attach, list, resume, close}` outbound — v1.29.0+ session-management surface. Useful when cyril gains multi-session UI.

## Out of scope from this diff

- `_kiro.dev/docs/cli/terminal` — purpose unclear without live exercise. Drop into the "investigate next" bucket.
- `agent_thought_chunk` — cyril already has the variant. If the backend ever surfaces thinking content on the wire, no code change needed beyond rendering.
- `plan` session-update variant — cyril already has `PlanUpdated`. Needs a prompt that triggers it to verify shape, but no schema work in advance.

## Verification approach

Once each Tier 1 item is implemented, re-run the code-review capture under Opus 4.7 + max effort (see `experiments/conductor-spike/test_bridge-2.4.1-codereview-max.out`) and verify:
- `trustOptions[]` appear in cyril's approval overlay
- `rawOutput` items render as either code (Text) or structured data (Json)
- Toolbar shows `effort=xhigh` / `effort=max`
- `/usage` displays the summed credits across the 29-element `meteringUsage[]`
