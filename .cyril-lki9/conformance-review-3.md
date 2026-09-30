# cyril-lki9 conformance review 3 (scoped re-review)

## Scope

- Base: `bfc498b1`
- Branch: `fix/cyril-lki9-i2-labels` (HEAD `b52650d8`), worktree
  `/home/dwalleck/repos/cyril-wt-fix-cyril-lki9-workflow-auto-wake`, including
  uncommitted working-tree changes.
- Affected modules (re-reviewed from source in an isolated context):
  - `crates/cyril-core/src/types/workflow.rs`
  - `crates/cyril-core/src/workflow.rs` (WorkflowTracker wake-label API + state)
  - `crates/cyril-core/src/protocol/turn_mediator.rs` (`mediate`, `announce`, crate surface)
- Retained record: `.cyril-lki9/conformance-review-2.md` for every other module,
  subject to the retention check in Phase 2.
- Method: production code only (outside `#[cfg(test)]`); `git diff bfc498b1`
  and `git diff HEAD` per file; no cargo, no kiro-cli, no repo edits.

## Phase 1 — blind reconstruction (written before opening `.cyril-lki9/`)

### Change census

`git diff --stat bfc498b1 -- crates/` touches 23 files (+3025/-85). Uncommitted
vs HEAD: 7 crate files (`convert/kas.rs`, `turn_mediator.rs`, `workflow.rs`,
`cyril-ui/lib.rs`, `cyril-ui/state.rs`, `cyril-ui/turn_labels.rs`,
`cyril/app.rs`). `types/workflow.rs` has NO uncommitted change (its diff is
entirely in committed history; mtime 2026-09-29 23:41, before HEAD 00:45).

mtimes of uncommitted crate files: `lib.rs` 00:56, `app.rs` 01:04:11,
`kas.rs` 01:04:31, `state.rs` 01:04:50, `turn_labels.rs` 01:05:03, then a gap,
then `workflow.rs` 01:14:36 and `turn_mediator.rs` 01:15:16. Only the latter two
plausibly carry the post-review fixes; this is cross-checked against the prior
review's timestamp in Phase 2. `app.rs` (01:04) already calls
`claim_wake_labels`/`take_wake_label`/`take_injection_label`, so the
claim-at-turn-start API predates the fix window; the fix-window edits in
`workflow.rs`/`turn_mediator.rs` are consistent with "debug logs + tests" (plus
the helper/visibility tidy noted below).

### `crates/cyril-core/src/types/workflow.rs`

- Interface added: `WorkflowSnapshotMetadata::with_run_label(String) -> Self`,
  `WorkflowSnapshotMetadata::run_label() -> Option<&str>`,
  `WorkflowSnapshot::run_label() -> Option<&str>`; crate-private
  `WorkflowSnapshotParts::run_label`, threaded by `into_parts`.
- Responsibility: domain carrier for KAS `finalState.runLabel` (pure data;
  builder + getters, `Option` for absence — no sentinel).
- Dependency direction: none new (core type, no ACP, no UI).
- Pass-through / hypothetical seam: none; the field is consumed by
  `workflow.rs::canonicalize_snapshot` → `WorkflowRun::run_label` →
  `label_for_run`. (Populated by the KAS converter — outside this scope.)
- Defects: none found. No wire literals.

### `crates/cyril-core/src/workflow.rs`

