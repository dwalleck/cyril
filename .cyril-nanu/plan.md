# Plan: cyril-nanu — non-blocking usage-panel refresh

Design verified 2026-08-28: Falsification table complete with no empty cells, C10 `PASS`, no row `FAIL`, every other row `PENDING — checkpointed-build, per-slice gate`, Approval carries the requester's verbatim "yes" and `Risk acceptances: None`.

## Partition arithmetic

| Slice | Diff estimate |
|---|---|
| 1 — atomic snapshot | 120 |
| 2 — snapshot worker | 380 |
| 3 — panel refresh state | 240 |
| 4 — App wiring | 400 |
| **Sum** | **1,140** |

Churn margin: **40% (456 lines)**, giving **1,596**. Rationale for 40%: the preceding change on this subsystem (cyril-9kyk) grew ~600 lines beyond its ~780-line original during review remediation — about 77% — but that pipeline had no pre-PR review stage, which this one runs at step 3 and which front-loads most of that churn into the slices themselves. 40% is the measured drift halved, not a round number.

1,596 is at or below the contract's 4,000-line review-size gate, so the plan has **a single PR increment**.

**Increment A — "non-blocking usage refresh"**: slices 1–4. Mergeable definition: merges to the repository's default branch (`git symbolic-ref refs/remotes/origin/HEAD`) on its own; it changes no public behavior outside the `/usage` panel and leaves every existing usage fence passing. Verified without any later increment because there is none.

---

## Slice 1: `UsageLog::snapshot()` observes one point in time

