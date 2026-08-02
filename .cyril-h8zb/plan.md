# cyril-h8zb plan — budgeted slices

Design: `.cyril-h8zb/design.md` (approved 2026-08-01: reconcile=yes, wording=as-spec'd,
dedupe=per-turn). Claim numbers below reference the design's claim list.

Gates per slice: `cargo test -p <touched crates>`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, plus the slice's stress fixture. The prove-it oracle
(`.cyril-h8zb/probe-wire-refusal.py` + `oracle-wire-refusal.sh`) is re-run at the end of the
build; it reads shipped Kiro artifacts, not cyril code, so slice-level re-runs are no-ops by
construction — it pins the CONTRACT the fixtures encode.

No slice introduces a loop (all per-frame O(1) field reads; the dedupe is a bool). No always-on
phase is touched, so no wall budgets. All diagnostics go through `tracing` (log file), no stdout.

## Slice 1: `RefusalAlert` type + stale-comment deletion

**Claim:**   design #10 + the type substrate for #1-#9.
**Oracle:**  field asserts against literal inputs; `grep -c "hardcodes" types/session.rs` == 0.
**Stress fixture:** `RefusalAlert::from_parts(Some(""), Some("x"), Some(""))` → category `None`,
  explanation `Some("x")`, recommended_model `None` (sentinel-leak bug class: a `Some("")`
  surviving construction later selects the wrong claim-8 message branch).
**Loop budget:** none (no loops).
**Files:** `crates/cyril-core/src/types/session.rs` (type + tests + delete NOTE lines 573-575).

