# cyril-0qe6 — budgeted plan

Implements `.cyril-0qe6/design.md` (approved 2026-08-11; all four open
decisions resolved: keep attach+status, include `k=v` inputs, manual fence on
claim 1 approved, suppression in `convert/kas.rs`). Wire facts revalidated on
kiro-cli 2.18.0 (2026-08-13).

Global notes:
- Every slice's verification = `cargo test -p <crate>` (touched crates),
  `cargo clippy --all-targets --features kas -- -D warnings` AND default
  features, `cargo fmt --check`. Real exit codes, no pipes.
- Oracle throughout = the committed live captures (real agent bytes), used
  as fixtures verbatim: `logs/kas-workflow-reattach-2.16.2.jsonl`,
  `kas-workflow-cancel-gateoff-2.16.2.jsonl`, + 2.18.0 twins. No hand-built
  reply JSON where a capture exists.
- New modules follow the staged-module pattern (`#[cfg(test)]` declaration
  until first production consumer) so every intermediate commit passes
  `-D warnings` on the lib target.
- Output streams: this is a TUI — all user output rides notifications into
  UiState messages (data); all diagnostics are `tracing` to cyril.log. No
  slice writes to stdout/stderr directly.
- Estimated total: ~1.6k changed lines incl. tests — under the ~4k PR gate.
- Loop-budget context: per-workspace runs ≈ dozens; recipes ≈ 10; commands
  list ≈ dozens; every new loop here is O(n) over one of those, n ≤ 10³,
  no always-on phases (all work is user-command-triggered) → all far under
  10⁶ ops; stated per slice anyway.

---

## Slice 1: Command-plane domain types + ref/input parsing

**Claim:** design C10 (run-ref mapping) + type substrate for all others.
**Oracle:** expected-value table transcribed from the engine's own ref
documentation (bundle strings, findings F4) — not from the implementation.
**Stress fixture:** ref-form table: bare `ralph` → `bundled://ralph`;
`bundled://x`/`generated://g` verbatim; `./wf/a.workflow.json` and
`wf/a.workflow.json` absolutized against workspace root; absolute path kept;
`a=b` as FIRST token is a REF (first token is always the ref, never an
input); `k=v=w` → value `v=w` (split_once); duplicate keys last-wins;
empty tail → `{}` inputs. Unicode recipe name `bundled://änderung`.
**Loop budget:** O(tokens) per invocation, tokens ≤ ~32 — trivial.
**Files:** `crates/cyril-core/src/types/workflow_command.rs` (new, staged),
`crates/cyril-core/src/types/mod.rs`.
**Contents (advisory):** `WorkflowOp` (7 variants), `WorkflowCommandOutcome`
(Recipes/Runs/Fetched/Launched/Cancelled/Resumed/Failed),
`WorkflowRecipe{name, description, source: Option}`,
`WorkflowRunSummary{workflow_id, name, status, created_at, updated_at,
started_at: Option, ended_at: Option, parent_session_id: Option}`,
`WorkflowFetchDisplay{workflow_id, name, status, nodes: Vec<NodeLine>}`,
`fn parse_run_ref(workspace_root, ref) -> RunTarget`,
`fn parse_inputs(tokens) -> serde_json::Map`. Statuses reuse
`WorkflowRunStatus` (already models the five observed values).
**Verification:** unit tests pass; ref/kv table green; budgets trivial;
doc-comment "first token is the ref" enforced by construction (parser
consumes it positionally — no runtime check needed, sanity covered by test).

## Slice 2: Snapshot-bearing reply parsers (`inspect`, `new`)

**Claim:** design C4 (parse half): an `inspect`/`new` reply parses to
`WorkflowSnapshot`; outer/inner workflowId mismatch is an error.
**Oracle:** captured live reply bytes (reattach capture id=5/id=4 frames,
new-reply frames) embedded as fixture files.
**Stress fixture:** (a) the real inspect reply fixture; (b) same bytes with
outer `workflowId` edited to mismatch `state.workflowId` → must error, not
pick one silently; (c) `state.root` removed → error naming the field.
**Loop budget:** O(nodes) per reply, nodes ≤ ~100 per DAG — trivial.
**Files:** `crates/cyril-core/src/protocol/convert/kas/workflow.rs`,
`crates/cyril-core/tests/fixtures/` (new fixture JSONs extracted from the
captures).
**Contents (advisory):** `pub(crate) fn parse_state_reply(&Value) ->
Result<WorkflowSnapshot, WorkflowAdapterError>` reusing `WireSnapshot::
try_into_domain` + the existing mismatch error variant.
**Verification:** unit tests incl. both adversarial fixtures; oracle bytes
untouched from capture.

## Slice 3: Summary reply parsers (`list`, `listRecipes`, `cancel`, `invoke`/`resume`)

