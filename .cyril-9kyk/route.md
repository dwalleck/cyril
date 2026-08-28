# Route: cyril-9kyk

Change: Add p90 latency percentiles (duration_ms, ttft_ms) to the usage summary rollup
Date: 2026-08-28

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The only external premise is SQLite's percentile support, which decides SQL-side vs Rust-side computation. **Verified during routing** against the exact dependency build (`rusqlite = { version = "0.39", default-features = false, features = ["bundled"] }`, workspace `Cargo.toml:59`) via a throwaway integration test, since removed: SQLite **3.51.3**; `percentile(x, 90)` → *no such function*; `percentile_cont(0.9) WITHIN GROUP (ORDER BY x)` → *syntax error*; `median(x)` → *no such function*. The `ORDER BY x LIMIT 1 OFFSET CAST((COUNT(*)-1)*0.9 AS INTEGER)` nearest-rank fallback **works** (fixture `1..9,100` → p90 = 9, distinct from mean 14.5 and max 100). **Correction (same day, prompted by the requester): the first probe only covered aggregate/ordered-set functions and wrongly implied SQLite cannot compute percentiles. It can — via WINDOW functions (present since 3.25).** Re-probed on the same build: `CUME_DIST() OVER (ORDER BY x)` yields the same nearest-rank p90 = 9; a single query returns `p90=9, max=100, n=10` in **one pass**; and `CUME_DIST() OVER (PARTITION BY g ORDER BY x)` **composes with grouping** (a→9/100, b→60/60). Verdict is unchanged — the premise is verified, not unverified — but the design's option set is wider than first recorded. Premise is covered by current repository evidence; nothing stale. | no |
| 2 | Structural boundary | **Public API change across a crate boundary.** `UsageSummary` (`crates/cyril-core/src/types/`) is `pub` with **public fields** and is built as a struct literal in `crates/cyril-ui/src/widgets/usage_panel.rs:606`, so adding a field breaks that construction and every other literal. `UsageSnapshot` also crosses into `cyril-ui` via `traits.rs:583`. Placement decision required and now made (see `spec.md`): `SUMMARY_COLUMNS` (`usage.rs:1722`) is one shared const consumed by **four** query sites — global (`:1216`), grouped-by-column (`:1227`), a joined query (`:1250`), and by-agent_type (`:1282`) — three of which are grouped. Scope was revised to cover grouped rollups, so all four sites are restructured to wrap a `CUME_DIST()` subquery. Reinforcing evidence found during interrogation: **every grouped rollup embeds the same `UsageSummary`** (`NamedUsageGroup`, `ModelUsageGroup`, `AgentUsageGroup` each hold `summary: UsageSummary`), so the new fields reach all of them whether or not they are populated — there is no smaller type-level option. | **yes** |
| 3 | Production-scale risk | Data-volume dimension is real: `usage_turns` has **no retention or pruning** (grep for delete/retention/prune/vacuum finds none; `RECENT_LIMIT = 20` caps only the recent list, `usage.rs:50`). One row per turn accumulates for the life of the install in `usage.sqlite3`. Ranking runs **per group** across an unbounded table, and the rollup groups by model, provider, agent_type and folder; `CUME_DIST() OVER (PARTITION BY …)` sorts every partition. Needs a budget and a large-table fixture, which Local has none of. Budget pinned in `spec.md`: ≤ 250 ms added to `snapshot()` at 100,000 rows across ≥ 20 `(provider, model)` groups. | **yes** |
| 4 | Explicit behavior | Not fully explicit. Unresolved decisions, each changing observable output: (a) percentile definition — nearest-rank vs linear interpolation (they differ on the same data); (b) `ttft_ms` is `Option`/nullable, so whether NULL rows are excluded before ranking or counted; (c) whether p90 appears in the grouped rollups (model/provider/agent_type/folder) or only the global summary; (d) small-n behavior — what p90 means for n=1 or n<10, and whether it is `None` below a threshold; (e) whether `max` is added alongside p90, since `/stats` shows avg/p90/max and the summary currently has neither p90 nor max for latency. | **no** |

Unknown tests: none

## Selected route

**Structural** — T1 no (premise verified during routing), T2 yes: a public-field struct crossing into `cyril-ui` plus a placement decision on the shared `SUMMARY_COLUMNS`; T3 yes reinforces it. Precedence Empirical > Structural > Local selects Structural.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — T4 `no`: five unresolved behavior decisions listed above |
| evidence.md, probe.* | prove-it-prototype | N/A — T1 `no`: the sole external premise was verified during routing |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no `FAIL` in checkpointed-build's recorded gate.