**Claim IDs:** C2, C10
**Expected behavior:** With turns being appended concurrently, one `snapshot()` call returns rollups that reconcile: the Overview turn count equals the sum of per-provider counts and the sum of per-model counts. Today those can differ once the call is not serialized by the event loop.
**Oracle:** Three independently-computed rollups reconciled against each other — the snapshot's own Overview, provider and model aggregates come from three separate SQL statements, so agreement cannot come from a shared computation. Backed by `probe_wal.py` / `oracle_wal.sh`, which established WAL isolation through two mechanisms that share no code with cyril.
**Stress fixture:** A writer thread committing turns in a tight loop for the whole duration of a snapshot over a 5,000-row log spanning 3 providers and 6 models, so a commit lands between rollups with near-certainty rather than by luck; expected outcome written now — Overview count == Σ provider counts == Σ model counts, and the writer's inserts all return `Ok`.
**Regression fence:** `crates/cyril-core/src/usage.rs` → `usage::tests::snapshot_is_atomic_under_concurrent_append` (created in this slice)
**Named mutation:** In `usage.rs`, remove the `unchecked_transaction()` wrapper and run the rollups on the bare connection. Expected red: Overview count exceeds the summed per-model counts by the number of mid-snapshot commits.
**Complexity/production scale:** `N/A — no new loop; this slice wraps the existing nine rollup queries in one transaction and adds no iteration.`
**Wall budget/phase:** always-on — every snapshot pays it. Budget: the existing `kiro_snapshot_100k_budget_reference` 2 s bound at 100,000 rows still holds, and the measured whole-snapshot time stays inside the run-to-run spread recorded before the change (689 / 696 / 757 ms on this workstation, 2026-08-28). Rationale — **revised at the plan critique**: the field first read "≤ 5 ms added at 100,000 rows", which is not measurable. A `BEGIN`/`COMMIT` pair genuinely is constant-time, but run-to-run variance on this fixture is ±40 ms, so a 5 ms delta is below the noise floor and any "PASS" against it would have been an eyeball dressed as a measurement. The checkable claim is that the transaction introduces no regression distinguishable from noise against a bound that already exists.
**Files:** `crates/cyril-core/src/usage.rs`
**Estimate:** 1–2 hours
**Diff estimate:** 120 (impl ~40, test ~80)
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-core --lib snapshot_is_atomic` → the fence passes: the three counts agree item-by-item, and the concurrent writer's inserts all returned `Ok` (proving commits really landed mid-snapshot rather than the test being vacuous).
- Apply the named mutation, re-run the same command → the fence goes red on the count-equality assertion specifically, naming the Overview/model discrepancy; restore and it is green again.
- `cargo test -p cyril-core --lib usage::` → every pre-existing usage test still passes, unchanged.

---

## Slice 2: a snapshot worker that owns a read-only connection and coalesces bursts

**Claim IDs:** C3, C7
**Expected behavior:** `spawn_usage_snapshot_worker(path)` returns a request handle and a result receiver. Requests arriving while a snapshot runs collapse: N requests during one in-flight snapshot produce exactly one further snapshot for any N ≥ 1, and none for N = 0. The worker's connection never advances the schema version.
**Oracle:** For coalescing — a counting fake incremented on the worker's own thread, compared against the hand-derived table (N=0→1 execution, N=1→2, N=10→2). For migration — a `PRAGMA user_version` read taken on a third, independent connection, not through any worker code.
**Stress fixture:** A burst of 10 requests delivered while a snapshot is held mid-execution on a barrier, plus the N=0 and N=1 cases in the same test; and, for C7, a database whose `user_version` is deliberately set one behind current. Expected outcomes written now: execution counts 1, 2, 2; `user_version` byte-identical before and after a successful snapshot.
**Regression fence:** `crates/cyril-core/src/usage.rs` → `usage::tests::snapshot_worker_coalesces_a_burst_to_one_followup` and `usage::tests::snapshot_worker_connection_does_not_migrate` (both created in this slice)
**Named mutation:** (C3) In the worker loop, replace "drain pending requests, keep the last" with "handle each request in turn" — expected red: the N=10 case counts 11 executions instead of 2. (C7) Construct the worker's connection through `UsageLog::from_connection` — expected red: `user_version` advances and the fence reports the before/after mismatch.
**Complexity/production scale:** One new loop — draining pending requests after each completed snapshot. O(pending) per completed snapshot, pending bounded by triggers arriving during one snapshot. Production-scale input: the measured worst case is KAS at 18 refresh triggers in one turn (`evidence.md` P1), so pending ≤ ~18 in practice and is bounded by the channel regardless. Resulting bound: a drain of tens of elements, microseconds. **Maximum accepted cost: 1 ms per drain**, rationale — the drain runs on the worker thread between snapshots that cost ~700 ms, so anything approaching a millisecond means the drain is doing per-item work it should not.
**Wall budget/phase:** Two phases. Thread spawn: one-off — `N/A — reason: one-off phase; no wall budget`. Snapshot execution: always-on, budget unchanged at ≤ 2 s per snapshot at 100,000 rows, rationale — spec D3 leaves `snapshot()` cost untouched, so the existing `kiro_snapshot_100k_budget_reference` bound is the budget and this slice must not move it.
**Files:** `crates/cyril-core/src/usage.rs`, `crates/cyril-core/src/lib.rs` (re-export)
**Estimate:** 3–4 hours
**Diff estimate:** 380 (impl ~180, test ~200)
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-core --lib snapshot_worker` → both fences pass: execution counts are exactly 1, 2, 2 for N = 0, 1, 10, and `user_version` is unchanged across a successful snapshot.
- Apply each named mutation in turn, re-run → C3's fence reddens on the execution count (11, not 2) and C7's on the `user_version` mismatch; each restores green.
- `cargo clippy -p cyril-core --all-targets -- -D warnings > /dev/null 2>&1 && echo OK` → prints OK.

---

## Slice 3: the panel carries and renders its refresh state

