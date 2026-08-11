# cyril-jxfu — budgeted plan

Design: `.cyril-jxfu/design.md` (approved 2026-08-10, cheapest falsifier passed).
Claims C1–C9 map onto six slices; coverage table at the bottom.

Feature-gating fact (checked): `Notification::Workflow`, `WorkflowEvent`,
`WorkflowTracker` are UNGATED; only `convert/kas` is behind `feature = "kas"`.
Slices 1–5 are feature-independent; slice 6's replay machinery is kas-gated.

---

## Slice 1: `WorkflowTracker::session_owner` — the ownership query

**Claim:** C3 (claims resolve from node state: re-emit, first-emit/resume,
snapshot-borne; no-sid emissions claim nothing), C4 (ownership persists
through `run_complete`), C9 (scan holds its budget).
**Oracle:** hand-built event fixtures with literal sid asserts; for C4 the
committed replay projection (`oracle-replay-expected.json`) independently
shows node sids surviving the full capture (probe 3).
**Stress fixture:**
- no-sid `node_start` → `None` for every queried sid (empty/absent path);
- claim via RE-emission (double-emit shape) → `Some`;
- claim via FIRST emission on a fresh run (resume shape) → `Some`;
- claim arriving ONLY via `apply_snapshot` node state → `Some`;
- duplicate claim (same sid twice) → still `Some`, `apply_event` second time
  reports no change (idempotence);
- TWO nodes in one run claiming DIFFERENT sids → each resolves to its own
  `(workflow_id, node_path)` (collision class: shared-node-id/distinct-path);
- full DAG-fixture event replay, then query after `run_complete` → both step
  sids still `Some` (C4);
- unknown sid → `None`;
- scale: 1 run × 200 claimed nodes × 10,000 queries under budget (C9).
**Loop budget:** `session_owner` scan is O(runs × nodes) per call; production
scale runs ≤ ~4, nodes ≤ ~200 → ≤ 800 `Option<&SessionId>` compares per
scoped frame; at a pathological 200 frames/s that is 1.6×10^5 compares/s —
under the 10^6 always-on line. No allocation per call.
**Wall budget:** C9 test: 10k queries × 200 nodes < 2s under the test profile
(CI-safe ceiling per house style, cf. commit 7383590; expected actual ≪ 100ms).
**Files:** `crates/cyril-core/src/workflow.rs`

**Code (advisory):** iterate `self.runs`, for each `run.nodes` compare
`node.session_id() == Some(sid)`, return first `Some((workflow_id, path))`.
Multiple runs claiming one sid cannot happen wire-side (sids are per-step);
first-match is documented as unspecified-order tie behavior, not a contract.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixtures produce expected outcomes
- [ ] Replay projection (probe 3) still agrees on the DAG capture's sids
- [ ] C9 budget holds at fixture scale

---

## Slice 2: `SubagentUiState::remove_stream` + reuse visibility

**Claim:** C8 seam (removal clears focus at the ONE place it cannot be
forgotten) + C5 substrate (a stream can leave the subagent store intact).
**Oracle:** focus/streams accessors asserted literally around the call.
**Stress fixture:**
- remove a missing key → `None`, store unchanged (empty path);
- remove the FOCUSED stream → `Some(stream)`, `focused_session_id()` is
  `None` afterwards (C8);
- remove a NON-focused stream while another is focused → focus RETAINED on
  the survivor (adversarial counterpart: an unfocus-everything bug fails);
- removed stream carries its messages (returned by value, not dropped).
**Loop budget:** none — two `HashMap` ops, O(1).
**Wall budget:** n/a (not always-on).
**Files:** `crates/cyril-ui/src/subagent_ui.rs`

**Code (advisory):** `pub fn remove_stream(&mut self, sid) -> Option<SubagentStream>`
clearing `self.focused` when it matches; widen `SubagentStream::new` and
`SubagentStream::apply_notification` to `pub(crate)` for slice 3's reuse.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixtures produce expected outcomes
- [ ] No behavior change for existing subagent tests (full `cargo test -p cyril-ui`)

---

## Slice 3: `WorkflowUiState` store + `UiState` delegation + re-parent op

**Claim:** C5 substrate (adopt preserves history; create-on-first-contact;
no subagent stream keyed by a workflow-owned id after re-parent) and C7
substrate (`any_workflow_active`).
**Oracle:** message texts/order asserted literally; activity asserted per
`Activity` enum.
**Stress fixture:**
- `apply_notification` on unseen sid creates the stream (first-contact);
- adopt a stream carrying 2 committed messages + 1 tool-call index entry →
  workflow store holds SAME messages in SAME order, tool-call updates still
  merge by id after adoption (index survives the move);
- adopt onto an OCCUPIED key (impossible single-threaded today, load-bearing
  if it ever happens): adopted (older) messages are spliced BEFORE existing
  ones with a `warn!` — chronological order preserved, nothing dropped;
