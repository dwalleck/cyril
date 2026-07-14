# cyril-vgcm — budgeted plan: /steer clear + steering-echo re-base

Decomposes `.cyril-vgcm/design.md` (approved 2026-07-13; D1–D5 all accepted as
recommended). Claims C1/C2 already ran live as the probe legs (findings
F1/F2/F8); this plan covers C3–C13. Slices execute in order — Slice 1 is the
type foundation every later slice compiles against; Slices 4–6 serialize on
`state.rs`.

**Fixture-provenance rule (applies to Slices 2–3):** converter fixtures are
verbatim captured frames, never hand-invented shapes. v2 new-family shapes are
fully recorded in findings F2 (field names + envelope, live-captured
2026-07-09). The KAS `session_info_update` steering payload location
(`_meta.kiro.*` presumed, per every existing KAS kind) is NOT durably recorded
— Slice 3 starts by re-running `.cyril-vgcm/probe-steer-clear-behavior-2.12.0.py`
in KAS mode and copying the printed `FRAME:` lines into fixture files. If the
capture contradicts the presumed shape, the capture wins and the design's wire
table gets a correction note.

**Output-stream rule (global):** no slice writes to stdout. All diagnostics are
`tracing` macros (→ `cyril.log`); all user-visible output is UI state consumed
by the renderer. No violations to justify.

**Prompt-response discipline:** `_session/steer/clear` is sent via
`conn.ext_method` (a JSON-RPC *request*, awaited) — the existing ClearSteering
arm already does this correctly (memory: `commands/execute`-style fire-and-
forget gets silently dropped).

---

## Slice 1: Notification re-shape + `steering_depth` deletion (foundation)

**Claim:** C13 (delete `steering_depth`; `steering_unsupported` semantics
unchanged; full suite passes) + design component 1 (type re-shape all later
slices compile against).

**Oracle:** compiler + existing test corpus (C13's designated oracle). The
slice is semantics-preserving by construction except the two named deltas
(depth deleted; new variant displayed), so the pre-existing 13 steering fences
must stay green untouched in meaning.

**Stress fixture:**
- Existing `steering_cleared_flips_all`-family tests still pass with the arm
  temporarily ignoring `message_ids` (flip-all preserved until Slice 6) —
  designed to fail if the shape change silently alters reconciliation.
- New: `SteeringClearUnsupported { message }` → exactly ONE system message,
  chips AND `steering_queued` counter untouched. Designed to fail if the new
  variant is wired to `SteeringUnsupported`'s zero-everything arm (the
  plausible copy-paste bug).
- `steering_state_transitions_and_reset` updated: asserts `steering_unsupported`
  transitions survive with `steering_depth` gone.

**Loop budget:** no new loops.
**Wall budget:** n/a (event-driven).

**Files:** `crates/cyril-core/src/types/event.rs`,
`crates/cyril-core/src/session.rs` (primary);
mechanical shape-propagation only (compiler-driven, no semantic change):
`crates/cyril-core/src/protocol/convert/kiro.rs` (construct sites gain
`message_id: None` / `message_ids: vec![]`),
`crates/cyril-ui/src/state.rs` (match shapes + new-variant arm),
`crates/cyril-ui/src/traits.rs` (`SteerEcho` gains `message_id: Option<String>`;
`steer_echo()` constructor sets `None`),
`crates/cyril-ui/src/widgets/chat.rs` (one `..` in the render match).

> **>2-files justification:** a Rust enum variant re-shape
> (`SteeringCleared` unit→struct) is atomic at compile level — every match
> site must change in the same commit or nothing builds. All non-primary
> touches are field-threading with behavior pinned by existing tests.

**Changes (advisory):**
- `event.rs`: `SteeringQueued { message, message_id: Option<String> }`;
  `SteeringConsumed { content, message_id: Option<String> }`;
  `SteeringCleared { message_ids: Vec<String> }` (doc: empty = "everything",
  the old-dialect shape — a semantic convention enforced by Slice 6's C7
  fence, not a runtime check);
  new `SteeringClearUnsupported { message: String }` (doc: bridge-synthesized,
  advisory-only — must NOT touch chips or `steering_unsupported`).
