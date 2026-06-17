# Design: K1a — Queue-steering wire + state plumbing

Upstream: `.k1a-steering/spec.md` (signed) · `prove-it-prototype` agreement recorded there · rivets `cyril-f2g8`.

## Purpose

Give cyril the wire + state machinery to send `_session/steer[/clear]` and to turn the three `_kiro.dev/session/update` steering echoes into typed notifications + state. No UX (K1b `cyril-bm1j`).

## Architecture (the seams)

1. **`BridgeCommand::SteerSession { session_id, message }` and `ClearSteering { session_id }`** — new typed variants (precedent: `SpawnSession`/`TerminateSession`), each sent as an awaited `ext_method` `ExtRequest` (`_session/steer`, `_session/steer/clear`).
2. **`Notification::SteeringQueued { message }`, `SteeringConsumed { content }`, `SteeringCleared`, `SteeringUnsupported { message }`** — four new variants in `types/event.rs`. The first three are converter-produced echoes; the fourth is bridge-synthesized on `-32601`.
3. **New outer converter arm `"_kiro.dev/session/update"`** in `convert/kiro.rs` `to_ext_notification` (proven: the existing `kiro.dev/session/update` arm is unprefixed and unreachable for these).
4. **Pure decision function** `steer_error_action(code: acp::ErrorCode, already_unsupported: bool) -> SteerAction` (`SteerAction ∈ {MarkAndNotify, AlreadyUnsupported, BridgeError}`) + a bridge-local `HashSet<SessionId>` of unsupported sessions. This is the testable seam — the `-32601` logic is unit-testable with no live backend.
5. **`SessionController`**: `steering_depth: usize` + `steering_unsupported: bool`, reset on new session (precedent: `context_usage` lives in both SessionController and UiState).
6. **`UiState`**: a queued-steer mirror field (for K1b's chip) + `SteeringUnsupported → add_system_message` (the existing system-message channel, state.rs:431-style).

## Input shapes

Inbound converter frame (method, `update.sessionUpdate`, payload):
- `_kiro.dev/session/update` + `steering_queued` + `message:"X"` → produce
- `_kiro.dev/session/update` + `steering_consumed` + `content:"X"` → produce
- `_kiro.dev/session/update` + `steering_cleared` + (no payload) → produce
- `_kiro.dev/session/update` + `steering_queued` + **missing** `message` → drop (no sentinel)
- `_kiro.dev/session/update` + **unknown** `sessionUpdate` / **missing** `sessionUpdate` → drop (tolerant)
- `kiro.dev/session/update` (**unprefixed**) + `tool_call_chunk` / unknown → **unchanged** (regression shape)
- unrelated method → unchanged (outer `other =>`)

Outbound steer/clear result (`acp::Result<ExtResponse>`):
- `Ok({queued:true})` / `Ok({cleared:true})` → no synthesized notification (echo is source of truth)
- `Err(code = MethodNotFound)`, session not-yet-unsupported → mark + notify once
- `Err(code = MethodNotFound)`, session already unsupported → unreachable (pre-send gate skips); idempotent if it races
- `Err(code = other)` → BridgeError, no mark
- pre-send: session already unsupported → no request at all

State transition inputs: `SteeringQueued`, `SteeringConsumed` (depth 0 and >0), `SteeringCleared` (depth 0, 1, >1), `SteeringUnsupported` (flag false→true, true→true), new-session (reset).

Out-of-scope shapes: subagent/non-main session id steering echoes route **global** at K1a (scoped routing = `cyril-28z2` / K1c); steer message content (empty/huge) is sent unvalidated (backend's concern).

## Claims

1. `_kiro.dev/session/update`+`steering_queued`+`message:"X"` → `Ok(Some(SteeringQueued{message:"X"}))`.
2. `…steering_consumed`+`content:"X"` → `Ok(Some(SteeringConsumed{content:"X"}))`.
3. `…steering_cleared` (no payload) → `Ok(Some(SteeringCleared))`.
4. `…steering_queued` with no `message` (and `…consumed` with no `content`) → `Ok(None)` + `warn!`; never `Err`, never `message:""`.
5. `…` with unknown `sessionUpdate`, or missing `sessionUpdate`/`update`, → `Ok(None)` (+debug); never `Err`.
6. The unprefixed `kiro.dev/session/update` arm is unchanged: `tool_call_chunk` still → `ToolCallChunk`; an unknown variant under it still → `Err`.
7. The bridge can detect `-32601` from the `ext_method` error: `err.code == acp::ErrorCode::MethodNotFound` type-checks.
8. `steer_error_action(MethodNotFound, already=false)` → `MarkAndNotify`; the mark is idempotent (a repeat insert yields no second `SteeringUnsupported`).
9. `steer_error_action(code≠MethodNotFound, _)` → `BridgeError`; the session is NOT marked unsupported.
10. A `SteerSession`/`ClearSteering` for an already-unsupported session sends zero `ext_method` requests and emits zero notifications (pre-send gate).
11. `SessionController` applying `[Queued,Queued,Consumed,Cleared]` yields depth `[1,2,1,0]` (floored at 0); `SteeringUnsupported` sets the flag; a new session resets both.
12. `UiState` applying one `SteeringUnsupported{message:M}` adds exactly one system message containing `M`.
13. `UiState` applying `Queued` then `Consumed` returns its queued mirror to 0.

## Falsification

All falsifiers are deterministic unit tests → the Regression fence is the named test itself (no empirical/measurement claims, so no "needs CI test" caveat applies).

| # | Claim | Falsifier (input → falsifying output) | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|---------------------------------------|----------------------|------|--------|------------------|
| 1 | queued→SteeringQueued | captured frame; result ≠ `SteeringQueued{message:"X"}` | captured wire log L25 (input+intended echo) | 5m | pending | `convert::kiro::tests::steering_queued_converts` |
| 2 | consumed→SteeringConsumed | result ≠ `SteeringConsumed{content:"X"}` | captured wire log L37 | 5m | pending | `…::steering_consumed_converts` |
| 3 | cleared→SteeringCleared | result ≠ `SteeringCleared` | captured wire log L120 | 5m | pending | `…::steering_cleared_converts` |
| 4 | missing field→None | frame w/o `message`; result `Ok(Some(…""))` or `Err` | CLAUDE.md no-sentinel rule | 5m | pending | `…::steering_missing_field_drops` |
| 5 | unknown sub-variant→None | frame `sessionUpdate:"steering_xyz"`; result `Err` | spec Decision: tolerant `_kiro.dev/*` | 5m | pending | `…::steering_unknown_variant_drops` |
| 6 | unprefixed arm unchanged | `kiro.dev/session/update`+`tool_call_chunk`; result ≠ `ToolCallChunk` | existing test `…_tool_call_chunk` (mod.rs:938) | 5m | pending | existing tests stay green + `…::unprefixed_session_update_unchanged` |
| 7 | -32601 detectable via `ErrorCode::MethodNotFound` | the comparison doesn't compile | `agent-client-protocol-schema` error.rs (`MethodNotFound`↔-32601) | 1m | **passed** | covered by #8's test (uses the variant; won't compile if API regresses) |
| 8 | MethodNotFound→MarkAndNotify once | `steer_error_action(MethodNotFound,false)≠MarkAndNotify`, or repeat emits 2nd notif | JSON-RPC spec + design decision table | 10m | pending | `protocol::…::steer_error_marks_and_notifies_once` |
| 9 | other code→BridgeError no-mark | `steer_error_action(-32603,false)` marks unsupported | JSON-RPC spec | 5m | pending | `…::steer_error_other_is_bridge_error` |
| 10 | already-unsupported→no send/no notif | a 2nd steer to a marked session sends a request or emits a notif | recording fake transport (send count) + channel drain | 30m | pending | `…::steer_skips_when_unsupported` |
| 11 | SessionController depth+flag+reset | depth sequence ≠ `[1,2,1,0]`, or flag survives new session | hand-computed arithmetic | 10m | pending | `session::tests::steering_state_transitions_and_reset` |
| 12 | UiState one system message | message delta ≠ 1, or text absent | message-list count before/after | 10m | pending | `state::tests::steering_unsupported_adds_one_message` |
| 13 | UiState queue mirror | mirror ≠ 0 after queued+consumed | hand-computed | 5m | pending | `state::tests::steering_queue_mirror` |

**Cheapest falsifier (#7) ran and PASSED** — `e.code == acp::ErrorCode::MethodNotFound` compiled against the real `ext_method` error type (`acp::Result<ExtResponse>`). Recorded probe: the throwaway line was added to the existing `BridgeCommand::ExtMethod` error arm in `bridge.rs`, `cargo check -p cyril-core` succeeded, then reverted. It also falsified a *wrong* sub-claim: `i64::from(ErrorCode)` is **not** implemented — the API is enum comparison only.

### Non-vacuity (a buggy impl that fails each fence)

- #1/#2: an arm that reads the wrong payload key (`content` for queued) → empty/None → drop.
- #3: an arm that requires a payload field → the payload-free `steering_cleared` frame drops.
- #4: `unwrap_or("")` on the payload → emits `SteeringQueued{message:""}` (sentinel) → fails the `Ok(None)` assert.
- #5: copying the unprefixed arm's `Some(other)=>Err` into the new arm → `Err` on a future variant → fails.
- #6: letting the new `_kiro.dev` arm subsume/shadow or accidentally edit the unprefixed arm.
- #8: emitting `SteeringUnsupported` without the `insert()->bool` guard → duplicate messages on a race.
- #9: classifying all `Err` as MethodNotFound → marks unsupported on a transient -32603, permanently disabling steering.
- #10: enqueuing the request before checking the unsupported set → re-sends + duplicate -32601 each time.
- #11: `SteeringCleared` decrementing by 1 instead of zeroing (→ depth 1) or not resetting on new session (→ a 2.6.1 session's "unsupported" leaks onto a later 2.7.0 session).
- #12: no `UiState` arm for `SteeringUnsupported` → 0 messages.
- #13: `SteeringConsumed` not decrementing the mirror.

## Negative space (what K1a deliberately does NOT do)

1. **No UI trigger to originate a steer** — no Enter-while-busy routing, no `/steer` command, no keybind. (K1b, `cyril-bm1j`.)
2. **No toolbar chip and no local-echo transcript entry** for a queued steer. (K1b, `cyril-bm1j`.)
3. **No session-scoped routing of steering notifications** — all route `RoutedNotification::global`; subagent-scoped steering and the `ext_notification` promotion change are K1c (`cyril-28z2`).
4. **No queue-mode client buffering / Ctrl+S parity.** (K1c, `cyril-28z2`.)
5. **No validation of steer message content** — empty or huge messages are sent verbatim; the backend decides.
6. **No `agentInfo.version` preflight** — the optional "nicer message" polish from ROADMAP K1a is not built; `-32601` is the only gate (settled rationale, not deferred work).
7. **No KAS-engine steering path** — KAS uses a different dialect (KAS track).

## Hard-gate checklist

- [x] Every production-reachable input shape covered by a claim (or out-of-scope w/ justification — see Input shapes).
- [x] Every claim has a falsifier in the table.
- [x] Every falsifier names an independent oracle (captured wire log, JSON-RPC spec, hand-computed, existing tests, CLAUDE.md rule).
- [x] Every falsifier names a specific buggy impl that fails it (Non-vacuity).
- [x] Every claim has a distinct verifiable output (one named test each).
- [x] No measurement-based claims → no CI-fence caveat; all fences are deterministic tests.
- [x] Every deferral cites a verified tracker ID: K1b `cyril-bm1j`, K1c `cyril-28z2` (created + verified this session).
- [x] Cheapest falsifier (#7) run and **passed**.
- [x] Negative space ≥3 (7 entries).