- Interface added/altered:
  - `WorkflowRun::run_label() -> Option<&str>` (+ private field).
  - `pub struct WakeLabel { name, status }` with `new`, `name()`, `status()`.
  - `WorkflowTracker` state: `wake_labels: HashMap<SessionId, VecDeque<WorkflowId>>`
    (unclaimed terminal completions per parent session, arrival order) and
    `claimed_wake_labels: HashMap<SessionId, Vec<WorkflowId>>` (completions
    claimed by the session's current turn at its start).
  - `claim_wake_labels(&mut self, &SessionId)` — at a turn start: moves the
    whole queue into the claim, replacing (discarding) any earlier unnamed claim.
  - `take_wake_label(&mut self, &SessionId) -> Option<WakeLabel>` — header
    label: the NEWEST resolvable claimed completion; consumes the claim;
    **debug-logs `"workflow wake with no known run; header left nameless"` on
    `None`** (fix-window edit, spec B4 per its doc comment).
  - `take_injection_label(&mut self, &SessionId) -> Option<WakeLabel>` — the
    newest unclaimed completion since the turn started (pop_back), consuming it.
  - private `wake_label`, free fn `label_for_run` (precedence
    `runLabel → workflowName → workflowId`, empty strings treated as absent;
    debug-logs a status-less run).
  - `apply_completion` path: enqueues `snapshot_id` under the parent only on a
    changed, terminal-status completion with a parent session (duplicates and
    `paused` never enqueue).
- Semantics are claim-at-turn-start / newest-wins, NOT FIFO/oldest (HEAD had
  `take_wake_label` = `pop_front` oldest-first; the working tree replaced it).
- Responsibility cluster: workflow-run state owner now also owns the per-session
  "which completion names this turn" correlation. Cohesive (it reads its own
  `runs`), no UI text.
- Dependency direction: core-only; caller is `App::annotate_transcript`
  (`TurnStarted` → claim; `AgentInitiatedTurn` with workflow-completion origin →
  `take_wake_label`; workflow `EngineMessageInjected` → `take_injection_label`).
  The UI receives only `Option<&WakeLabel>`.
- Pass-through / seam: `WakeLabel::new` is public "so presentation code can be
  exercised" — a test-motivated public constructor (non-blocking).
- Defects / observations:
  - O1 (non-blocking): `take_wake_label`'s B4 log is correct only by caller
    convention ("only workflow-completion wakes ask"); the tracker cannot see
    the origin. Today the single production caller honours it.
  - O2 (non-blocking): `take_injection_label` returns `None` silently when the
    session has no queue, and both `wake_label`/`take_injection_label` skip an
    id missing from `runs` without a log (HEAD logged "evicted run skipped").
    `runs` is never evicted in production (`grep` finds no remove/retain/clear),
    so the path is unreachable today; the mid-turn notice then reads
    "workflow ended". Whether an injection counts as a B4 "wake" is a spec
    question for Phase 2.
  - O3 (non-blocking): `wake_labels` entries accumulate until the next turn
    start on that session; `claimed_wake_labels` keeps one Vec per session until
    consumed/replaced. Both are bounded by sessions × runs, and `runs` itself is
    already unbounded — no new growth class, but nothing prunes a session that
    never starts another turn.
  - O4 (test-only, cosmetic): the C13 doc comment now sits above
    `nameless_wake_label_is_logged` (B4 test) rather than
    `wake_labels_claim_at_turn_start`, which has no doc comment.
- Literals: only `notify-wf` inside doc comments (lines 455, 879); no string
  literals from the KAS dialect or header text in code.

### `crates/cyril-core/src/protocol/turn_mediator.rs`

- Interface added/altered (crate-private throughout):
  - `Disposition::BeginServerTurn`.
  - `ActiveTurn { origin: TurnOrigin, bracket_open: bool }`, private
    `enum TurnOrigin { Dispatched, Server }`.
  - `TurnMediator.announced: HashSet<SessionId>` (cleared per session at
    `TurnStarted` and `TurnCompleted`).
  - `observe(&mut self, &RoutedNotification, main: Option<&SessionId>)` —
    signature gained `main`; `TurnStarted` routed to private
    `observe_turn_start` (main+idle → server turn; dispatched unbracketed →
    attach; otherwise forward, warn on nested start); a server turn's wire
    `turn_end` releases without registering a synthesized companion and without
    clearing an owed one.
  - `mediate(&mut self, RoutedNotification, main) -> Mediated` — the bridge's
    single per-frame decision: disposition + ordered forward list
    (announcement first, then the frame; empty on Absorb/DropStale/DropUnowned).
  - `pub(crate) struct Mediated { disposition, forward }`.
  - `announce` — now **private** (was `pub(crate)` at HEAD); on the first
    origin-tagged frame per session per turn returns `true` and, when
    `!origin.is_workflow_completion()`, **debug-logs
    `"agent-initiated turn with a non-workflow reason; generic header"` with
    `reason` (or `"unspecified"`)** (fix-window edit, spec B3 per its comment).
- Responsibility cluster: turn ownership (dispatched and server-started),
  terminal pairing, and once-per-turn agent-initiated announcement. The
  B3 log keys on the same `is_workflow_completion()` bool that
  `cyril-ui/turn_labels.rs::header_text` uses to choose the generic header, so
  "logged" ⇔ "generic header rendered" (for announced turns).
- Dependency direction: core-internal; the only production caller of `mediate`
  is `domain_mediator/inbound.rs` (unchanged since HEAD); `test_support.rs`
  replays captures through the same `mediate`.
- Pass-through / seam: `observe` remains `pub(crate)` although its only
  non-test caller is `mediate` in the same module (O5, non-blocking: could be
  private like `announce`).
- Defects: none blocking. `observe_turn_start` logs every non-owning arm that
  signals something unexpected (no-scope debug, exhaustion warn, nested warn).
  `announced` bounded by sessions with an agent-initiated turn in flight.
- Literals: `turn_start` appears only in doc comments/identifiers;
  `workflow-complete-wake` / `send-message-wake` only inside `#[cfg(test)]`.

### Literal sweep (production, outside owners)

No KAS wire literal (`"turn_start"`, `notify-`, `agentInitiated`,
`[notification/`, `workflow-complete-wake`) and no header text (`⚙`,
`agent follow-up`, `agent-initiated ·`) appears as a string literal in
production code of the three affected modules. Remaining hits repo-wide are doc
comments (`types/event.rs`, `engine.rs`, `state.rs`, `app.rs`), test code, and
JSONL fixtures — unchanged by the fixes.

## Phase 2 — comparison (read after Phase 1 was written)

Read: `.cyril-lki9/design.md` (whole file, incl. "Ledger amendments", "Spec
rule change", "Second isolated conformance review … resolved"),
`.cyril-lki9/spec.md` (B3, B4, Decisions), `.cyril-lki9/conformance-review-2.md`.
Executed read-only: `python3 .cyril-lki9/oracles/shape.py` →
`C21 PASS (… {'crates/cyril/src/app.rs': 58, 'crates/cyril-ui/src/state.rs': 71})`.

### N1 status — RESOLVED

- `types/workflow.rs` now has exactly one module-ledger row (design.md
  "types/workflow.rs (amendment N1, 2026-09-30)"): interface
  `run_label: Option<String>` on `WorkflowSnapshotMetadata` + `with_run_label`,
  `run_label()`, `WorkflowSnapshot::run_label()`, `WorkflowSnapshotParts.run_label`;
  owns "carry the parsed runLabel … (domain field only)"; must not own "parsing;
  naming precedence". This is item-for-item the Phase 1 reconstruction (builder +
  two getters + crate-private parts field; no parsing, no precedence — precedence
  lives in `workflow.rs::label_for_run`). The only other mention is the placement
  row's "(… carried by `types/workflow.rs`)", which is a placement cell, not a
  second ledger row.
- FIFO/oldest wording for wake labels is gone from every governing location:
  - Route/inputs Decisions summary: "claim-at-turn-start naming per session —
    amended 2026-09-30, was FIFO".
  - S21: "the turn claims all; the header names the newest; the rest are discarded".
  - Placement row: "Completions claimed per parent session at turn start
    (amended …; was oldest-unconsumed)" → `claim_wake_labels, take_wake_label,
    take_injection_label`.
  - `workflow.rs` ledger row: all three methods; "per-parent completion queue,
    turn-start claim, consumption".
  - App protected-parent row: "call `claim_wake_labels` / `take_wake_label` /
    `take_injection_label` … (amended 2026-09-30)"; App ledger row has no ordering text.
  - C13 claim + falsification row: NEWEST claimed / newest unclaimed; M13 = "take
    the OLDEST claimed" (now a genuine mutation), M13b = drop the App claim.
  - Remaining "FIFO" hits are legitimately unrelated or historical: operator-steer
    FIFO (S16, removed invariant 6, C6/M6) and Alternatives C, whose heading now
    states the rule was amended and the placement choice is unchanged.
- The ledger now matches the implemented claim-at-turn-start API (Phase 1:
  `claim_wake_labels` replaces the claim, `take_wake_label` = newest resolvable
  claimed and consumes the claim, `take_injection_label` = `pop_back` on the
  post-claim queue).

### N2 status — RESOLVED

- **B3** ("a debug log records the reason"): `TurnMediator::announce`
  (`turn_mediator.rs`, production region) emits `tracing::debug!(session, reason
  = origin.reason().unwrap_or("unspecified"), "agent-initiated turn with a
  non-workflow reason; generic header")` on the first announcement of a turn
  whose origin is not a workflow completion. It keys on the same
  `is_workflow_completion()` bool that `turn_labels::header_text` uses to pick
  the generic header, so it fires exactly for B3 headers (incl. absent reason →
  `unspecified`), once per announced turn. Owner: the mediator owns the
  once-per-turn announcement decision (ledger row + M3); this is the placement
  the design's N2 disposition names. Fence `generic_wake_reason_is_logged`
  (2 tagged frames → 1 log; workflow-completion wake → 0).
- **B4** ("a debug log records the missing correlation"): `WorkflowTracker::
  take_wake_label` emits `tracing::debug!(session, "workflow wake with no known
  run; header left nameless")` whenever it returns `None` (no claim, empty claim,
  or no resolvable claimed run). The tracker owns the correlation decision (C13
  row), so this is the owning module; it also closes the prior review's CLAUDE.md
  "log before returning None" note. Fence `nameless_wake_label_is_logged`
  (nameless → log; resolved → no log).
- Literal rules: neither log uses a KAS wire literal or header text; the reason
  is the converter-normalized `AgentInitiation::reason()`. The B3 test's
  `send-message-wake` / `workflow-complete-wake` strings are inside
  `#[cfg(test)]` (line ≥ 553).

### Row mapping (affected modules)

| Module | Amended ledger row | Verdict |
|---|---|---|
| `types/workflow.rs` | `types/workflow.rs` (amendment N1) | MATCH — exactly one row |
| `workflow.rs` | `workflow.rs` row (amended 2026-09-30) + C13 + M5 (`WakeLabel`) + N2 disposition (B4 log) | MATCH — exactly one row |
| `turn_mediator.rs` | `protocol/turn_mediator.rs` row + M3 amendment (`mediate`/`Mediated`, `observe` takes `main`, `announce` private) + N2 disposition (B3 log) | MATCH — exactly one row |

### Retention statement — HOLDS

The prior review's Phase 1 (P1–P15) and Phase 2 MATCH verdicts for every other
module still apply:

1. `git diff --stat bfc498b1 -- crates/` shows the same 23-file set the prior
   review enumerated (P1–P15 + test-only files).
2. The prior review already described, as present, every non-log change in the
   three affected files: `announce` private, `observe(routed, main)`, `mediate`,
   the claim API, `label_for_run` with its status-less log, and "take_wake_label
   returns None silently" / "every tracing:: call the diff adds was checked".
   The only production deltas since then are the two N2 debug logs (plus
   doc-comment wording on `take_wake_label`) and their tests.
3. mtimes: all other uncommitted crate files (`app.rs`, `kas.rs`, `state.rs`,
   `turn_labels.rs`, `lib.rs`) were last written ≤ 01:05:03; the fix window
   touched only `workflow.rs` (01:14:36) and `turn_mediator.rs` (01:15:16).
   `types/workflow.rs` has no uncommitted change at all (N1 was ledger-only).
4. Protected-parent production deltas are byte-count identical to the prior
   review's hand count and oracle run (app.rs +58, state.rs +71), and the C21
   literal census still passes.

### Mismatches

None blocking. No new mismatch found.

## Non-blocking observations

- R1 (ledger precision): the `workflow.rs` row writes
  `{claim_wake_labels(..), take_wake_label(..), take_injection_label(..)} -> Option<WakeLabel>`;
  `claim_wake_labels` returns `()`. The row also leaves `WorkflowRun::run_label()`
  (pub getter) implicit. Disposition: optional doc touch-up; no approval needed
  beyond the record.
- R2 (ledger precision): the `turn_mediator.rs` row body still lists
  `announce(&RoutedNotification) -> bool` as interface; M3 amends it (announce is
  private, `mediate`/`Mediated` is the interface). Governed by the amendment;
  folding M3 into the row text would remove the need to read both.
- R3 (caller convention): `take_wake_label`'s B4 log is accurate only because its
  single production caller (`App::annotate_transcript`) asks solely for
  workflow-completion wakes; the doc comment states this. A future non-wake
  caller would log spuriously.
- R4: `take_injection_label` returns `None` without a log when the session has
  no queue, and neither it nor `wake_label` logs an id missing from `runs` (HEAD
  logged "evicted run skipped"). Not a spec clause (B5's nameless notice has no
  log requirement) and `runs` is never evicted in production, so unreachable
  today.
- R5 (test-only, cosmetic): the C13 doc paragraph now heads the B4 test
  `nameless_wake_label_is_logged`; `wake_labels_claim_at_turn_start` lost its doc.
- Carried from review 2 unchanged: O1 injection naming LIFO, O2 severity from
  prefix, O3 bounds housekeeping (empty `claimed_wake_labels` entries, empty
  `VecDeque`s, `announced` for never-ended turns), O4 `observe` could be private,
  O5 duplicate `add_system_message` bodies, O6 test-motivated `WakeLabel::new`
  (doc still says built "via `take_wake_label`" only), O7–O9.

RESULT: PASS
