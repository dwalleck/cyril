# Spec: Non-blocking usage-panel refresh

## Request (verbatim)
> MEASURED while building cyril-9kyk. UsageLog::snapshot() rebuilds the ENTIRE snapshot — overview, provider/model/folder/agent_type rollups, cost and charge totals, tools, context, recent, errors — and it runs on an always-on path. [...] SCOPE: stop recomputing everything per refresh. Options to weigh: debounce the refresh (coalesce bursts of context samples), cache the snapshot and invalidate on append only, split the panel's cheap fields from the expensive rollups so a context tick refreshes only what changed, or compute rollups off the UI path. Whatever is chosen must keep the panel's displayed numbers honest — a stale cache that silently lags a completed turn is worse than a slow refresh.

## What this is
Today every usage-panel refresh runs `UsageLog::snapshot()` synchronously on the App event loop, so the terminal stops responding for the duration of a full nine-rollup recompute — ~690–760 ms over a 100,000-row log. After this change no `snapshot()` call runs on that loop: refreshes are computed off it and delivered back as events, the panel shows the values it last received while one is in flight, and it says on screen that it is doing so. The numbers themselves, and the queries that produce them, are unchanged.

## Roles
- **Cyril operator**: drives a Kiro v2 or KAS session and opens `/usage` to inspect live and aggregated usage, latency, context, tools, identity groupings, and account metering. Adopted verbatim from cyril-kryv's spec; this change alters when that operator's numbers update, not who reads them.

## Behavior

### B1 — First open renders without waiting
- **Given**: the usage panel is closed and no snapshot has completed in this process.
- **When**: the Cyril operator submits `/usage`.
- **Then**: the modal renders on the next drawn frame carrying the computing marker and no data rows; the App event loop continues to process terminal events while the snapshot runs; when the snapshot lands the rows appear and the marker clears.

### B2 — Reopen renders the last known values
- **Given**: the usage panel is closed and a snapshot completed earlier in this process.
- **When**: the Cyril operator submits `/usage`.
- **Then**: the modal renders immediately with that snapshot's values and the refreshing marker; a new snapshot is requested; when it lands the values are replaced and the marker clears.

### B3 — A refresh trigger never blocks the loop
- **Given**: the usage panel is open and no background snapshot is in flight.
- **When**: the App handles a `UsageWrite::Turn` append (`app.rs:1016`), a `UsageWrite::Context` sample (`app.rs:1041`), or a sidecar enrichment result (`app.rs:968`).
- **Then**: a snapshot is started off the event loop, the refreshing marker appears, and the loop continues; when the snapshot lands the panel's values are replaced and the marker clears.

### B4 — A burst of triggers coalesces to at most two snapshots
- **Given**: the usage panel is open and a background snapshot is in flight.
- **When**: one or more further refresh triggers arrive before it completes.
- **Then**: no additional snapshot starts while it runs; when it completes, exactly one further snapshot runs if any trigger arrived, and the values the panel finally shows reflect the most recent committed write.

### B5 — Every rollup on a frame agrees with every other
- **Given**: turns are being appended while a background snapshot runs.
- **When**: that snapshot completes and the panel renders.
- **Then**: every page derives from one point in time — the Overview turn count equals the sum of the per-provider turn counts and the sum of the per-model turn counts on that same frame.

### B6 — A failed refresh is stated, not swallowed
- **Given**: the usage panel is open showing the values of a completed snapshot.
- **When**: a background snapshot returns `Err`.
- **Then**: those values remain on screen, the in-flight marker is replaced by an explicit failure status, and the next refresh trigger attempts a new snapshot.

### B7 — Closing the panel abandons in-flight work
- **Given**: a background snapshot is in flight.
- **When**: the Cyril operator closes the panel.
- **Then**: the panel closes on the next frame; the snapshot's result, when it arrives, changes no visible state and starts no further snapshot.

## Success criteria

- **Binary / structural**: no code path reachable from the App event loop calls `UsageLog::snapshot()` synchronously — exact condition: `refresh_usage_panel_from_log` and the `CommandResultKind::ShowUsage` arm contain no direct `snapshot()` call; checked by a named unit test that drives both paths against a `UsageLog` whose snapshot blocks on a barrier and asserts the loop iteration completes without waiting.
- **Quantitative**: 10 refresh triggers delivered while one snapshot is in flight produce exactly 2 snapshot executions, measured by a counting fake snapshot source in an App-level test.
- **Quantitative**: after the last trigger of a burst, the panel shows values derived from that trigger's committed state within one snapshot duration, measured by a test harness that signals snapshot completion deterministically rather than by wall clock.
- **Binary / structural**: while a snapshot is in flight the rendered buffer contains the in-flight marker, and once it lands the marker is absent — exact condition on the two states of B1/B2/B3; checked by `TestBackend` render assertions on the panel.
- **Binary / structural**: a background snapshot returning `Err` leaves the previously rendered values intact and renders an explicit failure status — exact condition of B6; checked by a render test with an injected snapshot error.
- **Binary / structural**: with a writer appending turns concurrently, the Overview turn count equals the sum of per-provider and per-model turn counts on the same snapshot — exact condition of B5; checked by a concurrency test that appends during a snapshot and reconciles the three counts.
- **Quantitative**: the two existing scale fences still hold when run explicitly — `grouped_percentile_stays_within_budget` ≤ 700 ms and `kiro_snapshot_100k_budget_reference` ≤ 2 s at 100,000 rows, measured by those tests.

