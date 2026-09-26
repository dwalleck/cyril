# Spec: Thinking on/off toggle on the v2 and KAS engines

## Request (verbatim)
> /gilfoyle claim and implement cyril-k3lz

Issue text (cyril-k3lz): "Thinking on/off toggle on both engines: KAS 'thinking' configOption (0.66.8) + v2 'reasoning' command (2.23.0)"

## What this is
Cyril gains a `/thinking` builtin that reports and sets extended thinking for the main session's current model on both engines, plus a toolbar segment showing the state. Today both engines expose the lever (v2 `reasoning` command, KAS `thinking` configOption) and report the state, but cyril neither reads the state nor offers the lever.

## Roles
- **Operator**: the person driving a cyril session. Types `/thinking`, `/thinking on`, `/thinking off`; reads the system messages and the toolbar.
- **Agent engine** (v2 `kiro-cli acp` or KAS): owns the thinking setting; reports it (v2 `_kiro.dev/metadata reasoning{support, thinkingEnabled?}`; KAS `thinking` select option in `configOptions`) and accepts changes (v2 `commands/execute reasoning {thinkingEnabled}`; KAS `session/set_config_option {configId:"thinking", value}`).

## Thinking state (definitions used below)
Derived from the latest main-session snapshot only (never cached across snapshots):
- **toggleable-on / toggleable-off**: v2 — latest `reasoning.support == "toggleable"` and `thinkingEnabled` is `true` / `false`. KAS — latest configOptions carry a `thinking` option whose current value is `"on"` / `"off"`.
- **toggleable-unknown**: v2 — `support == "toggleable"` with `thinkingEnabled` absent.
- **always-on**: v2 — `support == "alwaysOn"`.
- **not-toggleable**: v2 — `support` is `"unavailable"` or an unrecognized value; KAS — configOptions received without a `thinking` option (or with a `thinking` value other than `on`/`off`, logged at warn).
- **unreported**: no snapshot for the current session yet (includes: before any session, after `/new` until the first snapshot, after disconnect).

A new snapshot of the *other* kind never erases state from the kind that set it (v2 never sends configOptions; KAS never sends `_kiro.dev/metadata`), except that a KAS configOptions snapshot without `thinking` sets not-toggleable.

## Behavior

### B1 Report
- **Given**: any thinking state
- **When**: the Operator enters `/thinking` (no argument)
- **Then**: one system message, nothing sent to the agent engine:
  - toggleable-on → `Thinking is on.`
  - toggleable-off → `Thinking is off.`
  - toggleable-unknown → `Thinking can be toggled on this model, but its current state hasn't been reported yet.`
  - always-on → `Thinking is always on for the current model.`
  - not-toggleable → `Thinking can't be toggled on the current model.`
  - unreported → `Thinking state hasn't been reported yet.`
  Every message is followed by the usage line `Usage: /thinking [on|off]`.

### B2 Set on v2
- **Given**: an active session and thinking state toggleable-on, toggleable-off or toggleable-unknown sourced from v2 metadata
- **When**: the Operator enters `/thinking on` or `/thinking off` (argument case-insensitive, surrounding whitespace ignored)
- **Then**: cyril sends exactly one `kiro.dev/commands/execute` with `{"command":{"command":"reasoning","args":{"thinkingEnabled":true|false}}}` and no other args; on a `success:true` response the chat shows `Thinking turned on.` / `Thinking turned off.` (not `/reasoning: done.`); on `success:false` or an RPC error the chat shows `Thinking change failed: <error text>` (`<error text>` = response `error`/`message` when present, else `unknown error`). The toolbar changes only when the next `_kiro.dev/metadata` snapshot arrives (captured: it follows the response immediately, `v2-reasoning-args2-2.24.0-2.24.0.jsonl:29–30`).

### B3 Set on KAS
- **Given**: an active session and thinking state toggleable-on or toggleable-off sourced from KAS configOptions
- **When**: the Operator enters `/thinking on` or `/thinking off`
- **Then**: cyril sends exactly one `session/set_config_option {configId:"thinking", value:"on"|"off"}`; on the acknowledged rebuilt configOptions (`ConfigOptionSet{config_id:"thinking"}`) the thinking state is taken from that rebuilt set and the chat shows `Thinking turned on.` / `Thinking turned off.` matching the rebuilt value (if the rebuilt set has no `thinking` option: `Thinking can't be toggled on the current model.`); on failure the existing bridge-error message for `set_config_option` is shown.