**Claim:** design C12 (per-entry status tolerance) + C13 (cancel shape) +
list/recipes shapes.
**Oracle:** captured list/recipes/cancel reply bytes (2.16.2 + 2.18.0
captures agree).
**Stress fixture:** (a) real 2-run list fixture; (b) fixture with one
`"status": "quantum"` entry among two good → 2 parsed + 1 warned/skipped,
NOT a whole-Vec serde failure; (c) list entry missing `startedAt` (real
never-invoked capture bytes) → `Option::None`, no default-sentinel;
(d) cancel reply parsed as `{ok, previous_status}` — parsing it with the
invoke-shape struct must fail the fixture (guards against struct reuse);
(e) empty `{"runs": []}` (real capture bytes).
**Loop budget:** O(entries) per reply, entries ≤ ~100 — trivial.
**Files:** same two as slice 2 (parsers colocate; fixtures extend).
**Verification:** unit tests incl. tolerance + absence cases; warn path
asserted via the parser returning per-entry results (log-before-skip rule).

## Slice 4: Bridge command + notification variants

**Claim:** substrate for C3/C4 — `BridgeCommand::Workflow{session_id,
workspace_paths, op}`, `Notification::WorkflowSnapshot(Box<_>)`,
`Notification::WorkflowCommand(Box<_>)`; boxed variants keep enum sizes flat.
**Oracle:** the existing `size_of` fences in `event.rs` (independent of new
code — they existed before this feature).
**Stress fixture:** size assertions: adding the variants must not grow
`Notification`/`BridgeCommand` beyond their current boxed ceiling (event.rs
already asserts this pattern for `Workflow`).
**Loop budget:** none (types only) — paired with slice 5 for logic; fixture
here is the size fence, which is a real bug class (unboxed payload bloats
every channel message).
**Files:** `crates/cyril-core/src/types/event.rs`,
`crates/cyril-core/src/types/workflow_command.rs` (unstage → production).
**Verification:** size fences pass; `-D warnings` on lib target (staging
flip correct).

## Slice 5: Bridge pure outcome mapping + read-op arms