- `session.rs`: delete `steering_depth` field + getter + arms (grep-verified
  zero readers; one stale comment in `state.rs:582` gets trimmed);
  `SteeringClearUnsupported => false`.
- `state.rs`: `SteeringClearUnsupported { message }` → `add_system_message`,
  nothing else.

**Verification:**
- [ ] Unit tests pass (`cargo test -p cyril-core -p cyril-ui`)
- [ ] Stress fixtures produce expected outcomes (flip-all preserved; new
      variant advisory-only)
- [ ] Oracle agrees: full suite + `cargo clippy -- -D warnings` (dead-code
      lint confirms no orphaned `steering_depth` remnants)
- [ ] No new loops; budgets vacuously hold

---

## Slice 2: kiro.rs converts BOTH v2 echo families (C3 + C4)

**Claim:** C3 (new family: `AgentExecutionUserMessageQueued {messageId,
content}` → `SteeringQueued`, `AgentExecutionSteeringInjected` →
`SteeringConsumed`, `AgentExecutionUserMessageCleared {messageIds}` →
`SteeringCleared`) + C4 (old family still converts; ids default None/empty).
D1 accepted: both families stay.

**Oracle:** captured-frame shapes from findings F2 (live wire 2026-07-09) for
the new family; the in-repo K1b captures (`.k1b-steering/*.log`, 2026-06-17 —
pre-date this design) for the old family.

**Stress fixture:**
- New-family frame with `messageId` present but `content` absent → variant
  still emitted, `message: None` + warn (kills "drop on missing field", the
  counter-desync bug class the old arms already guard).
- `messageId: ""` → `message_id: None` (no-sentinel discipline; kills
  empty-string-as-id).
- `AgentExecutionUserMessageCleared` with `messageIds: []` present-but-empty
  AND with the key absent → both yield `message_ids: vec![]` (old-dialect
  "everything" semantics; kills absent-vs-empty divergence).
- Old-family `steering_queued {message}` → `message_id: None` (C4; kills the
  "delete the old arms" simplification — non-vacuity per design).
- `steering_paused` unknown variant still Errs (existing fence, unchanged —
  proves the catch-all survived the arm additions).

**Loop budget:** no loops — three O(1) field reads per frame.
**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/protocol/convert/kiro.rs` only.

**Code (advisory):** mirror the existing three arms; new-family arms read
`content` (string) into `message`/`content` and `messageId` via
`.and_then(as_str).filter(|s| !s.is_empty())`. Tests
`steering_new_family_{queued,injected,cleared}` + extended existing three.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: fixture JSON matches findings-F2 shapes verbatim (reviewer-checkable)
- [ ] No loops; budgets vacuously hold

---

## Slice 3: kas.rs maps the three steering kinds (C5)

**Claim:** C5 — KAS kinds `steering_queued`/`steering_injected`/
`steering_cleared` convert to the same three notifications; `turn_end`,
`context_usage`, `user_message_id_assigned`, `steering_inclusion` behavior
unchanged.

**Oracle:** verbatim captured KAS frames (probe re-run, step 1 below) +
the existing kas fixture corpus for unchanged kinds.

**Step 1 (before any code):** run
`MODE=kas python .cyril-vgcm/probe-steer-clear-behavior-2.12.0.py`, copy the
three `FRAME:` JSON lines into
`crates/cyril-core/tests/fixtures/kas/steering_{queued,injected,cleared}.json`
(full `session/update` envelopes, matching the existing fixture-loading
convention in kas.rs tests). ~10 min including auth-token read.

**Stress fixture:**
- `steering_inclusion` and a hypothetical `steering/documents_changed`-adjacent
  kind must return `None` (kills substring/prefix matching — the probe itself
  had to exclude this noise, so the bug class is live).
- Queued frame with `content` absent → variant emitted, text `None` + warn
  (same counter-desync guard as Slice 2).
- Cleared frame with `messageIds: []` → `SteeringCleared { message_ids: [] }`
  (KAS clear-on-empty broadcasts nothing, but a hostile empty frame must not
  panic or invent ids).
- Existing `turn_end` / `context_usage` / `other_sub_kind_is_ignored` tests
  unchanged and green (non-regression is half the claim).

**Loop budget:** no loops — O(1) key lookups per frame (messageIds vec is
`serde` collect, O(ids) with ids ≤ frame payload; a frame is bounded by the
wire read buffer, not cyril).
**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/protocol/convert/kas.rs` +
`crates/cyril-core/tests/fixtures/kas/steering_*.json` (new fixtures).