**Claim IDs:** C4, C6, C8
**Expected behavior:** `UsagePanelState` distinguishes three conditions — no snapshot yet, a snapshot held with a refresh in flight, a snapshot held with none in flight — plus a failed-refresh condition. Rendering shows the computing marker, the refreshing marker, neither, and an explicit failure status respectively, with the last-known values still on screen in the failure case.
**Oracle:** `TestBackend` buffer character extraction — the rendered frame, not the state struct's own fields, so a state flag that never reaches the screen still fails.
**Stress fixture:** A panel holding a populated snapshot that then receives a failure, asserted to still show its turn count and duration figures alongside the failure status; plus the empty-database case, which must render the existing "No usage recorded yet" placeholder and NOT the computing marker, since the two mean different things.
**Regression fence:** `crates/cyril-ui/src/widgets/usage_panel.rs` → `usage_panel::tests::refresh_marker_matches_panel_state` and `usage_panel::tests::failed_refresh_keeps_values_and_states_the_failure` (both created in this slice)
**Named mutation:** (C4) In `usage_panel.rs`, render the refreshing marker whenever a snapshot is held, regardless of in-flight state — expected red: the held-and-idle case finds a marker it must not, and the assertion names that state. (C6) Handle the failure condition by leaving the marker untouched — expected red: the failure status is absent from the buffer. (C8) Add `rusqlite` to `crates/cyril-ui/Cargo.toml` — expected red: the existing `ui_declares_no_statistics_dependency` names the banned dependency.
**Complexity/production scale:** `N/A — no new loop; this slice adds state fields and conditional line rendering to a fixed-size overview list.`
**Wall budget/phase:** always-on — the panel renders every frame while open. Budget: the existing `usage_panel_render_budget_reference` bound of 16 ms for a 10,000-group frame must still hold, rationale — that is one frame at 60fps and is the bound already agreed for this widget; two extra conditional lines must not move it.
**Files:** `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/widgets/usage_panel.rs`
**Estimate:** 2–3 hours
**Diff estimate:** 240 (impl ~90, test ~150)
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-ui` → both new fences pass: each of the four conditions renders exactly its own marker or status, each asserted present in its own state (the positive control for the absence assertions) and absent in the others.
- Apply each named mutation in turn → the corresponding fence reddens naming the state that rendered wrongly; each restores green.
- `cargo test -p cyril-ui --test no_percentile_computation` → `ui_declares_no_statistics_dependency` still passes, and reddens under the C8 mutation.

---

## Slice 4: the App drives the worker and no event-loop path calls `snapshot()`

**Claim IDs:** C1, C5, C9, C11
**Expected behavior:** `refresh_usage_panel_from_log` and the `ShowUsage` command arm send a request to the worker instead of computing; results arrive in the `tokio::select!` loop and are applied only when a panel is open at that moment, raising a redraw. A worker whose channel is closed drives the panel to its failure status rather than leaving it computing forever. Both existing scale fences still hold.
**Oracle:** (C1) A barrier held by a second thread observes that the request arrived and that the loop iteration returned, independent of any assertion the App makes about itself. (C5, C9) The read-only `TuiState` view, not the fields the handler writes. (C11) The two fences' own wall-clock measurements against bounds fixed before this change.
**Stress fixture:** A snapshot result delivered in three App states — panel open, panel closed, panel closed-then-reopened — with the closed case asserted to create no panel; plus a pre-closed worker channel; plus a request issued while another is in flight, so the App path is exercised under the coalescing it does not itself implement.
**Regression fence:** `crates/cyril/src/app.rs` → `app::tests::usage_refresh_does_not_block_the_event_loop`, `app::tests::snapshot_result_applies_only_while_a_panel_is_open`, `app::tests::snapshot_worker_unavailable_surfaces_as_failure_status` (all created in this slice). C11's fences already exist: `usage::tests::grouped_percentile_stays_within_budget` and `usage::tests::kiro_snapshot_100k_budget_reference`.
**Named mutation:** (C1) In `app.rs`, replace the worker request with a direct `self.usage_log.snapshot()` call — expected red: the test fails its "iteration completed" assertion / hangs to timeout. (C5) Drop the `has_usage_panel()` re-check at apply and call `show_usage_panel` unconditionally — expected red: the panel-closed case finds a panel. (C9) Treat a send failure as a no-op — expected red: the panel stays in the computing state and the failure-status assertion fails. (C11) Reintroduce a synchronous `snapshot()` inside the 100k fence's timed region — expected red: the 2 s bound is exceeded on the same fixture.
**Complexity/production scale:** `N/A — no new loop; this slice replaces two synchronous calls with channel sends and adds one select! arm.`
**Wall budget/phase:** always-on — the request path runs on every refresh trigger, and P1 measured up to 18 triggers in one KAS turn. Budget: **≤ 5 ms of event-loop time per trigger**, rationale — the trigger path becomes a bounded channel send plus a state flag, so its cost must be indistinguishable from the other notification handling in the same loop; anything above a few milliseconds means snapshot work is still on the loop, which is the defect being fixed.
**Files:** `crates/cyril/src/app.rs`
**Estimate:** 3–4 hours
**Diff estimate:** 400 (impl ~140, test ~260)
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril --lib` → the three new fences pass: the loop iteration completes while a snapshot is blocked on an unreleased barrier AND the request was observed to arrive; results apply in the open and reopened cases and create nothing in the closed case; a closed channel yields the failure status rather than a permanent computing state.
- Apply each named mutation in turn → each fence reddens on its own assertion, naming its own claim's condition rather than "something broke"; each restores green.
- `cargo test -p cyril-core --lib -- --ignored grouped_percentile kiro_snapshot_100k_budget` → both budget fences pass at 100,000 rows, within 700 ms and 2 s respectively.
- `cargo test --workspace && cargo clippy --all-targets -- -D warnings > /dev/null 2>&1 && echo OK && cargo fmt --all --check` → the whole workspace is green, clippy prints OK, formatting is clean.