### B4 Refuse
- **Given**: thinking state always-on, not-toggleable, or unreported
- **When**: the Operator enters `/thinking on` or `/thinking off`
- **Then**: nothing is sent; one system message: always-on → `Thinking is always on for the current model and can't be turned off.` (for `off`) or `Thinking is always on for the current model.` (for `on`); not-toggleable → `Thinking can't be toggled on the current model.`; unreported → `Thinking state hasn't been reported yet — try again after the first reply.`

### B5 Invalid argument
- **Given**: any state
- **When**: the Operator enters `/thinking <x>` where `<x>` is not `on`/`off` (case-insensitive)
- **Then**: nothing is sent; system message `Usage: /thinking [on|off]`.

### B6 No session
- **Given**: no active session
- **When**: the Operator enters `/thinking on` or `/thinking off`
- **Then**: nothing is sent; the existing no-session command error is shown (same path as agent commands).

### B7 Toolbar
- **Given**: thinking state toggleable-on / toggleable-off for the main session
- **When**: the toolbar renders
- **Then**: after the effort badge (or after the model when no effort badge) it shows ` · think` (on) or ` · no think` (off). In every other state no thinking segment is shown.

### B8 Snapshot replacement
- **Given**: any displayed thinking state
- **When**: a new main-session snapshot arrives (v2 metadata with a `reasoning` block; KAS `ConfigOptionsUpdated`/`ConfigOptionSet`), a new session is created, or the bridge disconnects
- **Then**: the state is recomputed from that snapshot alone (session creation / disconnect → unreported); a v2 metadata frame without a `reasoning` block leaves the state unchanged; subagent-scoped frames never change it.

### B9 Discoverability
- **Given**: any engine
- **When**: the Operator enters `/help`
- **Then**: `/thinking` is listed.

## Success criteria
- Each B1/B4/B5 message is emitted verbatim for its state, checked by unit tests on the command against a `SessionController` in each state, asserting zero bridge commands sent.
- B2: exactly one `BridgeCommand::ExecuteCommand{command:"reasoning", args:{"thinkingEnabled":bool}}` (args object has exactly one key), checked by a unit test reading the bridge channel.
- B3: exactly one `BridgeCommand::SetConfigOption{config_id:"thinking", value:"on"|"off"}`, checked by a unit test.
- B2/B3 ack and failure messages, checked by App-level tests feeding `CommandExecuted{command:"reasoning"}` / `ConfigOptionSet{config_id:"thinking"}`.
- B7: toolbar render test on `TestBackend` asserts `think` / `no think` present exactly in the toggleable-on/off states and absent in the other four.
- B8: state tests replaying the captured v2 frame sequence (`v2-reasoning-args2-2.24.0-2.24.0.jsonl` lines 8, 24–30) and the KAS § 7.1 sequence (model → sonnet-4.6 adds `thinking:on`; set off; model → gpt-5.6-luna drops it) assert the derived state after each frame.
- `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` pass, with and without `--features kas`.

## Out of scope
This change does NOT include:
- Picker badges for `thinkingToggleable` / `defaultThinkingEnabled` on model options (cyril-lxuo).
- A KAS effort badge or displaying KAS's thinking-off effort cap (cyril-4jt7).
- A KAS `/model` or `/effort` surface (KAS-4).
- v2 `reasoning` `effort`, `setAsDefault`, `targetModelId` args; suppressing or surfacing the v2 persisted default (cyril-v2ol).
- Bare `/thinking` toggling; autocomplete of `on`/`off` values.
- Per-subagent thinking control.

## Related issues