**Claim:** design C3 (every path notifies) for ListRecipes/ListRuns/Attach/
Status; C2 (workspacePaths always sent).
**Oracle:** mpsc receiver contents in tests (channel truth, not logs); built
request params serialized and asserted against the capture's request shape.
**Stress fixture:** for one op, drive all paths through the pure mapper:
ok / agent-error(with details) / agent-error(no details) / malformed-reply →
exactly the expected notification multiset each time, never zero (the named
bug: today's `ExtMethod` log-and-continue). Params builder: `list` params
must contain `workspacePaths == [root]` — omitting is the live `-32603`.
**Loop budget:** O(1) per op dispatch — trivial.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (helper fns + arm;
`#[cfg(not(feature = "kas"))]` arm answers `BridgeError` like hooks).
**Contents (advisory):** mirror hooks: pure
`fn workflow_outcome_notifications(op, Result<Parsed, OpErr>) ->
Vec<Notification>` (snapshot-first ordering for Fetched) + thin async glue
`send_workflow_outcome`.
**Verification:** per-path unit tests (distinct asserts per claim); clippy
both feature sets.

## Slice 6: Bridge mutating ops (`run`, `cancel`, `resume`) + error-detail extraction

**Claim:** design C6 (`new`→`invoke` sequencing, no gate settings anywhere) +
C7 (`data.details` surfaces verbatim).
**Oracle:** captured live-owner refusal frame (reattach round-1 capture,
error id=8) as the C7 fixture; recorded outgoing-frame log in the test
harness for C6.
**Stress fixture:** (a) `new` returns error → zero `invoke` frames recorded
(named bug: unconditional invoke); (b) `new` ok → snapshot notification
emitted BEFORE `invoke` is sent (ordering); (c) the real refusal error frame
→ rendered message contains "running in another process (owner pid" — a
message of just "Internal error" fails; (d) error without `data` → falls
back to `message`; (e) grep-test: the initialize/session-new param builders
contain no `"workflows"` key (named bug: gate flipped during debugging and
left in).
**Loop budget:** O(1) per op — trivial.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`,
`crates/cyril-core/src/protocol/kas/settings.rs` (test only, no prod change).
**Verification:** all five fixtures green; the C7 formatter is a pure fn.

## Slice 7: App wiring — snapshot consumed exactly once + attach seeding fence

**Claim:** design C4 (end): an inspect reply seeds the tracker and
`/workflow status` sees it; C5: terminal-conflict surfaces and changes
nothing.
**Oracle:** fixture = capture bytes through the full parse→apply chain;
tracker state asserted (not the parser's own return).
**Stress fixture:** (a) parse real inspect fixture → `apply_snapshot` →
`tracker.get(wid)` Some with status `completed` and node count matching the
capture's tree; (b) seed tracker terminal `completed`, apply a `failed`
snapshot of the same id → `Err(TerminalSnapshotConflict)`, tracker
unchanged (named bug: ignoring `apply_snapshot`'s Err); (c)
`Notification::WorkflowSnapshot` is NOT forwarded to SessionController/
UiState (routing assert, mirroring the existing `Notification::Workflow`
exactly-once test if present — else add the same shape).
**Loop budget:** O(nodes) apply — existing tracker budgets already fence
this (cyril-6beh).
**Files:** `crates/cyril/src/app.rs`,
`crates/cyril-core/tests/workflow_attach_seeding.rs` (new).
**Verification:** integration test green; App arm warns (never panics) on
apply error.

## Slice 8: CommandContext gains `workflow_tracker`

**Claim:** substrate for C14 — commands can read run state without a wire
call.
**Oracle:** compile + existing command tests (they pass `None`, unchanged
semantics).
**Stress fixture:** a test command reading `ctx.workflow_tracker` sees the
run seeded in slice 7's fixture; all existing command tests still compile
with `workflow_tracker: None` (the named bug: breaking every existing test
site instead of defaulting).
**Loop budget:** none (plumbing).  Paired fixture is the compile matrix +
one read-through test — plumbing with a behavioral assert, not a bare type
change.
**Files:** `crates/cyril-core/src/commands/mod.rs`, `crates/cyril/src/app.rs`
(construction site).
**Verification:** `cargo test -p cyril-core` + `-p cyril` green both
feature sets.

## Slice 9: `/workflow` command — arg parsing + dispatch

**Claim:** design C14 (no-arg status = tracker-only, zero sends) + the
subcommand/arg input matrix; C6 front (run maps ref+inputs into one bridge
op).
**Oracle:** mock `BridgeSender`'s recorded sends (channel contents).
**Stress fixture:** matrix: no subcommand / unknown subcommand → usage
message, zero sends; `attach`/`cancel`/`resume` without id → usage, zero
sends; no active session → message, zero sends; `status` no-arg with empty
tracker → "no runs known" message, zero sends (named bug: status always
round-trips); `status` no-arg with seeded tracker → summary lists the run,
zero sends; `run ralph k=v` → exactly one `BridgeCommand::Workflow` with
`Run{reference: bundled://ralph, inputs:{k:"v"}}`.
**Loop budget:** O(runs) for the status summary, runs ≤ dozens — trivial.
**Files:** `crates/cyril-core/src/commands/workflow.rs` (new),
`crates/cyril-core/src/commands/mod.rs` (module decl only).
**Verification:** matrix tests green; usage text names all seven
subcommands.

## Slice 10: Registration gating + feature matrix

**Claim:** design C11: `/workflow` registers only under the KAS engine;
default-feature builds have no `/workflow` and the bridge arm answers
`BridgeError`.
**Oracle:** the compiled test matrix itself (`cargo test` with and without
`--features kas`) — the compiler is the oracle.
**Stress fixture:** registry built for v2 engine → `parse("/workflow …")`
is None; for KAS engine → Some; feature-off bridge arm test asserts the
`BridgeError` notification (named bug: unconditional registration gives v2
users a dead command; silent arm gives a hung App).
**Loop budget:** none — fixture is the registration matrix.
**Files:** `crates/cyril-core/src/commands/mod.rs` (registration source
param, mirroring `HooksCommandSource`), `crates/cyril/src/app.rs` (pass
engine).
**Verification:** both feature-set test runs green (AC6); registry tests
per engine.

## Slice 11: Autocomplete suppression of the four gate commands

**Claim:** design C8: exactly `workflow-run|workflow-status|workflow-cancel|
workflow-resume` are dropped from KAS available-commands; nothing else is.
**Oracle:** fixture list contents vs output set (set difference computed in
the test, not by the filter under test).
**Stress fixture:** available-commands containing all four + `workflow-creator`
+ `steer` + unicode-named command → output keeps the latter three verbatim
(named bug: prefix-match `workflow-` eats `workflow-creator`); empty list →
empty; list without any of the four → identity.
**Loop budget:** O(commands) per update, ≤ dozens — trivial.
**Files:** `crates/cyril-core/src/protocol/convert/kas.rs`.
**Verification:** exact-set tests green; debug log on each suppression
(log-before-drop rule).

## Slice 12: UiState formatters (staged) — recipes, runs, outcomes

**Claim:** design C9 (recipes render name+source; 7-bundled fixture) + every
`WorkflowCommandOutcome` variant renders (Failed shows the verbatim details
— C7's display half).
**Oracle:** the capture-derived fixtures (recipe names from the live
listRecipes reply; the refusal text from the live error frame).
**Stress fixture:** (a) 7-bundled fixture → all 7 names + descriptions
present, count line correct; (b) workspace recipe renders its absolute
source path (named bug: rendering only names); (c) runs table with absent
`startedAt`/`endedAt` renders `—` not `0`/empty-shift (named bug: sentinel
formatting); (d) empty runs → "no runs" line; (e) Failed{details:
refusal-text} renders the pid-bearing sentence verbatim; (f) unicode recipe
description round-trips.
**Loop budget:** O(entries) per render, ≤ ~100 — trivial; rendering is
event-driven, not per-frame.
**Files:** `crates/cyril-ui/src/workflow_format.rs` (new, staged),
`crates/cyril-ui/src/lib.rs` (decl).
**Verification:** formatter unit tests green.

## Slice 13: UiState arm — outcomes become system messages

**Claim:** `Notification::WorkflowCommand` renders exactly one system
message per outcome; `WorkflowSnapshot` never reaches UiState (C4/C5
routing complement).
**Oracle:** UiState message list contents after `apply_notification`.
**Stress fixture:** one notification per outcome variant → exactly one
message each, containing its formatter output; a `WorkflowSnapshot`
notification passed to UiState (defensively) → no message, no panic, warn
(named bug: double-consuming the snapshot as both state and message).
**Loop budget:** O(1) per notification.
**Files:** `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/
workflow_format.rs` (unstage).
**Verification:** state tests green; `apply_notification` returns `true`
(dirty) for rendered outcomes.

## Slice 14: Live AC sweep (verification only, no code)

**Claim:** AC1 — all seven subcommands work against a live gate-off KAS
session (now 2.18.0).
**Oracle:** the live agent + the run directory on disk (findings F1 layout),
same as the probes.
**Stress fixture:** scripted sequence against a temp workspace:
`recipes` (expect 7+), `list` (empty), `run` an input-free one-step inline…
— run uses a workspace `.workflow.json` (validates the path-ref branch),
`status <id>`, `cancel`, `resume` on a killed-tree run (reattach scenario),
`status` no-arg. Driven through cyril itself via the pty harness pattern
(`.cyril-a14l/oracle-pty.py` as the template) — cyril's real binary, real
key input, screen scraped; NOT a re-run of the python wire probes (those
bypass the code under test).
**Loop budget:** n/a (manual-scripted verification).
**Files:** `.cyril-0qe6/live-sweep.md` (results record; script under
`.cyril-0qe6/` if reusable).
**Verification:** every AC line item recorded pass/fail with screen
evidence; failures stop the pipeline (drift rule).

---

## Plan Self-Review

1. **Loops:** ref/token parse O(tokens≤32); reply parsers O(entries≤100) /
   O(nodes≤100); suppression filter O(commands≤dozens); status summary
   O(runs≤dozens); formatters O(entries≤100). No always-on phases; all
   user-triggered; every product ≪ 10⁶. No unstated loops.
2. **Fixtures:** every slice names its bug class: struct-reuse (S3d),
   whole-Vec serde death (S3b), sentinel formatting (S12c), prefix-match
   over-suppression (S11), unconditional-invoke (S6a), log-and-continue
   (S5), ignored apply error (S7b), status-always-round-trips (S9),
   double-consumed snapshot (S13), id-mismatch pick-one (S2b), staging/size
   regressions (S4). No happy-path-only fixture.
3. **Doc-comment preconditions:** "first token is the ref" — enforced by
   construction (positional parse), sanity-tested S1. "workspacePaths
   non-empty" — load-bearing (live `-32603`): enforced by construction
   (built from the command's `workspace_root: PathBuf`, never caller-
   supplied), asserted in S5's params test. "snapshot before display
   message" — ordering asserted S6b. No unenforced contract lines planned.
4. **Write targets:** notifications → UiState messages (data); `tracing`
   (diagnostic, cyril.log). No stdout/stderr writes anywhere (TUI).
5. **Tracker references:** cyril-zd8u (rendering), cyril-oieu (command
   dispatch gap), cyril-2ibk (extended verbs + typed inputs), cyril-z4eo
   (approvals queue), cyril-taba (token refresh) — all verified open in
   `rivets` this run. Paths-with-spaces in `run` refs: settled rationale
   (token-based args; refs never contain spaces; documented in usage text) —
   no deferred work.

Claim coverage: C1 probes (done, manual fence approved) · C2 S5 · C3 S5/S6 ·
C4 S2+S7 · C5 S7 · C6 S6 · C7 S6+S12 · C8 S11 · C9 S12 · C10 S1 · C11 S10 ·
C12 S3 · C13 S3 · C14 S9 — all 14 covered.