---

## Plan critique (before the first slice)

Run against the plan as written; findings applied above rather than carried into the build.

- **Slice 1 wall budget cited the wrong baseline (corrected during the slice-1 gate)** — the field first pointed at 565–601 ms, which is `grouped_percentile_stays_within_budget`'s bound for the five latency queries, not the whole-snapshot time this budget governs. The correct pre-change baseline for `kiro_snapshot_100k_budget_reference` is 689 / 696 / 757 ms. Corrected above; the gate was then measured against it.
- **Slice 1 wall budget was unmeasurable** — "≤ 5 ms added at 100,000 rows" sits below this fixture's ±40 ms run-to-run spread, so no honest measurement could have discharged it. Replaced with a bound that is checkable: the existing 2 s fence holds and the time stays inside the recorded 565–601 ms spread. Fixed above.
- **Slice 1 oracle checked for implementation coupling** — reconciling Overview against summed provider and model counts uses three separate SQL aggregations from one snapshot call, so it is an internal-consistency relation rather than a second copy of the same computation; a write landing between rollups breaks the relation, which is exactly the bug class. It is additionally backed by `probe_wal.py`/`oracle_wal.sh`, which share no code with cyril. Accepted as independent.
- **Slice 3 introduces state nothing sets until slice 4** — checked against the repo's staged-module convention, which applies to whole modules rather than struct fields; fields on an existing `pub` struct raise no dead-code warning and are exercised by that slice's own render fences. No change needed.
- **Stress fixtures checked for gentleness** — slice 1's writer runs a continuous append loop for the whole snapshot rather than a fixed three inserts, and slice 2 exercises N = 0, 1, 10 rather than a single burst. Both can fail under the bug class they target. No change needed.

## Self-review

1. **Every design row assigned exactly once** — C2, C10 → slice 1; C3, C7 → slice 2; C4, C6, C8 → slice 3; C1, C5, C9, C11 → slice 4. Eleven claims, eleven assignments, no duplicates. Every `PENDING` falsifier is discharged by the slice implementing its claim; C10 is already `PASS` and its fence is shared with C2 in slice 1. ✓
2. **All thirteen fields per slice**, conditional cells carrying `N/A — reason`. ✓
3. **Every fence created in the slice implementing its claim**; every fence carries its design-named mutation. No claim is fence-less, so no `N/A — approved risk` appears anywhere — consistent with the design's `Risk acceptances: None`. ✓
4. **New loops**: only slice 2's drain, which records asymptotic cost, the production-scale input (18 triggers, measured), the resulting bound, and a 1 ms maximum with rationale. Every always-on phase has a wall budget with rationale; the one-off thread spawn records `N/A`. ✓
5. **Partition**: 1,140 + 40% margin = 1,596 ≤ 4,000, single increment A; every slice names it; the increment has a mergeable definition against the discovered default branch. ✓
6. **Tracker taxonomy**: the only deferral phrases in this plan are the design's two intended-future-work items, both citing verified IDs (cyril-b163, cyril-c6la); no new deferral is introduced here. ✓
7. **No slice is declared complete** — completion is `checkpointed-build`'s to judge. ✓
