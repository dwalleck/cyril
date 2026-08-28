# Design: cyril-nanu — non-blocking usage-panel refresh

## Route and inputs

- **Route**: Empirical (`route.md`, 2026-08-28). T1 fired on four unverified premises; T2 and T3 also fired.
- **Behavior set**: `spec.md` behaviors B1–B7, approved verbatim 2026-08-28. Not restated here; claims reference them by ID.
- **Spec decisions consumed**: D1 visible in-flight marker · D2 asynchronous open · D3 off-loop only, `snapshot()` cost unchanged · D4 dirty-flag coalescing · D5 one deferred read transaction · D6 last values on reopen · plus the cyril-kryv-adopted failure status and trigger-driven retry.
- **Empirical premises** (`evidence.md`, all PASS): P1 refresh cadence — v2 peaks at 2 triggers/turn, KAS at **18** · P2 WAL second-connection consistent read with the writer unblocked, `DELETE` control blocked · P3 `UsageSnapshot: Send + 'static` · P4 the App's existing `mpsc` + `select!` carrier is `Send + 'static` too. P5 (cost decomposition) and P6 (stall observability) are recorded non-premises.
- **Source pointers**: `.cyril-nanu/{route,spec,evidence}.md`, probes `probe_cadence.py`, `probe_wal.py`, `probe_send.rs`, `probe_txn.rs`.

## Input shapes

| ID | Shape | Status |
|----|-------|--------|
| S1 | Trigger arrives, panel closed | Covered — C5 (existing `has_usage_panel()` guard keeps it free) |
| S2 | Trigger arrives, panel open, nothing in flight | Covered — C1, C3 |
| S3 | Trigger arrives, panel open, snapshot in flight | Covered — C3 |
| S4 | N triggers during one in-flight snapshot: N = 0, 1, many (KAS measured 17) | Covered — C3 |
| S5 | Panel opened with no snapshot completed yet in this process | Covered — C4 |
| S6 | Panel reopened with a snapshot held from earlier | Covered — C4 |
| S7 | Snapshot completes while a panel is open | Covered — C5 |
| S8 | Snapshot completes after the panel closed | Covered — C5 |
| S9 | Snapshot completes after the panel closed and was reopened | Covered — C5 (the rule is "a panel is open when it lands", not "the same panel instance") |
| S10 | Snapshot returns `Err` | Covered — C6 |
| S11 | Empty database — snapshot succeeds with zero rows | Covered — C4 (existing "No usage recorded yet" placeholder is reached only after a completed snapshot, never confused with the computing state) |
| S12 | A write commits while the snapshot's rollups are running | Covered — C2 |
| S13 | The snapshot worker thread fails to spawn | Covered — C9 |
| S14 | The worker's result channel closes while the panel is open | Covered — C9 |
| S15 | Two `/usage` opens in quick succession before any snapshot lands | Covered — C3 (an open is a trigger like any other; coalescing applies) |
| S16 | Usage log path unwritable / `UsageLog::open` fails at startup | `N/A — pre-existing startup path, unchanged by this design: the App already fails or degrades before any panel exists.` |
| S17 | Snapshot takes longer than the operator's whole session | `N/A — permanent non-goal: D3 leaves snapshot() cost unchanged, and no timeout is introduced; an unbounded-latency guard would be a different feature.` |

## Placement

### Capability A — producing a snapshot off the event loop

- **Owner**: `cyril-core::usage` — a `spawn_usage_snapshot_worker(path) -> (UsageSnapshotHandle, tokio::sync::mpsc::UnboundedReceiver<UsageSnapshotResult>)`, placed beside the existing `spawn_usage_enrichment_worker` (`usage/kiro_sidecar.rs:76`). It wins over `cyril` because the worker owns a `rusqlite::Connection`, and CLAUDE.md forbids the binary from containing storage logic; `cyril` keeps the receiver and the routing, exactly as it does for enrichment today.
- **New seam**: none required — this slots behind the shape the enrichment worker already established (named OS thread owning its resource, `std::sync::mpsc` for commands in, `tokio::sync::mpsc::unbounded` for results out, consumed in the App's `select!`). Two shapes were considered and one rejected: **(i) `Arc<Mutex<UsageLog>>` + `tokio::task::spawn_blocking`** — fewer moving parts, but it puts a mutex on the path the writer also uses, so a long snapshot would block the *write* the event loop performs, reintroducing the stall this change removes; **(ii) the worker thread**, chosen — the connection stays on one thread with no lock, and coalescing has an obvious home (drain the request channel, keep the last).
- **Forbidden**: the worker must not open its connection through `UsageLog::from_connection` (that path runs schema migrations — the worker is a reader and must never migrate); it must not hold or touch `UiState`; `cyril` must not call `UsageLog::snapshot()` from any event-loop path.

### Capability B — coalescing

- **Owner**: the worker. It is the only component that knows whether a snapshot is currently running; putting the dirty flag in `App` would duplicate in-flight tracking the worker already has, and would leave `App` guessing when a request is redundant.
- **New seam**: none — the request channel *is* the seam; coalescing is "drain pending requests, keep at most one".
- **Forbidden**: `App` must not attempt its own suppression of triggers; every trigger sends, and the worker decides.

### Capability C — in-flight and last-known panel state

- **Owner**: `cyril-ui::UiState`, on the existing `usage_panel: Option<UsagePanelState>` field (`state.rs:100`). The marker is a rendering concern and `UsagePanelState` already carries the snapshot it renders.
- **New seam**: none — a refresh-state field on `UsagePanelState`, rendered by `usage_panel::render`.
- **Forbidden**: `cyril-ui` must not gain a dependency on `UsageLog`, `rusqlite`, or any statistics crate — the existing `tests/no_percentile_computation.rs::ui_declares_no_statistics_dependency` fence already asserts this and must keep passing.

## Claims

- **C1** — No code path reachable from the App event loop calls `UsageLog::snapshot()`.
- **C2** — A snapshot's rollups observe one point in time even when a write commits partway through it.
- **C3** — Triggers arriving while a snapshot is in flight produce exactly one follow-up snapshot, regardless of how many arrive.
- **C4** — The panel shows the computing state only before the process's first completed snapshot, and last-known values with the refreshing marker on every later open.
- **C5** — A completed snapshot updates the panel when a panel is open as it lands, and is discarded otherwise.
- **C6** — A failed snapshot leaves the previously rendered values intact and renders an explicit failure status.
- **C7** — The snapshot worker's connection never runs schema migrations.
- **C8** — `cyril-ui` gains no dependency on `UsageLog`, `rusqlite`, or a statistics crate.
- **C9** — A worker that cannot spawn, or whose channel closes, surfaces as the panel's failure status rather than a permanent computing state.
- **C10** — `snapshot()` opens its deferred read transaction through `&self`, keeping its receiver unchanged.
- **C11** — The two existing scale fences still hold at 100,000 rows.

## Subtractive sweep

This change is **subtractive**. The removed constraint: *the App event loop's single-threaded execution serialized every usage snapshot against every usage write and against all UI-state mutation.* What that silently guaranteed, and what now replaces it:

| Removed guarantee | Now possible | Replacement |
|---|---|---|
| A snapshot observed one DB state; no write could interleave its nine queries. | A write commits between rollups; Overview disagrees with the per-model page. | C2 (deferred read transaction, D5) |
| The rendered snapshot was always the newest at render time. | The panel shows values older than the last committed write. | C4 (visible marker, D1) — bounded and stated, never silent |
| At most one snapshot existed at a time. | KAS bursts (measured: 17 triggers in one turn) could queue 17 recomputes. | C3 (coalescing, D4) |
| A snapshot's result always applied to the panel that asked for it. | The panel may have closed, or closed and reopened, before the result lands. | C5 |
| `has_usage_panel()` was checked immediately before the values were applied. | The guard and the apply are now separated in time. | C5 — the guard is re-checked at apply, not only at request |
| A redraw followed the trigger synchronously. | The redraw must be raised when the result lands, not when it is requested. | C5 — `redraw_needed` set on apply |

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | No event-loop path calls `snapshot()` | S2 | Drive `refresh_usage_panel_from_log` and the `ShowUsage` arm against a snapshot source whose production blocks on a barrier the test never releases; the loop iteration must still complete. Falsified if the call blocks. **Other cause that would produce a completing iteration:** the panel guard short-circuiting before any snapshot is requested — so the test asserts the request was *sent* as well as that the iteration completed. | The barrier itself: a second thread observes the request arrived and the loop returned, independent of any App assertion. | In `app.rs`, replace the worker request with a direct `self.usage_log.snapshot()` call. Expected red: the test hangs to timeout / fails the "iteration completed" assertion. | `app::tests::usage_refresh_does_not_block_the_event_loop` | minutes | PENDING — checkpointed-build, per-slice gate |
| C2 | One point in time per snapshot | S12 | Seed a log, start a snapshot, commit a turn while its rollups run, and reconcile the returned Overview turn count against the sum of per-provider and per-model counts. Falsified if they differ. **Other cause:** a fixture whose writer never actually commits mid-snapshot — so the test asserts the writer's insert returned Ok before reconciling. | `probe_wal.py` / `oracle_wal.sh` established the isolation independently of any cyril code; the fence reconciles three independently-computed rollups against each other rather than against one query. | In `usage.rs`, drop the `unchecked_transaction()` and run the rollups on the bare connection. Expected red: Overview count exceeds the summed per-model counts by the number of mid-snapshot commits. | `usage::tests::snapshot_is_atomic_under_concurrent_append` | minutes | PENDING — checkpointed-build, per-slice gate |
| C3 | Coalescing to one follow-up | S3, S4, S15 | Deliver N triggers (N = 0, 1, 10) while one snapshot is held mid-execution, release it, and count executions: expect 1, 2, 2. Falsified by any other count. **Other cause:** a worker that silently drops every trigger would also yield 1 for N=0 — so the N=1 and N=10 cases, which require exactly 2, are what discriminate. | A counting fake snapshot source incremented on a separate thread, compared against the hand-derived table (0→1, 1→2, 10→2). | In the worker, replace "drain pending, keep last" with "handle each request in turn". Expected red: N=10 yields 11 executions, not 2. | `usage::tests::snapshot_worker_coalesces_a_burst_to_one_followup` | minutes | PENDING — checkpointed-build, per-slice gate |
| C4 | Computing vs refreshing states | S5, S6, S11 | Render the panel in three states — no snapshot yet, snapshot held with one in flight, snapshot held with none in flight — and assert the buffer carries the computing marker, the refreshing marker, and neither, respectively. Falsified if any state renders the wrong marker. **Absence assertion**, so a positive control is required: the same test asserts each marker DOES appear in its own state. | `TestBackend` character extraction, independent of the state struct's own fields. | In `usage_panel.rs`, render the refreshing marker whenever a snapshot is held. Expected red: the no-snapshot-in-flight case finds a marker it must not. | `usage_panel::tests::refresh_marker_matches_panel_state` | minutes | PENDING — checkpointed-build, per-slice gate |
| C5 | Apply only when a panel is open | S1, S7, S8, S9 | Deliver a completed snapshot in three App states — panel open, panel closed, panel closed-then-reopened — and assert the panel's values update in the first and third and no panel is created in the second. Falsified if a result creates or mutates a closed panel. **Other cause:** a result that never arrives would also leave the closed case untouched — so the open case in the same test is the positive control. | App-level state inspection through the `TuiState` read-only trait, not the field the handler writes. | In `app.rs`, drop the `has_usage_panel()` re-check at apply and call `show_usage_panel` unconditionally. Expected red: the panel-closed case finds a panel. | `app::tests::snapshot_result_applies_only_while_a_panel_is_open` | minutes | PENDING — checkpointed-build, per-slice gate |
| C6 | Failure keeps values, states the failure | S10 | Deliver an `Err` snapshot result to a panel holding values; assert the values are unchanged and the buffer carries the failure status. Falsified if values clear or the status is absent. **Other cause:** a handler that ignores errors entirely also leaves values intact — so the status assertion, not the value assertion, is what discriminates. | `TestBackend` buffer text, independent of the error type. | In `app.rs`, handle the `Err` arm with `tracing::warn!` only (today's behavior). Expected red: the failure status is absent from the buffer. | `usage_panel::tests::failed_refresh_keeps_values_and_states_the_failure` | minutes | PENDING — checkpointed-build, per-slice gate |
| C7 | Worker never migrates | S13 | Point the worker at a database whose schema is at an older version and assert the schema version is unchanged after the worker runs a snapshot. Falsified if the version advances. **Other cause:** a worker that failed to open at all would also leave the version unchanged — so the test asserts the snapshot succeeded first. | Direct `PRAGMA user_version` read on a third connection, independent of the worker's own code. | Construct the worker's connection through `UsageLog::from_connection`. Expected red: `user_version` advances. | `usage::tests::snapshot_worker_connection_does_not_migrate` | minutes | PENDING — checkpointed-build, per-slice gate |
| C8 | cyril-ui stays persistence-free | Placement (Capability C) | Scan `crates/cyril-ui/Cargo.toml` for `rusqlite`, `statrs`, `quantiles`. Falsified by any hit. | The existing `tests/no_percentile_computation.rs::ui_declares_no_statistics_dependency`, already in the repo and independent of this change. | Add `rusqlite` to `crates/cyril-ui/Cargo.toml`. Expected red: that test names the banned dependency. | `no_percentile_computation::ui_declares_no_statistics_dependency` (existing) | minutes | PENDING — checkpointed-build, per-slice gate |
| C9 | Spawn/channel failure is visible | S13, S14 | Construct the App with a worker handle whose channel is already closed, open the panel, and assert the panel reaches the failure status rather than remaining in the computing state. Falsified if it stays computing. **Other cause:** a panel that never opened would also lack the computing state — so the test asserts a panel exists first. | `TestBackend` buffer text plus the `TuiState` read-only view. | In `app.rs`, treat a send failure as a no-op. Expected red: the panel remains in the computing state forever. | `app::tests::snapshot_worker_unavailable_surfaces_as_failure_status` | minutes | PENDING — checkpointed-build, per-slice gate |
| C10 | Deferred read txn through `&self` | S12 | Call `unchecked_transaction()` inside a `&self` method and assert two reads inside it agree while a writer commits between them, with a post-commit read proving the write was visible afterwards. Falsified if it does not compile or the reads differ. **Other cause:** a reader that simply never observes writes — excluded by the post-commit positive control. | `.cyril-nanu/probe_txn.rs`, compiled against real rusqlite and run against a real file database on a second thread. | Replace `unchecked_transaction()` with two bare `query_row` calls. Expected red: the two reads differ (3 then 4). | `usage::tests::snapshot_is_atomic_under_concurrent_append` (shared with C2) | minutes | **PASS** |
| C11 | Scale fences unregressed | S4 at max scale | Run both ignored budget fences explicitly and assert they pass. Falsified if either exceeds its bound. | The fences' own wall-clock measurement against bounds fixed before this change. | Reintroduce a synchronous `snapshot()` call inside the 100k fence's timed region. Expected red: the 2 s bound is exceeded on the same fixture. | `usage::tests::grouped_percentile_stays_within_budget` and `usage::tests::kiro_snapshot_100k_budget_reference` (both existing, `#[ignore]`) | minutes | PENDING — checkpointed-build, per-slice gate |