**Verification:**
- [ ] Unit tests pass (`kas::tests::steering_kind_*` ×3 + existing corpus)
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: fixtures are unedited probe output (byte-fidelity checkable
      against the probe re-run transcript)
- [ ] Budgets hold

---

## Slice 4: UiState binds SteeringQueued ids to optimistic chips (C8)

**Claim:** C8 — `SteeringQueued { message_id: Some(id), .. }` binds `id` to
the OLDEST id-less `Queued` steer echo; `steering_queued` counter unchanged
(the optimistic count was already taken at `add_steer_echo`; re-counting the
wire echo is the double-count bug — cyril-7z7u contract).

**Oracle:** the cyril-7z7u optimistic-count contract (committed findings:
counter mirrors Queued-echo count, incremented at user-send, never at the
wire echo).

**Stress fixture:**
- Two id-less Queued chips; apply `Queued{Some("id1")}` → FIRST chip gains
  the id, second stays id-less, counter unchanged (kills newest-first binding
  and kills re-count).
- Duplicate wire echo `Queued{Some("id1")}` again → no second chip gains it
  (id uniqueness; kills double-bind).
- `Queued{Some(id)}` with ZERO chips (foreign/multi-client steer — display is
  out of scope per cyril-8lfs) → no-op, counter still unchanged (kills
  "wire echo creates a chip" scope creep).
- `Queued{None}` (old dialect) → no-op as today.

**Loop budget:** one pass over `messages` — O(messages), messages ≤
`max_messages` (500 default; user-configurable, same bound the existing
flip loop already accepts). Event-driven, ≤ ~10^3 ops per echo. Within budget.
**Wall budget:** n/a.

**Files:** `crates/cyril-ui/src/state.rs` only.

**Verification:**
- [ ] Unit tests pass (`steering_queued_binds_id_no_count`)
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: 7z7u contract holds (counter == count of Queued chips after
      every fixture step — assert it explicitly)
- [ ] Loop budget stated and held

---

## Slice 5: UiState SteeringConsumed — id-match preferred, FIFO fallback (C9)