- `any_workflow_active`: one stream `Streaming` among idle ones → `true`;
  all `Ready`/`Idle` → `false` (adversarial pair);
- `UiState::claim_stream_for_workflow(sid)`: subagent store loses the key,
  workflow store gains it, focus cleared if it pointed there (delegates to
  slice 2).
**Loop budget:** adopt splice is O(messages of the two streams) — bounded by
what streamed, only on claim events (not always-on). `any_workflow_active`
is O(streams) per frame-rate tick, streams ≤ ~20 → trivial.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/workflow_ui.rs` (new),
`crates/cyril-ui/src/state.rs` (+ one `mod workflow_ui;` line in `lib.rs`,
declared here so the module has its production consumer in the same slice —
staged-module dead_code hazard).

**Code (advisory):** `WorkflowUiState { streams: HashMap<SessionId, SubagentStream> }`
with `apply_notification`, `adopt`, `streams`, `any_active`. `UiState` gains
the private field + `apply_workflow_notification`, `any_workflow_active`,
`claim_stream_for_workflow`, `workflow_streams` (read accessor for tests/zd8u).

**Doc-comment contract:** `adopt`'s "target key vacant" precondition is
load-bearing (violation would silently drop history) → runtime splice+warn,
NOT `debug_assert!`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixtures produce expected outcomes
- [ ] `cargo test -p cyril-ui` fully green (no subagent regressions)

---

## Slice 4: classifier fourth input + `NotificationRoute::Workflow` + route arm

**Claim:** C1 (owned ∧ scope≠main → Workflow across every `main` ×
`tracked` combination; owned=false rows byte-identical), C2 (scope==main →
Main even when owned).
**Oracle:** the expected table is hand-derived in-test from the design doc's
priority list — an explicit enumeration, not a re-derivation from the impl.
**Stress fixture:** the extended `classify_notification_route_truth_table`:
- ALL cells: scope ∈ {None, ==main, ≠main} × main ∈ {None, Some} ×
  tracked ∈ {f,t} × owned ∈ {f,t} (reachable set; scope==main requires
  main=Some);
- the anomaly cell (scope==main, owned=true) → Main, with an assert message
  naming C2 (a build that tests `owned` before `scope==main` fails HERE and
  nowhere else — distinct localization);
- owned=true, main=None, tracked=false → Workflow (attributable without
  main; a build that lets the Drop arm win over ownership fails);
- owned=true, tracked=true → Workflow (ownership beats trackedness);
- every owned=false row asserted equal to today's table (an accidental
  behavior change to the legacy rows fails loudly).
App wiring in the same slice: call site computes
`workflow_owned = self.workflow_tracker.session_owner(sid).is_some()`; the
`Workflow` arm mirrors the Subagent arm (`apply_workflow_notification`,
redraw, early return) with a `debug!` line for the pre-claim/unknown case.
**Loop budget:** one `session_owner` call per scoped notification (slice 1's
budget). No new loops.
**Wall budget:** covered by slice 1's C9.
**Files:** `crates/cyril/src/app.rs`

**Verification:**
- [ ] Extended truth table passes (and pre-existing routing tests untouched)
- [ ] `cargo test -p cyril` green
- [ ] Clippy pedantic `-D warnings` green

---

## Slice 5: late-claim sweep + frame-rate wiring

**Claim:** C5 (a claim landing after frames re-parents the optimistic stream,
messages intact; afterwards no subagent stream key is workflow-owned), C7
(fast tick while a workflow stream is active), C8 (focused stream re-parent
unfocuses) — all at the App level.
**Oracle:** message texts asserted literally; tick selection asserted at the
strongest testable seam (the `any_*_active` disjunction inputs/outputs);
focus accessor.
**Stress fixture (App unit tests):**
- capture-shaped ordering: main created → 2 absorbable frames for foreign sid
  X (optimistic subagent stream, 2 messages) → `Notification::Workflow`
  node_start claiming X → assert: subagent streams lack X, workflow stream X
  holds the SAME 2 messages in order, `is_subagent(X)` false; a further
  X-frame appends to the workflow stream (3 messages) and NOT to a re-created
  subagent stream (a missing-sweep build fails the intersection assert; a
  drop-not-adopt build fails the message-count assert);
- claim with NO prior stream → no subagent entry, no workflow entry until the
  first post-claim frame (fresh-create path);
- claim event that applies with `Ok(false)` (duplicate) → sweep may run or
  skip, but state is already invariant-clean (assert invariant, not
  mechanism);
- focused optimistic stream X, then claim X → `focused_session_id()` None
  (C8 at App level);
- frame rate: workflow stream `ToolRunning`, subagents idle, voice off →
  fast tick; all idle → slow tick (adversarial pair, C7).
Sweep skips nothing it shouldn't: main's sid never appears among subagent
stream keys (main frames never route Subagent — classifier rows), so the
sweep needs no main-guard; asserted implicitly by the truth table.
**Loop budget:** sweep is O(subagent_streams × session_owner) per
state-changing workflow event: ≤ 20 streams × 800 compares = 1.6×10^4 ops on
a low-rate event (node lifecycle, not streaming) — well under budget.
**Wall budget:** n/a (event-driven, low rate).
**Files:** `crates/cyril/src/app.rs` (sweep in the `Notification::Workflow`
arm after `Ok(true)`, one added disjunct at the tick selector app.rs:313)

**Verification:**
- [ ] App unit tests pass (C5/C7/C8 fixtures above)
- [ ] Invariant test: after every applied workflow event in the fixtures, no
      subagent stream key is workflow-owned
- [ ] `cargo test -p cyril` green

---

## Slice 6: the capture replay fence (AC1)

**Claim:** C6 — replaying the REAL `kas-custom-dag-2.16.0.jsonl` through the
real conversion path attributes every forwarded frame correctly.
**Oracle:** probe 1 + `oracle.sh` (committed, text-only) fix the sids and
per-session frame counts; the fence's expected constants cite them.
**Stress fixture:** the live capture itself — it already embeds the bug
class (late claim, ignored-kind bootstrap frames, interleaved branches).
Expected outcomes pre-registered here, BEFORE the fence is written:
- exactly 2 workflow streams, keyed `sess_a3d8bb37…` / `sess_fd35dac1…`;
- each holds exactly ONE committed message: its step's completed "Send
  Message" tool call (pre-claim frames are ignored kinds — config, session
  info, commands — so adopted history is structurally empty in THIS capture;
  the synthetic S5 fixture covers non-empty history);
- `SubagentUiState` ends with ZERO streams (the optimistic entries created at
  lines 33/39 were re-parented at the claims, lines 46/48);
- `SubagentTracker` never tracks any of the three sids;
- main pipeline saw the parent's frames only (test counters:
  `record_subagent_ui_apply` stays at the pre-claim count, none after the
  claims).
If any pinned constant disagrees at run time, the FENCE is not adjusted until
the disagreement is explained against probe 1's line-level data (drift here
means either a conversion arm surprise or a routing bug — investigate, don't
re-pin).
**Loop budget:** test-only; 82 lines × conversion — trivial.
**Wall budget:** n/a (test).
**Files:** `crates/cyril-core/src/test_support.rs` (kas-gated
`capture_to_notifications(&str) -> Vec<(Option<SessionId>, Notification)>`
wrapping the pub(crate) converters — session/update via acp deser +
`session_update_to_notification` (+ `session_info_to_notification` for the
KAS envelope), `_kiro/workflow/*` via the workflow adapter),
`crates/cyril/src/app.rs` (the `#[cfg(feature = "kas")]` fence test).

**Output stream note:** the helper returns values; any skipped/unconvertible
line logs `debug!` (diagnostic, tracing) — no stdout.

**Verification:**
- [ ] Fence passes with the pinned constants
- [ ] Fence fails against a build with slice 4's arm reverted (manual
      mutation check during build — the fence must be able to fire)
- [ ] `cargo test -p cyril --features kas` AND default-features both green
- [ ] `cargo test -p cyril-core --features kas,test-support` green

---

## Claim coverage

| Claim | Slice | Fence |
|---|---|---|
| C1 | 4 | extended truth table |
| C2 | 4 | truth table anomaly row (distinct message) |
| C3 | 1 | workflow.rs unit family |
| C4 | 1 | DAG-fixture replay + post-terminal query |
| C5 | 3 (substrate), 5 (App) | adopt tests + late-claim App test |
| C6 | 6 | capture replay fence |
| C7 | 3 (substrate), 5 (App) | any_workflow_active + tick tests |
| C8 | 2 (seam), 5 (App) | remove_stream focus tests + App unfocus test |
| C9 | 1 | scale-budget test |

## Plan self-review

1. **Loops:** slice 1 scan O(runs×nodes) ≤ 800/call, 1.6×10^5/s worst —
   stated, budgeted, C9-fenced. Slice 3 adopt splice O(messages), claim-rate
   only. Slice 5 sweep 1.6×10^4/event, low-rate. Slice 6 test-only. No loop
   without a statement.
2. **Fixtures:** every slice names its bug class (absent-sid/empty path,
   idempotence, collision, unfocus-everything, occupied-adopt splice, legacy
   row drift, missing-sweep vs drop-not-adopt, tick omission, real-capture
   late claim). No happy-path-only fixture.
3. **Doc-comment preconditions:** one load-bearing contract (adopt's vacant
   target) → runtime splice+warn. `session_owner`/`classify` are total, no
   preconditions. No unenforced contract shipped.
4. **Write targets:** tracing only (`warn!`/`debug!`) — diagnostics; no
   stdout/file writes anywhere in the six slices.
5. **Tracker references:** cyril-zd8u (rendering), cyril-kzke (rename),
   cyril-0qe6 (commands/lifecycle), cyril-ebqu / cyril-fjfu (agent-subtask
   channel) — all verified to exist this session with covering descriptions.
   No uncited deferral.