## Non-goals and future work

**Permanent non-goals** (rationale recorded, no ticket):

- Reducing `snapshot()`'s own cost — caching, invalidate-on-append, or splitting cheap fields from expensive rollups. Settled by spec D3; the requester chose "off-loop is enough" over "off-loop now, cost later".
- A timeout or unbounded-latency guard on a snapshot (S17). No bound is introduced; the marker states that work is in flight for as long as it takes.
- Moving usage *writes* off the event loop. Out of scope in `spec.md`; writes are single statements, not nine-rollup reads.

**Intended future work** (verified tracker IDs):

- **cyril-b163** (open, verified 2026-08-28) — `usage_turns` has no retention, so snapshot cost grows with install age. This design leaves that axis untouched; coalescing bounds how *often* the cost is paid, not how large it is.
- **cyril-c6la** (open, verified 2026-08-28) — the same always-on-recompute shape on the conversation render path. Not addressed here.

## Falsifier run log

- **C10** (cheapest, run before approval): `cp .cyril-nanu/probe_txn.rs crates/cyril-core/tests/probe_nanu_txn.rs && cargo test -p cyril-core --test probe_nanu_txn -- --nocapture` → `C10: first=3 second=3 writer_rows_inserted=1` / `C10 control: after_txn=4` / `test result: ok. 1 passed`. **PASS** — `unchecked_transaction()` compiles through `&self`, both reads inside it agree while a writer commits, and the post-transaction read sees 4, proving isolation rather than a reader that never updates. Probe copy removed; no production file changed. 2026-08-28.

## Approval

Requester approval (verbatim): "yes"
Date: 2026-08-28

Approved: claims C1–C11 with their falsifiers, oracles, named mutations and regression fences; the placement decisions for capabilities A, B and C including the rejection of `Arc<Mutex<UsageLog>>` + `spawn_blocking`; the subtractive sweep's six replacements; the non-goals and the two deferrals to cyril-b163 and cyril-c6la; and the C10 falsifier result.

Risk acceptances approved: **None** — no row carries `Regression fence: N/A — approved risk`.