**Claim:** C9 — `Consumed { message_id: Some(id) }` flips the Queued chip
carrying `id`; no match or `None` → FIFO oldest Queued (today's behavior);
counter decrements by actual flips (0 or 1), saturating.

**Oracle:** expected-state fixtures derived from the captured turn-2 sequences
(healthy KAS turn: queued → injected → cleared, same id — findings F4) + the
7z7u counter contract.

**Note on refinement vs today:** current code decrements unconditionally then
flips FIFO. Under id-scoping, decrement follows the flip (counter stays ==
Queued-chip count — the 7z7u invariant). Existing consumed tests still pass
(their Consumed always flips a chip).

**Stress fixture:**
- Chips [A(id1, Queued), B(id2, Queued)]; `Consumed{Some(id2)}` → B flips to
  Applied, A untouched, counter 2→1 (kills FIFO-always: FIFO would flip A —
  the design's named wrong-chip bug).
- `Consumed{None}` after that → A flips (FIFO fallback intact).
- `Consumed{Some(id2)}` AGAIN (duplicate injected echo; id2 now Applied) →
  nothing flips, counter unchanged (kills unconditional decrement — counter
  would drift below chip count).
- `Consumed{Some("ghost")}` with zero Queued chips → no flip, counter stays 0
  (saturation; kills underflow).

**Loop budget:** O(messages) single pass, same 500-cap bound as Slice 4.
**Wall budget:** n/a.

**Files:** `crates/cyril-ui/src/state.rs` only.

**Verification:**
- [ ] Unit tests pass (`steering_consumed_id_match_then_fifo`)
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: counter == Queued-chip count after every step
- [ ] Loop budget held

---

## Slice 6: UiState SteeringCleared — id-scoped drain (C6 + C7)

**Claim:** C6 — `Cleared{ids}` flips exactly the Queued chips whose ids match;
each unmatched id falls back to ONE oldest id-less Queued chip; ids matching
Applied/terminal chips are consumed with no flip; counter -= actual flips.
C7 — `Cleared{[]}` flips ALL Queued, counter → 0 (old-dialect semantics AND
the session-end finalization path, unchanged in meaning). D3 accepted.

**Oracle:** hand-computed expected state for the design's canonical fixture,
cross-checked against the captured turn-2 sequences (C2's frames: KAS fires
post-injection Cleared carrying the already-Applied id — the exact input that
makes id-blind flip-all wrong).

**Stress fixture (the design's canonical one, verbatim):**
- Chips [A(id1, Applied), B(id2, Queued), C(no-id, Queued)]; apply
  `Cleared{[id1, id3]}` → expect: A untouched (id1 matched a terminal chip —
  consumed, no fallback), B untouched (id2 not named), C flipped to Cleared
  (id3 unmatched → oldest id-less fallback), counter -1.
  Kills flip-all AND kills len-based id-blind (`counter -= ids.len()` would
  give -2; flipping B would betray positional matching).
- `Cleared{[]}` with mixed chips → ALL Queued flip, Applied untouched,
  counter → 0 (C7; re-pointed existing `steering_cleared_flips_all`).
- `Cleared{[ghost]}` with NO id-less chips and no matching ids → zero flips,
  counter unchanged, no underflow.
- Session-end finalization (`SessionCreated` arm) still drains everything
  (it reuses the empty-ids path — assert unchanged).

**Loop budget:** O(ids × messages) naive worst case — ids bounded by the
frame's queue snapshot (realistically ≤ 8; even a hostile 10^3-id frame ×
500 messages = 5×10^5, under the 10^6 line, event-driven not always-on).
Advisory: a single pass collecting id→chip-index first makes it
O(ids + messages); take it if the naive shape reads worse.
**Wall budget:** n/a.

**Files:** `crates/cyril-ui/src/state.rs` only.

**Verification:**
- [ ] Unit tests pass (`steering_cleared_id_scoped_*` per-shape)
- [ ] Stress fixtures produce expected outcomes (canonical fixture first)
- [ ] Oracle: counter == Queued-chip count after every step
- [ ] Loop budget stated and held at the hostile-frame scale

---

## Slice 7: `/steer clear` subcommand parse (C10)

**Claim:** C10 — trimmed arg exactly `"clear"` (case-sensitive) →
`CommandResultKind::ClearSteer`; `"Clear"`, `"clear the tests"`, any other
non-empty text → steer text unchanged; empty → usage message unchanged.
D2 accepted: the bare word "clear" is carved out of the steer-text namespace.

**Oracle:** the input-shape enumeration in the design doc (its "Input shapes"
section) — the test IS that list, one assert per shape.

**Stress fixture:**
- `"clear "` (trailing space) → ClearSteer (trim wired).
- `"clear the tests"` → Steer{"clear the tests"} (kills `starts_with("clear")`
  — the design's named non-vacuity check).
- `"Clear"` → Steer{"Clear"} (kills case-folding).
- `""` / whitespace → usage (existing fence extended, not weakened).

**Loop budget:** no loops (one trim + one compare).
**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/commands/builtin.rs`,
`crates/cyril-core/src/commands/mod.rs` (new `CommandResultKind::ClearSteer`
+ constructor). Mechanical compile-touch: `crates/cyril/src/app.rs` gains the
exhaustive-match arm for ClearSteer as a routing-bug `tracing::error!` stub
(mirrors the existing Steer arm at `app.rs:798`) — real dispatch lands in
Slice 8.

**Doc-comment contract:** `/steer`'s "empty arg must NOT produce an empty
steer" stays load-bearing and runtime-enforced (existing early return —
unchanged, re-asserted).

**Verification:**
- [ ] Unit tests pass (`steer_clear_subcommand_parses` covering all shapes)
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: one assert per design input shape, none skipped
- [ ] Budgets vacuously hold

---

## Slice 8: App dispatch_clear_steer (C11)

**Claim:** C11 — ClearSteer routes through a new `dispatch_clear_steer`
mirroring `dispatch_steer`'s position (special-cased before
`handle_command_result`, exactly like Steer at `app.rs:699`); gate reuses the
existing pure `steer_gate(unsupported, has_session)`: no session → advisory
system message, no send; steering-unsupported → advisory, no send; else send
`BridgeCommand::ClearSteering` with ZERO optimistic mutation (D4: silent
success — chips flip only when the `SteeringCleared` broadcast lands).

**Oracle:** the `steer_gate` truth table (existing, committed, CI-tested) —
clear's gating is definitionally identical (a steer-unsupported session has
nothing queued to clear).

**Stress fixture:**
- Gate matrix test (pure, CI-runnable): (unsupported=false, session=false) →
  advisory; (true, true) → advisory; (false, true) → send. Reuses
  `steer_gate` — the test pins that clear does NOT grow its own divergent gate.
- Dispatch with two Queued chips present → after dispatch (send path), chips
  AND counter are untouched, no system message added (kills optimistic drain
  AND kills success-chatter; D4's named bug classes).
- ClearSteer reaching `handle_command_result` → routing-bug error arm (from
  Slice 7) — asserted like Steer's.

**Loop budget:** no loops.
**Wall budget:** n/a.

**Files:** `crates/cyril/src/app.rs` only.

**Doc-comment contract:** `dispatch_clear_steer` doc states "no text
precondition; gating identical to steer by design" — sanity-hint only, no
enforcement needed (nothing silent-wrong happens on misuse; the bridge gate
re-checks).

**Verification:**
- [ ] Unit tests pass (gate matrix + no-mutation dispatch test)
- [ ] Stress fixtures produce expected outcomes
- [ ] Oracle: gate outcomes equal `steer_gate`'s for all four cells
- [ ] Budgets vacuously hold

---

## Slice 9: bridge — clear's -32601 must not poison steer (C12)

**Claim:** C12 — `ClearSteering`'s -32601 arm emits
`SteeringClearUnsupported` (advisory), does NOT insert into the
`steering_unsupported` set, does NOT emit `SteeringUnsupported`; a subsequent
`SteerSession` on the same session still sends. Non--32601 errors →
`BridgeError` unchanged. The pre-send `should_skip_steer` gate on
ClearSteering stays (steer-unsupported ⇒ nothing queued to clear; its
debug-log-only skip is the established SteerSession pattern, pre-existing).

**Oracle:** today's implementation is the named buggy baseline — the design
requires the new test to FAIL against current code (non-vacuity by
construction). Mechanism: extract a pure
`clear_steer_error_action(code) -> {NotifyClearUnsupported, BridgeError}`
(never a mark action, by type — the illegal state is unrepresentable), used
only by the ClearSteering arm; `steer_error_action` stays for SteerSession.

**Stress fixture:**
- `clear_steer_error_action(-32601)` → NotifyClearUnsupported;
  `(-32603)` / others → BridgeError. Companion assert:
  `steer_error_action(-32601, false)` → MarkAndNotify — documenting in one
  test that the two arms now intentionally diverge (this pairing is what
  fails against today's shared-action code).
- Arm-level: after a clear--32601, the session id is NOT in
  `steering_unsupported` (testable at the pure-fn level: no action variant
  can mark; the arm has no insert call — clippy + review pin it).
- Notification text is a fresh literal ("steer/clear not supported…"), not
  `STEERING_UNSUPPORTED_MSG` (kills message-reuse confusion in the UI).

**Loop budget:** no loops.
**Wall budget:** n/a.

**Files:** `crates/cyril-core/src/protocol/bridge.rs` only.

**Doc-comment contract:** the bridge invariant "every command produces a
notification (including errors)" — load-bearing, already runtime-honored:
Ok → broadcast is the notification; Err → SteeringClearUnsupported or
BridgeError; pre-send skip → pre-existing debug-log pattern shared with
SteerSession (documented exception, unchanged here).

**Verification:**
- [ ] Unit tests pass (`clear_32601_does_not_poison_steer`)
- [ ] New test demonstrably fails when pointed at the shared
      `steer_error_action` (run once against a scratch revert — record it)
- [ ] Oracle: divergence pair asserted in one test
- [ ] Budgets vacuously hold

---

## Post-build verification (checkpointed-build final oracle)

Not a slice — the binary-level oracle run after Slice 9:

1. **v2 live smoke** (`kiro-cli acp`, 2.12.0): mid-turn `/steer` → chip
   Queued; echo reconciles to Applied when injected (the revived pipeline —
   dead since the backend rollout); `/steer clear` on a queued steer → chip
   flips Cleared, marker instruction absent from output.
2. **KAS live smoke** (`--agent-engine kas`): same lifecycle; post-injection
   Cleared must NOT flip a second still-queued chip (the id-scoping payoff).
3. Both runnable via the committed probes as cross-checks
   (`.cyril-vgcm/probe-steer-clear-behavior-2.12.0.py` both modes).

## Bookkeeping (with the PR)

- cyril-ppkx (P1, verified open): fixed by Slices 1–6 — close with the PR.
- cyril-nvmh (verified open): close-out NOTE only — echo revival changes its
  phantom-chip calculus; not fixed here.
- cyril-wmc3 (verified open): steering-lifecycle A/B coverage stays there.
- cyril-8lfs (verified open): foreign-steer display stays out of scope
  (Slice 4 explicitly no-ops it).
- cyril-28z2 (verified open): subagent steering untouched.
- Commit `.rivets/issues.jsonl` with every mutation.

## Plan Self-Review

1. **Loops:** Slice 4 O(messages ≤ 500); Slice 5 O(messages ≤ 500); Slice 6
   O(ids × messages), hostile bound 5×10^5 < 10^6, advisory O(ids + messages)
   variant named; converters O(1)/O(ids-in-frame); parse O(arg len). No
   always-on phases. **No gaps.**
2. **Fixtures:** every slice names the bug class each fixture kills
   (flip-all, id-blind len-math, FIFO-always, double-count, double-bind,
   underflow, `starts_with` parse, case-fold, optimistic drain, success
   chatter, shared-error-action poisoning, prefix-matching kinds,
   drop-on-missing-field, empty-vs-absent ids, sentinel empty-string id).
   No happy-path-only slice. **No gaps.**
3. **Doc-comment preconditions:** SteeringCleared empty-vec convention →
   enforced by C7 fence; "never Some(\"\")" id discipline → converter filter
   (runtime); /steer empty-arg → existing runtime early-return re-asserted;
   dispatch_clear_steer → sanity-hint, justified; bridge every-command-notifies
   → runtime-honored, skip-path exception documented as pre-existing.
   **No gaps.**
4. **Write targets:** tracing → diagnostic (cyril.log); UI state → renderer;
   no stdout data writes anywhere. **No gaps.**
5. **Tracker references:** cyril-ppkx, cyril-wmc3, cyril-nvmh, cyril-8lfs,
   cyril-28z2 — all five verified existing and open on 2026-07-13; content
   covers each deferral (checked against issue titles/notes). **No gaps.**

**Claim coverage:** C1 ✅ ran (probe), C2 ✅ ran (probe), C3→S2, C4→S2, C5→S3,
C6→S6, C7→S6, C8→S4, C9→S5, C10→S7, C11→S8, C12→S9, C13→S1. Complete against
the design's claim list.
