# Route: cyril-nanu

Change: `UsageLog::snapshot()` recomputes every rollup on each usage-panel refresh, synchronously on the App event loop
Date: 2026-08-28

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Four premises the design must rest on, none covered by current evidence. **(a) Cost decomposition.** `snapshot()` (`crates/cyril-core/src/usage.rs:1144`) issues nine sub-rollups — overview, providers, models, folders, agent_types, tools, context, recent, errors. Choosing between "split the cheap fields from the expensive rollups" and the other candidates requires knowing which of the nine dominate; no per-query breakdown exists. cyril-9kyk measured only its own additions (~570 ms of latency queries) and whole-snapshot totals (~690–760 ms at 100k rows, 2026-08-28), never a decomposition. **(b) Refresh cadence.** cyril-nanu asserts context samples fire "MULTIPLE TIMES DURING a turn"; the producer is `UsageObserver::apply` returning `UsageWrite::Context` (`usage.rs:473`), but the actual count per turn has never been observed on a live session, and it sets the whole cost multiplier. **(c) Concurrent-reader viability.** Whether a second read-only SQLite connection can serve `snapshot()` while the writer appends, under the existing `BUSY_TIMEOUT` (`usage.rs:51`), without lock contention or stale reads — rusqlite/SQLite behavior, unverified in this repo. **(d) Observability of the stall.** Whether the block manifests as dropped keystrokes or a frozen frame at all; the fix's value depends on it, and no probe has driven the TUI under load. | **yes** |
| 2 | Structural boundary | `App` owns `usage_log: UsageLog` as a plain owned field (`crates/cyril/src/app.rs:74`, constructed at `:345`/`:398`), and `UsageLog` is `{ connection: rusqlite::Connection }` — `Send` but not `Sync`. `snapshot(&self)` is called at `app.rs:940` (`refresh_usage_panel_from_log`) and `app.rs:1838` (`/usage` open); `append`/`record_context`/`enrich_record` need `&mut self` on the same value. Taking snapshots off the event loop requires either a new public `cyril-core` surface (a `Send` read-only snapshot handle over its own connection) or an ownership change in `cyril`, plus a new delivery path into the `tokio::select!` loop. That is a cross-module placement decision between `cyril-core` and `cyril` and a public-API addition. | **yes** |
| 3 | Production-scale risk | Latency, on an always-on path. Measured 2026-08-28: `snapshot()` costs ~690–760 ms at 100,000 rows (`kiro_snapshot_100k_budget_reference`), against a 2 s fence that already failed on the CI ubuntu leg for machine load. `refresh_usage_panel_from_log` (`app.rs:936`) fires per turn end (`:1016`), per context sample (`:1041`) and per sidecar enrichment (`:968`), guarded only by `has_usage_panel()`. Data volume compounds it: `usage_turns` has no retention (cyril-b163), so the row count grows for the life of the install and the cost grows with it. Budget and stress-fixture machinery are required; Local has none. | **yes** |
| 4 | Explicit behavior | No. The issue names four candidate mechanisms — debounce the refresh, cache the snapshot and invalidate on append, split the panel's cheap fields from the expensive rollups, or compute rollups off the UI path — and decides none of them. It states one hard constraint ("a stale cache that silently lags a completed turn is worse than a slow refresh") without saying what staleness *is* tolerable, whether the panel may show a loading state, or what the user-visible contract is while a refresh is in flight. Unresolved decisions: staleness tolerance and how it is signalled; whether `/usage` open may block; whether the fix applies to all three call sites or only the per-sample one; what the post-fix budget is. | **no** |

Unknown tests: none

## Selected route

**Empirical** — T1 fires first in precedence (Empirical > Structural > Local). T2 and T3 also fire, so the route would be Structural on their own; T1's unverified cost decomposition, refresh cadence, concurrent-reader viability and stall observability must be measured before a design can choose between the four candidate mechanisms.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — T4 `no`: four candidate mechanisms and no decided staleness contract |
| evidence.md, probe.* | prove-it-prototype | required — T1 `yes`: cost decomposition, refresh cadence, concurrent-reader viability, stall observability |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural/Empirical — `prove-it-prototype` records `PASS` for every empirical premise, every later artifact satisfies its owning stage's completion criterion, and `checkpointed-build` records no `FAIL` in its recorded gate.