## Out of scope

This change does NOT include:

- **Reducing `snapshot()`'s own cost** — caching with invalidate-on-append, splitting the panel's cheap fields from the expensive rollups, or decomposing any query. Decided out by D3; the nine rollups and their SQL are untouched.
- **Retention or pruning of `usage_turns`** — the row growth that makes the cost scale is cyril-b163.
- **The conversation render path** — every frame re-rendering all historical messages is cyril-c6la, a sibling with the same shape and its own ticket.
- **Moving usage writes off the event loop** — `append`, `record_context` and `enrich_record` keep running on the loop; only the read path moves.
- **Changing what the panel displays** — fields, pages, ordering and formatting are unchanged apart from the in-flight marker (D1) and the failure status (B6).
- **The account-usage query** — `_kiro/account/getUsage` is already asynchronous per cyril-kryv and is not revisited.

## Related issues

- **cyril-9kyk** (closed): the p90/max latency work that measured this. Carries the cost history (~342 ms baseline → ~690–760 ms at 100k rows after p90/max) and the operator's 2026-08-28 "Raise budget + ticket the refresh" decision that created cyril-nanu. Its 700 ms `grouped-percentile` budget and 2 s `kiro_snapshot_100k` budget are the fences this change must not regress.
- **cyril-b163** (open, P3): `usage_turns` has no retention, so row count — and therefore `snapshot()` cost — grows for the life of the install. Bears directly: retention would shrink this cost but cannot fix the recompute-per-sample shape. Not a blocker; not in scope here.
- **cyril-kryv** (closed): shipped the `/usage` panel. Its spec already decided this exact shape for the *account* query — "opening `/usage` never waits for `_kiro/account/getUsage`; exactly one query is dispatched per open ... and the response refreshes the open panel", with "a failed refresh shows explicit status beside the last successful in-process value". That is the precedent D1 adopts for local rollups.
- **cyril-c6la** (open, P2): the same disease on the render path — every frame re-renders all historical messages before viewport clipping. Sibling, not dependency; its AC shape ("a production-shape benchmark exists; steady-state work is bounded; output remains correct") is the model for this spec's criteria.
- **cyril-gfkm**, **cyril-4h6i** (closed): built the usage observer that produces the `UsageWrite::Turn` / `UsageWrite::Context` writes driving the refresh. No staleness decision recorded in either.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| While the panel is open and a refresh is in flight, what must the operator see? | Numbers may lag a completed turn, provided the panel marks it on screen and converges within a bounded time. | Requester selected "Lag, but visibly marked", 2026-08-28. Consistent with the precedent cyril-kryv already shipped for the account query: render what is held, refresh asynchronously, show explicit status rather than a silent stale value. | The panel gains a visible in-flight marker. A refresh never blocks the event loop, and the displayed values are those of the last completed snapshot until a newer one lands. Rules out the silent-lag option the ticket itself warns against. |
| Is the panel's first `/usage` open in scope, or only the recurring refreshes? | Open goes asynchronous too. | Requester selected "Open goes async too", 2026-08-28. Open is the single worst stall (~700 ms at 100k rows, `app.rs:1838`) and it reuses the D1 marker rather than needing a second mechanism. | `/usage` renders its frame immediately in a computing state with no rows, and fills in when the first snapshot lands. The panel gains a no-data-yet state it does not have today. No call site of `snapshot()` remains on the event loop. |
| Is reducing `snapshot()`'s own cost in scope, or is moving it off the loop enough? | Off-loop is enough; `snapshot()` is unchanged. | Requester selected "Off-loop is enough", 2026-08-28. The ticket's complaint is the stall, and off-loop plus coalescing removes it without decomposing a query layer that cyril-9kyk fenced in the preceding change. | Convergence bound is one snapshot duration after the last trigger, not a smaller number. Caching, invalidate-on-append and cheap/expensive field splitting are out of scope. Cost still grows with row count; that axis is cyril-b163. |
| What happens to triggers that arrive while a background snapshot is already running? | Set a dirty flag; when the running snapshot lands, if the flag is set, exactly one more runs. | Requester selected "Mark dirty, run once more", 2026-08-28. Dropping mid-flight triggers can leave the panel showing pre-turn numbers indefinitely — the silent lag the ticket rules out; cancel-and-restart can starve under a sample stream arriving faster than one snapshot. | At most one extra recompute per burst. The panel always converges to the newest committed write once triggers stop. A burst of N context samples costs at most 2 snapshots, not N. |
| What does the panel show while a snapshot is in flight after a close and reopen? | The last completed snapshot's values, with the refreshing marker; only the process's first open shows the empty computing state. | Requester selected "Last snapshot, marked refreshing", 2026-08-28. Consistent with D1 — visible lag is acceptable, and discarding held values would cost an empty panel on every open. | The held snapshot outlives a panel close. B1 applies once per process; B2 applies to every later open. |
| May the nine rollups of one snapshot straddle a concurrent write? | No — they run inside one deferred read transaction. | Requester selected "One consistent read", 2026-08-28. WAL is already enabled (`usage.rs:723`), so a consistent read never blocks the writer and costs nothing extra; without it the Overview total can contradict the per-model page on the same frame. | `snapshot()` opens a read transaction around its rollups. The panel may lag a turn (D1) but never disagrees with itself (B5). |
| What does the panel show when a background snapshot fails? | The last successful values remain, with an explicit failure status in place of the in-flight marker. | Adopted from cyril-kryv, which already decided this for the account query: "a failed refresh shows explicit status beside the last successful in-process value". Consistency with shipped behavior on the same panel. | B6. Today the failure is `tracing::warn!` only (`app.rs:946`) and the operator sees a silently stale panel. |
| Does a failed background snapshot retry on its own? | No — the next refresh trigger retries. | Derived from D4: the dirty-flag mechanism already re-runs on the next trigger, so a separate retry timer would be a second mechanism for the same outcome. No requester decision needed. | No retry/backoff machinery. A failure status persists until the next trigger produces a successful snapshot. |
| What happens to an in-flight snapshot when the panel closes? | Its result is discarded and starts no further snapshot. | Derived: with the panel closed there is no state for the result to update, and `refresh_usage_panel_from_log` is already guarded by `has_usage_panel()` (`app.rs:937`). | B7. Closing is not blocked by in-flight work. |
| **Edge — empty set (zero rows)** | Unchanged: the panel renders "No usage recorded yet". | Existing behavior (`usage_panel.rs`, `overview_lines`); this change alters when a snapshot arrives, not what an empty one renders. | A completed snapshot over an empty log clears the computing marker and shows the existing placeholder, distinct from B1's no-data-yet state. |
| **Edge — max scale** | 100,000 rows across ≥ 20 (provider, model) groups, the cyril-9kyk fixtures. | Consistency with the fences cyril-9kyk already pins; no new scale point is introduced. | The success criteria measure at that scale and the two existing budget fences must still hold. |
| **Edge — null / missing field** | N/A — no field semantics change. | The snapshot's contents and their `Option` semantics are untouched by D3. | No new absent-value handling. |
| **Edge — concurrent writes** | Covered by D5: one deferred read transaction per snapshot; writes continue on the event loop. | Requester decision D5 plus the out-of-scope line on writes. | B5. A write landing mid-snapshot is either wholly visible to it or wholly absent. |
| **Edge — permission denied / unauthenticated** | N/A — `usage.sqlite3` is a local file opened by this process; there is no authentication boundary. | Same rationale recorded in cyril-9kyk's spec for the same store. | No auth surface. |
| **Edge — partial failure (one of N succeeded)** | N/A — a snapshot is one transaction that either returns a whole `UsageSnapshot` or an `Err`; there is no partial result to display. | D5 makes the read atomic; the existing `snapshot()` signature returns `Result<UsageSnapshot, _>` with no partial variant. | B6 covers the whole-failure case; no partial-render state exists. |
| **Edge — retries / idempotency** | Covered above: no automatic retry; the next trigger retries. Snapshots are pure reads, so repeating one is idempotent. | Derived from D4. | Running an extra snapshot is always safe; the coalescing bound is about cost, not correctness. |
| **Edge — soft-deleted records** | N/A — `usage_turns` has no soft-delete column and no delete path exists. | Same rationale recorded in cyril-9kyk's spec; cyril-b163 confirms no delete path. | Every row is live. |
| **Edge — multi-tenancy boundaries** | N/A — one local file for one Cyril operator; there is no tenant dimension. | Same rationale recorded in cyril-9kyk's spec. | No tenancy surface. |
| **Edge — time-zone / DST** | N/A — this change moves when a computation runs, and introduces no wall-clock formatting or date predicate. | The snapshot's timestamps are stored epoch milliseconds, unchanged. | Zone-independent. |
| **Edge — replication lag** | N/A — a single local SQLite file with no replica. | Same rationale recorded in cyril-9kyk's spec. | No replication. |
| **Edge — cache invalidation** | The dirty flag of D4 is the invalidation, and the held snapshot of D6 is the only cache. | Requester decisions D4 and D6. | A held snapshot is superseded only by a newer completed one; it is never invalidated into an empty state after the first open. |

## Approval

Requester approval (verbatim): "yes"
Date: 2026-08-28

Approval covers: the seven behaviors B1–B7, the seven success criteria, the out-of-scope list, and Decisions D1–D6 plus the derived and prior-art-adopted rows presented in summary — the visible in-flight marker, asynchronous panel open with last-values-on-reopen, off-loop-only scope with `snapshot()` unchanged, dirty-flag coalescing, the single deferred read transaction, the cyril-kryv-adopted failure status, and trigger-driven rather than timer-driven retry.