- cyril-q1xs (closed): shipped the v2 `reasoning` snapshot parse (`ReasoningInfo` on `MetadataUpdated`); this change consumes its `support` + `thinkingEnabled`. Its research note: `thinkingEnabled:false` keeps `effort` on the wire, and "thinking-off display belongs with cyril-k3lz".
- cyril-lxuo (open): per-model capability surfacing in the model picker (`thinkingToggleable`, `defaultThinkingEnabled`, `hasEffort`). Picker badges stay there, not here.
- cyril-4jt7 (open): KAS `effortLevels`/`defaultEffortLevel`; KAS effort badge from configOptions is a pre-existing gap and stays there.
- cyril-838u (open): never carry effort across a model switch. Adopted: this change never caches or re-sends a thinking value across a model switch; state is re-derived from each snapshot.
- cyril-v2ol (open): Kiro `/model` and `/effort` persist user-global defaults. The v2 `reasoning` command also "persists a reasoning default" (static `reasoning.rs`; `defaultThinkingEnabled` flips with the toggle in capture `v2-reasoning-args2-2.24.0-2.24.0.jsonl:29`). Same class of side effect.
- cyril-cxwb (open): lift KAS `session/new` configOptions. Already partly live (domain mediator emits `ConfigOptionsUpdated` from the `session/new` response).
- cyril-fh06 (closed): metadata frames are routed by `params.sessionId`; subagent frames never reach main state.
- cyril-imjx (closed): picker current-row marking.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Is thinking state re-derived from every snapshot rather than cached across model switches? | Yes | cyril-838u (adopted prior art) | B8. |
| Command surface: toggle, explicit, or both? | `/thinking on` and `/thinking off` set; bare `/thinking` reports the current state. No bare-toggle. | Requester chose (a), 2026-09-25 | B1, B2, B3; unknown state never blocks reporting. |
| Toolbar presentation? | When toggleable and known: ` · think` / ` · no think` after the effort badge. Nothing otherwise. | Requester chose (b), 2026-09-25 | B7. |
| `/thinking on\|off` when toggleability is not confirmed? | Refuse locally with a system message naming the reason; send nothing. | Requester chose (a), 2026-09-25 | B4. Both engines silently no-op on a non-toggleable model (§ 7.1 luna row). |
| Chat feedback after an acknowledged toggle? | One system message on both engines: `Thinking turned on.` / `Thinking turned off.`; failures show their error. | Requester chose (a), 2026-09-25 | B2, B3; v2 ack must not render as `/reasoning: done.`. |
| v2 `setAsDefault` / persisted default? | Send only `{thinkingEnabled}`. | Requester chose (a), 2026-09-25; matches captured behavior and `/effort`; side effect tracked by cyril-v2ol | B2 args object has exactly one key. |
| toggleable-unknown on v2: allow set? | Yes — support is confirmed, only the value is unknown. | Consistent with B4's trigger (toggleability, not value) | B2 given-clause includes toggleable-unknown. |
| Argument parsing | `on`/`off`, case-insensitive, trimmed; anything else → usage. | Proposed by agent; confirm at sign-off | B5. |
| Already in the requested state | Send anyway (engine is authoritative; both levers are idempotent). | Proposed by agent; confirm at sign-off | No local short-circuit in B2/B3. |
| Busy turn (concurrent writes) | Allowed mid-turn, no busy gate — same as `/effort` and `/model` agent commands today. | Existing behavior (AgentCommand has no busy gate); KAS applies next request | No `is_busy` check. |
| Empty set / null field | Covered by the state definitions: absent `thinkingEnabled` → toggleable-unknown; absent `thinking` option → not-toggleable; absent `reasoning` block → unchanged. | cyril-q1xs snapshot semantics | B1, B8. |
| Partial failure | KAS rebuilt set without `thinking` after a set → reported as not-toggleable (B3). v2 `success:true` with no follow-up metadata → message shown, toolbar unchanged until next snapshot. | Engine is source of truth | B2, B3. |
| Retries / idempotency | No automatic retry; one request per command. | Levers are idempotent; retry is the Operator re-issuing | B2, B3. |
| Subagent sessions (multi-tenancy boundary) | Main session only; subagent snapshots never touch the state. | cyril-fh06 routing | B8. |
| Max scale / permissions / soft-delete / time-zone / replication lag / cache invalidation | N/A — one local command and one small state value per snapshot; no auth, storage, time or replicas involved. | — | — |
| Unknown KAS `thinking` value (not on/off) | Treated as not-toggleable, warn-logged. | No sentinel defaults; errors not defaults | Thinking-state definitions. |

## Approval

Requester approval (verbatim): "yes, I agree"
Date: 2026-09-25