Notes: private fields + getters (TurnSummary idiom). `from_parts` normalizes `Some("")`→`None` —
this is the runtime enforcement of the doc contract "fields are never empty strings"
(load-bearing: claim 8's explanation-vs-fallback branch keys off `is_some()`). Doc comment cites
the carved wire shape. Type is `pub` (no dead-code warning while unconsumed).

**Verification:** unit tests pass; fixture passes; clippy/fmt clean.

## Slice 2a: `refusal` field threaded through `MetadataUpdated` (always `None`)

**Claim:**   design #2's substrate — field exists, zero behavior change.
**Oracle:**  the entire existing metadata test corpus (pre-dates this feature) stays green
  unmodified except for mechanical `refusal: None` literal additions.
**Stress fixture:** the existing 1gim fence `to_ext_notification_metadata_refusal_and_stop_reason_not_flagged`
  still passes with the parser emitting `refusal: None` (bug class: field-add silently changing
  parse output or missing a construction site — the compiler enforces the latter).
**Loop budget:** none.
**Files:** `crates/cyril-core/src/types/event.rs` (field + doc),
  `crates/cyril-core/src/protocol/convert/kiro.rs` (emit `refusal: None`),
  plus mechanical `refusal: None` additions at every `MetadataUpdated { … }` construction site
  the compiler names (test literals in `session.rs`, `state.rs`, `client.rs`, destructure arms).
  Justification for >2 files: a named-field enum variant admits no default; the field addition
  is one atomic compilation unit. All non-event.rs/kiro.rs edits are one-line literal additions.

**Verification:** `cargo test` workspace-green; clippy/fmt clean.

## Slice 2b: parse the OR-condition (shapes a-d)

**Claim:**   design #1, #2, #3.
**Oracle:**  hand-written JSON fixtures transcribed from the carved tui.js contract
  (`findings.md` site 0) — independent of the parser under test.
**Stress fixture:** three-fixture matrix: (i) full refusal object + `stopReason:"end_turn"` →
  `Some` all fields (kills AND-instead-of-OR); (ii) no object + `stopReason:"CONTENT_FILTERED"`
  → `Some` all `None`; (iii) no object + `stopReason:"content_filtered"` (lowercase) → `None`
  (kills case-insensitive matching — the wire literal is exact).
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/convert/kiro.rs` (parse),
  `crates/cyril-core/src/protocol/convert/mod.rs` (fences
  `to_ext_notification_metadata_refusal_full`, `…_refusal_absent_unchanged`,
  `…_content_filtered_no_object`, `…_content_filtered_case_exact`).

**Verification:** unit tests; fixtures; clippy/fmt.

## Slice 3: robustness matrix + does-not-disturb + session scope

**Claim:**   design #4, #5, #6, #11.
**Oracle:**  fixture literals (shapes e-k from the design's input-shape table); CaptureWriter
  log capture (existing 1gim idiom) proving warns fire for corrupt shapes and the unknown-key
  log stays silent for `refusal`/`stopReason`.
**Stress fixture:** (i) `"refusal": 5` + `stopReason:"CONTENT_FILTERED"` → warn + `Some`(all
  `None`) — kills corrupt-aborts-frame AND corrupt-kills-stopReason-branch; (ii) full kitchen-sink
  frame (context % + metering + duration + tokens + effort + sessionId + full refusal) → every
  sibling field parses identically to the refusal-free control frame — kills
  parse-order/consumption bugs; (iii) `explanation: 42` → warn, that field `None`, others kept.
**Loop budget:** none (the existing unknown-key loop is untouched; refusal adds O(1) reads).
**Files:** `crates/cyril-core/src/protocol/convert/kiro.rs`,
  `crates/cyril-core/src/protocol/convert/mod.rs` (fences `…_refusal_partial_matrix`,
  `…_refusal_corrupt_object`, `…_refusal_corrupt_subfield`, `…_refusal_preserves_existing_fields`
  — the last updates the 1gim `_not_flagged` fence in place, keeping its no-unknown-key-log
  assertion),
  and `…_refusal_keeps_session_scope` for #11.

**Verification:** unit tests; fixtures; clippy/fmt.

## Slice 4: `SessionController` reconcile

**Claim:**   design #9.
**Oracle:**  SessionController field asserts after applying literal notification sequences.
**Stress fixture:** (i) refusal + `TurnCompleted(Cancelled)` → summary reads `Cancelled` (kills
  blanket override); (ii) turn 1 refusal + EndTurn → `Refusal`, then turn 2 no-refusal + EndTurn
  → `EndTurn` (kills flag leak — the buffered bool must be taken, not read); (iii) refusal +
  `TurnCompleted(Refusal)` → `Refusal` (idempotence).
**Loop budget:** none.
**Files:** `crates/cyril-core/src/session.rs` (field `pending_refusal: bool`, MetadataUpdated
  arm sets it when `refusal.is_some()`, TurnCompleted arm takes it; colocated tests).

**Verification:** unit tests; fixtures; clippy/fmt.

## Slice 5: `UiState` system message + per-turn dedupe + wording

**Claim:**   design #7, #8.
**Oracle:**  committed-message count and full-string equality against the spec strings in
  `design.md` (the design doc is the contract; tests transcribe it).
**Stress fixture:** (i) refusal, refusal, `TurnCompleted`, refusal → exactly 2 system messages
  (kills no-dedupe spam AND never-resets); (ii) `SessionCreated` mid-stream resets the flag;
  (iii) wording matrix: explanation-only / fallback-only / explanation+recommended /
  fallback+recommended — four exact strings asserted (kills format drift and
  recommendation-dropped); fallback string names `/model` and `/new`.
**Loop budget:** none (bool flag; `add_system_message` is the existing API).
**Files:** `crates/cyril-ui/src/state.rs` (MetadataUpdated arm consumes `refusal`, flag +
  reset in TurnCompleted and SessionCreated arms; colocated tests).

**Verification:** unit tests; fixtures; clippy/fmt.

## Slice 6: render fence

**Claim:**   design #12.
**Oracle:**  ratatui `TestBackend` buffer text extraction (render layer, independent of the
  state-machine asserts in slices 4-5).
**Stress fixture:** full lifecycle — refusal metadata frame → TurnCompleted(EndTurn) — rendered
  at a realistic viewport: buffer contains the explanation text in the chat area AND "Refused"
  in the toolbar row (kills committed-but-not-rendered, e.g. wrong message kind, and
  reconcile-not-reaching-toolbar).
**Loop budget:** none (test-only slice; no production code expected — system messages and the
  Refused chip already render; if a production gap surfaces, STOP per checkpointed-build drift
  rules).
**Files:** `crates/cyril-ui/src/state.rs` or `crates/cyril-ui/src/chrome_theme_tests.rs`
  (wherever the existing render-fence idiom for toolbar+chat lives — follow
  `chrome_theme_tests.rs:652`'s pattern), one file.

**Verification:** render test passes; clippy/fmt.

## Plan self-review

1. **Loops:** none introduced anywhere; all additions are O(1) per-frame field reads and a bool.
   No always-on phase touched → no wall budgets. No gaps.
2. **Fixtures:** every slice names its bug class (sentinel leak; missed construction site;
   AND-vs-OR + case-exactness; corrupt-aborts-frame + sibling-field disturbance; blanket
   override + flag leak; dedupe spam + format drift; committed-but-not-rendered). No
   happy-path-only fixtures. No gaps.
3. **Doc-comment preconditions:** one load-bearing contract — "RefusalAlert fields are never
   `Some(empty)`" — enforced at runtime by `from_parts` normalization (slice 1), not by
   `debug_assert!`. No other new preconditions. No gaps.
4. **Write targets:** all new output is `tracing::warn!`/`debug!` (diagnostic → log file);
   the system message is UI state, not a stream. No stdout writes. No gaps.
5. **Tracker references:** KAS surface → **cyril-ker1** (verified, filed this session);
   live-capture validation → **cyril-pz51** (verified, filed this session); ordering-race
   context → **cyril-9akh** (verified open, P3). No uncited deferrals. No gaps.

Claim coverage: #1→2b, #2→2a+2b, #3→2b, #4→3, #5→3, #6→3, #7→5, #8→5, #9→4, #10→1, #11→3,
#12→6. All 12 design claims covered.
