# Budgeted plan — cyril-j16p (KAS-2a): a plain KAS turn renders and ends

Pipeline: `prove-it-prototype` (turn_end capture) → this plan. The cheapest-falsifier
ran 2026-06-29 and **passed with one refinement** (below); its artifacts are committed,
so the slices have real fixtures, not guesses.

## Design basis (prove-it-prototype + cheapest-falsifier — PASSED)

Live free-path capture (`experiments/conductor-spike/probe-kas-turnend-capture.py`,
one real turn, `stopReason: end_turn`):

- **`turn_end` is the terminal lifecycle signal.** Frame = standard `session/update`,
  `sessionUpdate: "session_info_update"`, `_meta.kiro.kind == "turn_end"`, with
  `_meta.kiro.stopReason: "end_turn"` (mirrored at `_meta.kiro.turnEnd.stopReason`).
  Fixture: `crates/cyril-core/tests/fixtures/kas/session_info_update_turn_end.json`.
- **`turn_completion` is distinct — metering only** (`promptTurnSummaries`/`elapsedTime`/
  `status`), fires *before* `turn_end`, is **not** the busy-clear signal. Fixture:
  `…/session_info_update_turn_completion.json`. (Resolves the turn_end-vs-turn_completion
  ambiguity in `turn_end`'s favour.)
- **Refinement that broke a plausible assumption:** observed order is
  `… turn_completion → turn_end → context_usage`. `turn_end` is **not the last frame** —
  a `context_usage` trails it. ⇒ the converter must key on `kind == "turn_end"`
  *specifically*, never on "the last `session_info_update`," and later frames must not
  disturb the now-idle state.
- **Schema (0.11.4):** `SessionUpdate::SessionInfoUpdate(SessionInfoUpdate)`;
  `SessionInfoUpdate { title, updated_at, meta: Option<Meta> }` — all KAS payload is in
  `meta` (`_meta.kiro`), read as `serde_json` (same pattern as `convert/kiro.rs`).
- **Double-`TurnCompleted` hazard is real:** the prompt response *also* returned
  `end_turn`, so both `turn_end` (notification) and the prompt response resolve → two
  completions for one turn. Dedup is load-bearing (Slice 2).

**Two corrections to the issue framing, confirmed in code:**
1. The "CRITICAL REWORK" (busy-guard off `prompt_task.is_finished()`) is **already done** by
   KAS-0/ADR-0004 — `turn_in_flight` clears by *observing* `TurnCompleted` on the internal
   channel (`bridge.rs:463-477`, `:1370-1379`). KAS-2a does not rework the guard; it makes
   KAS *emit* the right `TurnCompleted` and dedups it.
2. The handler goes in **`KasEngine::convert_session_update`** (the `SessionInfoUpdate`
   variant), **not** `convert::kiro::to_ext_notification` — `session_info_update` is a
   standard `session/update` variant, not a `_kiro/*` ext frame.

**Design decision (dedup mechanism):** emit `TurnCompleted` from *both* `turn_end` and the
prompt response, and make the loop observer **idempotent** — clear+forward only when a turn
is in flight, drop otherwise. This satisfies non-blocking (`turn_end` completes even if the
prompt response never returns) and single-completion (the second is dropped). The residual
cross-turn-staleness case (a very-late duplicate after a new same-session turn started) is
**out of scope per [cyril-a71q]** (turn-seq identity hardening).

**Scope refinement (capture-justified):** the issue's `_kiro/terminal/shell_type` acceptance
criterion is **moved to KAS-5 ([cyril-7bdu])**. The capture proved a plain turn renders and
ends with **empty** client capabilities — `shell_type` never fired. It is *gated on
advertising `terminal`*, and advertising `terminal` also makes KAS route shell execution
through `terminal/*` host callbacks cyril does not implement (KAS-5). So the minimal skeleton
**declines `terminal`** (status quo), `shell_type` never fires, `"Shell: undefined"` is
accepted (cosmetic, in the agent's system prompt only). Recommend amending cyril-j16p's
acceptance criteria accordingly. No code change is needed for this — it is the current
`KasEngine::client_capabilities()` returning `ClientCapabilities::new()`.

Net: the walking skeleton is **Slices 1–3**. Agent text + tool-call rendering is already
covered (KAS-0's `KasEngine::convert_session_update` delegates non-`turn_end` updates to the
generic `convert::session_update_to_notification`, which the capture exercised live).

---

## Slice 1: KAS `turn_end` → `TurnCompleted`

**Claim:** A `session/update` whose `SessionInfoUpdate` has `_meta.kiro.kind == "turn_end"`
converts to `Notification::TurnCompleted { stop_reason }`, with `stop_reason` from
`_meta.kiro.stopReason`. Every other `session_info_update` sub-kind
(`user_message_id_assigned`, `turn_completion`, `context_usage`, …) converts to `None`
(ignored in 2a); all non-`SessionInfoUpdate` updates still delegate to the generic converter.

**Oracle:** the captured `session_info_update_turn_end.json` deserialized via
`acp::SessionNotification` and run through `KasEngine::convert_session_update` yields
`TurnCompleted { stop_reason: EndTurn }` — cross-checked against `convert::to_stop_reason`
applied to the raw `_meta.kiro.stopReason` string independently.

**Stress fixture(s)** (expected output written before code):
- `session_info_update_turn_end.json` → `Some(TurnCompleted{EndTurn})`.
- `session_info_update.json` (`user_message_id_assigned`) → **`None`** (guards the
  "every `session_info_update` is a turn end" bug).
- `session_info_update_turn_completion.json` → **`None`** (guards confusing metering for
  completion — the exact ambiguity the falsifier resolved).
- Synthetic `kind == "turn_end"` with **no** `stopReason` → `Some(TurnCompleted{EndTurn})`
  + one `warn!` (load-bearing fallback — see contract below; a dropped turn-end hangs the UI).
- `agent_message_chunk.json` → unchanged delegation to the generic converter (proves Slice 1
  doesn't break text rendering).

**Loop budget:** No new loop. One `match` on the update variant + a constant-depth
`serde_json` lookup (`meta → "kiro" → "kind"/"stopReason"`). O(1) per notification.

**Wall budget:** n/a (not an always-on phase; per-notification, microseconds).

**Files:**
- `crates/cyril-core/src/protocol/convert/kas.rs` (new — `turn_end` reader, mirrors
  `convert/kiro.rs`; keeps KAS specifics out of `mod.rs` per the architecture rule).
- `crates/cyril-core/src/protocol/engine.rs` (`KasEngine::convert_session_update`: intercept
  `SessionInfoUpdate` → `convert::kas::session_info_to_notification`, else delegate).

**Doc-comment-as-contract:** `convert::kas::session_info_to_notification` — "a
`turn_end` frame without `_meta.kiro.stopReason` still completes the turn (defaults
`EndTurn`)." **Load-bearing for correctness** (silently returning `None` would leave the UI
busy forever): a **runtime** fallback that survives release (`unwrap_or(EndTurn)` + `warn!`),
not `debug_assert!`.

**Output streams:** the `Notification` is data (to the App over the channel). The `warn!`
is diagnostic (tracing → `cyril.log`, never stdout — the TUI owns stdout).

**Verification:**
- [ ] Unit tests pass (the 5 fixtures above).
- [ ] Stress fixtures produce the expected `Some`/`None`/fallback outcomes.
- [ ] Oracle: independent `to_stop_reason("end_turn")` agrees with the converter.
- [ ] Loop/wall budgets hold (no loop; O(1)).

---

## Slice 2: idempotent completion (dedup the double `TurnCompleted`, non-blocking)

**Claim:** A KAS turn that produces both a `turn_end` notification *and* a resolving prompt
response forwards **exactly one** `TurnCompleted` to the App and clears `turn_in_flight`
exactly once; a turn whose prompt response never resolves still completes (driven by
`turn_end`); a second `SendPrompt` after `turn_end` is **accepted**, not rejected with
"a turn is already in progress."

**Oracle:** the existing `count_turn_completions` harness helper (`bridge.rs:1706`) counts
App-visible `TurnCompleted`s for one prompt; the independent check is `turn_in_flight`
transitions (Some→None exactly once).

**Stress fixture(s)** (KAS-shaped `FakeAgent`, extending the harness that backs
`second_prompt_rejected_then_next_turn_starts`, `bridge.rs:1881`):
- **Double-fire:** for one prompt, the fake emits a `session_info_update`→`turn_end`
  *and* resolves `prompt()` with `EndTurn`. Expect: `count == 1`; `turn_in_flight` cleared
  once; a follow-up `SendPrompt` is accepted (no `BridgeError "a turn is already in
  progress"`). Designed to fail the double-forward / double-clear bug.
- **Non-blocking:** the fake emits `turn_end` but **never** resolves `prompt()`. Expect:
  `count == 1` within the turn, UI returns to ready. Designed to fail if completion depends
  on the prompt response.
- **v2 regression:** a v2-shaped turn (one prompt-response `TurnCompleted`, no `turn_end`)
  still forwards exactly one and clears once (no behaviour change for the default engine).

**Loop budget:** No new loop. A single added guard in the existing
`Some(routed) = inbound_rx.recv()` arm (`bridge.rs:1370`). O(1) per notification.

**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/protocol/bridge.rs` (the observer guard + the KAS-shaped
fake-agent tests).

**Doc-comment-as-contract:** "forward/clear a `TurnCompleted` only when a turn is in flight;
drop it otherwise." **Load-bearing for correctness** — a forwarded duplicate makes
`UiState` re-run `commit_streaming` + re-book metering (`state.rs:418-437`). Realized as a
**runtime** guard (`if turn_in_flight.is_some() { … } else { /* drop */ }`), survives release.

**Output streams:** `TurnCompleted` forwarded to the App is data; the dropped-duplicate path
emits at most one `debug!` (diagnostic → `cyril.log`).

**Verification:**
- [ ] Unit tests pass (double-fire, non-blocking, v2-regression).
- [ ] Stress fixtures produce the expected single completion / no-hang.
- [ ] Oracle: `count_turn_completions == 1`; `turn_in_flight` Some→None once.
- [ ] Loop/wall budgets hold (no loop; O(1)).

---

## Slice 3: `sess_`-prefixed ids + unknown-variant tolerance (regression lock)

**Claim:** A `sess_`-prefixed session id round-trips through `SessionController`/`UiState`
with no uuid assumption; an unrecognised `_kiro/*` ext frame produces one `debug!` and
`Ok(None)` (no error, no hang).

**Oracle:** the captured live id `sess_00bf2044-…` flows through `SessionCreated` and a
following update keyed to it without panic/format error; the unknown-frame path returns
`Ok(None)` (already at `convert/kiro.rs:714`) — verified by assertion, not by absence of
a crash.

**Stress fixture(s):**
- `SessionCreated { session_id: "sess_00bf2044-…" }` then a `turn_end` update for that id →
  the turn completes and is attributed to that session (guards any `len`/`split`/uuid-parse
  assumption on the id shape — backslashes/length/non-uuid).
- An ext frame `_kiro/does/not/exist` → `Ok(None)` (guards a future refactor that turns the
  unknown-variant drop into an `Err`/panic).

**Loop budget:** No new loop. Pure assertions over existing converters. O(1).

**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/protocol/convert/mod.rs` (or `convert/kas.rs`) — tests only;
no production change expected (this slice *locks in* behaviour the capture showed already
works, so a green run with no edit is a valid outcome — if an edit is needed, the assumption
was wrong and that's the finding).

**Doc-comment-as-contract:** none added (assertion-only slice).

**Output streams:** test assertions only.

**Verification:**
- [ ] Unit tests pass.
- [ ] Stress fixtures produce the expected round-trip / `Ok(None)`.
- [ ] If any production edit was required, it is ≤1 file and re-runs green.

---

## Plan Self-Review

**1. Every loop — complexity stated, within budget?**
- Slice 1 converter: no loop, O(1) `match` + constant `serde_json` lookups. ✓
- Slice 2 observer guard: no loop, O(1) per notification. ✓
- Slice 3: no loop, assertion-only. ✓
- No always-on phase introduced; no wall budget needed. ✓

**2. Every fixture — which bug class, more than happy-path?**
- Slice 1: `turn_completion`/`user_message_id_assigned` → `None` (mis-classify metering/other
  as completion); missing-`stopReason` fallback (silent drop → hang); `agent_message_chunk`
  delegation intact (regression). Adversarial, not happy-path. ✓
- Slice 2: double-fire (double-clear/forward), non-blocking (completion depends on prompt
  response), v2-regression. ✓
- Slice 3: non-uuid `sess_` id (parse/length assumption), unknown ext frame → `Ok(None)` not
  `Err`. ✓

**3. Every doc-comment precondition — classified + enforced?**
- Slice 1 "turn_end w/o stopReason still completes" → load-bearing-correctness → **runtime**
  `unwrap_or(EndTurn)` + `warn!` (not `debug_assert!`). ✓
- Slice 2 "forward/clear only when a turn is in flight" → load-bearing-correctness →
  **runtime** guard. ✓
- Slice 3: no preconditions added. ✓

**4. Every write target — data or diagnostic?**
- `Notification`/`TurnCompleted` → data (App channel). ✓
- `warn!`/`debug!` → diagnostic (tracing → `cyril.log`, not stdout). ✓
- No `println!`/stdout writes added (TUI owns stdout). ✓

**5. Every tracker reference resolves to an issue?**
- Cross-turn-staleness dedup hardening → **[cyril-a71q]** (filed 2026-06-29). ✓
- `shell_type` + `terminal` host callbacks → **[cyril-7bdu]** (KAS-5, existing). ✓
- No other deferrals.

No gaps. Slices 1–3 are the walking skeleton; `shell_type` is scoped to KAS-5 by the
capture-justified decision above (recommend amending cyril-j16p's acceptance criteria).
