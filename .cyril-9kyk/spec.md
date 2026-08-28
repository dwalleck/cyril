# Spec: p90 and max latency in the global usage summary

## Request (verbatim)
> Use the new gilfoyle skill at @../gilfoyle/skills/gilfoyle/SKILL.md and lets do cyril-9kyk

Referenced ticket cyril-9kyk: "Usage aggregation: add p90 (and max) latency percentiles to the summary".

## What this is
`UsageLog::snapshot` currently reports only mean latency (`avg_duration_ms`, `avg_ttft_ms`), so the tail is invisible — a single 300 s stall-watchdog timeout is indistinguishable from a slightly slow session. This change adds nearest-rank p90 and max for both turn duration and time-to-first-token to `UsageSummary`, which the overview **and every grouped rollup** share.

## Roles
- **cyril operator**: the person running the cyril TUI who opens the usage panel (`crates/cyril-ui/src/widgets/usage_panel.rs`). They read the latency line to judge how slow turns have been and whether the tail is pathological. They will see two more statistics beside each existing average.

## Behavior

### p90 and max reported for turn duration
- **Given**: `usage_turns` holds N ≥ 1 rows with non-NULL `duration_ms`
- **When**: `UsageLog::snapshot()` is called
- **Then**: `UsageSummary.p90_duration_ms` is `Some(v)` where `v` is the nearest-rank 90th percentile — the value at **1-based ordered position `ceil(0.9 × N)`** of the ascending `duration_ms` values (zero-indexed `ceil(0.9 × N) - 1`) — and `UsageSummary.max_duration_ms` is `Some(m)` where `m` is the largest non-NULL `duration_ms`. Both are values that some recorded turn actually took.

### p90 and max reported for time-to-first-token
- **Given**: `usage_turns` holds K ≥ 1 rows with non-NULL `ttft_ms` (K may be smaller than N; `ttft_ms` is nullable)
- **When**: `UsageLog::snapshot()` is called
- **Then**: `UsageSummary.p90_ttft_ms` and `UsageSummary.max_ttft_ms` are computed over exactly those K non-NULL values, ranked independently of `duration_ms`

### Absent data reports absence, never zero
- **Given**: `usage_turns` holds zero rows, or every row's `ttft_ms` is NULL
- **When**: `UsageLog::snapshot()` is called
- **Then**: the corresponding fields are `None`. No field is `Some(0.0)` and no field is `Some` of a fabricated value

### Every rollup that carries a UsageSummary reports its own p90 and max
- **Given**: `usage_turns` holds rows spanning at least two distinct groups on a grouping dimension (provider, model, folder, or agent_type)
- **When**: `UsageLog::snapshot()` is called
- **Then**: each group's `summary` carries `p90_*`/`max_*` computed over **only that group's** rows, ranked within the group and not inherited from the overview. Two groups with different latency distributions report different p90 values.

### The tools rollup is unaffected
- **Given**: any contents of `usage_turns` and `usage_tools`
- **When**: `UsageLog::snapshot()` is called
- **Then**: `ToolUsageGroup` exposes exactly the fields it exposes today. It does not carry a `UsageSummary`, so it gains no latency statistics.

## Success criteria

- **Binary / structural**: for a fixture whose p90, mean and max are mutually distinct (e.g. `duration_ms` = 1..9 and 100 → p90 = 9, mean = 14.5, max = 100), `snapshot()` returns exactly `p90 = 9` and `max = 100`, checked by a unit test asserting equality against a sorted-slice oracle computed in Rust independently of the SQL path.
- **Binary / structural**: for an empty `usage_turns` and for an all-NULL-`ttft_ms` table, every new field is `None`, checked by two unit tests asserting `is_none()`.
- **Binary / structural**: for a fixture with two groups whose distributions differ (group `a` = 1..9,100 → p90 9; group `b` = 50,60 → p90 60), each group's `summary.p90_duration_ms` equals its own group oracle and neither equals the overview's, checked by a unit test over provider-grouped rollups.
- **Binary / structural**: `ToolUsageGroup` gains no fields, checked by the existing tools-rollup tests passing unchanged.
- **Quantitative**: with 100,000 rows in `usage_turns` spread across ≥ 20 distinct `(provider, model)` groups, the added p90/max computation increases `UsageLog::snapshot()` wall-clock by ≤ 700 ms, measured by a timed test that runs `snapshot()` against a generated fixture with and without the new statistics.

  **Revised 2026-08-28 from 250 ms, after the build measured the real cost.** The original figure was chosen when writing the spec, not measured. Two facts moved it: (a) the implementation was already rewritten once to fit — from ranked subqueries joined into `SUMMARY_COLUMNS` (~1,100 ms added) to lean sibling queries merged in Rust (600 ms added), so the number is not covering a lazy implementation; (b) the **baseline** `snapshot()` already costs ~342 ms at this scale on the same always-on refresh path, so 250 ms was never achievable by any version of this change. 700 ms is the measured 600 ms plus ~17% headroom. The scale point itself is a worst case that only arises because `usage_turns` has no retention (cyril-b163), and the always-on recompute shape is filed separately as cyril-nanu.

## Out of scope

This change does NOT include: latency statistics on the **tools** rollup (`ToolUsageGroup` carries no `UsageSummary`); p50, p95 or p99; linear-interpolation percentiles; per-provider-request granularity (permanent non-goal, cyril-uvrf); retention or pruning of `usage_turns`; any change to the UI layout beyond rendering the new fields; persisting request IDs (cyril-uefh); any new database column or migration.

## Related issues

- **cyril-9kyk**: this spec implements it. The ticket's "(and max)" is confirmed in scope by the metric-set decision below.
- **cyril-kryv** (closed): established that unavailable metrics render as "n/a (backend-gated)", **never a 0 sentinel**. Adopted for the absent-data behavior.
- **cyril-4h6i** (closed, epic): the usage-observer stage this summary belongs to; establishes the omp-parity panel set and that latency/TTFT are the metrics kiro *does* expose.
- **cyril-uvrf** (open, wontfix): per-request granularity is a permanent non-goal; named in Out of scope so p90 is not later expected per request.
- **cyril-uefh** (open): persisting request IDs. Adjacent, not overlapping — named in Out of scope.
- **cyril-79df** (open): the `turn_completion` **fixture** does not model `requestIds[]`. Adjacent to cyril-uefh, distinct from this change; no bearing beyond confirming the fixture family this spec's tests live in.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Does p90 appear only on the global summary, or also on every grouped rollup? | **Every rollup that carries a `UsageSummary`** — overview, providers, models, folders, agent_types | cyril operator's decision, 2026-08-28, revised same day after two evidence corrections. First recorded as global-only on three grounds; two did not survive checking. (a) *Composition* — the requester asked whether window functions could do this: `CUME_DIST() OVER (PARTITION BY g ORDER BY x)` composes with grouping and returns p90+max+count in one pass (re-probed, see `route.md` T1). (b) *API surface* — every grouped rollup embeds the **same** `UsageSummary` (`NamedUsageGroup`, `ModelUsageGroup`, `AgentUsageGroup`), so the fields land on all of them regardless; global-only would ship them permanently `None` in four of five places, conflating "no data" with "not computed", which cyril-kryv forbids. (c) *Scale* — survives, and is now carried by the budgeted criterion below | `SUMMARY_COLUMNS` and its four call sites are restructured to compute per-group p90/max; the tools rollup is untouched because it carries no `UsageSummary` |
| Which rollups receive the new statistics? | Exactly those embedding `UsageSummary`: overview, providers, models, folders, agent_types — not tools | Derived from the type graph, not a preference: `ToolUsageGroup` has its own flat field set and no `UsageSummary` member | No new type is introduced; the four fields on `UsageSummary` reach every carrier at once |
| Which percentile definition? | Nearest-rank, at 1-based position `ceil(0.9 × N)` | cyril operator's decision, 2026-08-28: always reports a latency some turn actually took; computable in SQL and trivially oracle-checkable by sorting | p90 is an observed sample, never interpolated; `1..9,100` → 9, not 18.1. **Formula corrected 2026-08-28** after the design stage's cheapest falsifier: the spec first said `floor((N-1)×0.9)`, which agrees with `ceil(0.9×N)` only at N=1 and N=10 and diverges at N=2,3,5,6 (e.g. `[50,60]` → 60, not 50). The decision (nearest-rank) is unchanged; only its transcription was wrong |
| Which latency statistics does the summary expose? | p90 + max, for both `duration_ms` and `ttft_ms` (four new fields) | cyril operator's decision, 2026-08-28: matches the avg/p90/max triple v2's `/stats` shows; max bounds the tail that p90 alone hides | `UsageSummary` gains `p90_duration_ms`, `max_duration_ms`, `p90_ttft_ms`, `max_ttft_ms` |
| How are NULL `ttft_ms` rows treated when ranking? | Excluded before ranking | Consistency with existing behavior: `AVG(ttft_ms)` in `SUMMARY_COLUMNS` already ignores NULLs under SQL semantics | p90/max ttft are computed over K non-NULL rows, independent of the N rows used for duration |
| What is reported when there is no data (zero rows, or all `ttft_ms` NULL)? | `None` | Prior art cyril-kryv: unavailable metrics are never a 0 sentinel; also matches the existing `avg_ttft_ms: Option<f64>` | All four fields are `Option<f64>`; absence is representable and distinct from zero |
| Is there a minimum sample count below which p90 is suppressed? | No | Consistency with existing behavior: `avg_*` is reported at n=1 today, and nearest-rank is well-defined for n ≥ 1 | p90 at n=1 equals that single value; no new threshold concept is introduced |
| What time window does the summary cover? | All time (no time predicate) | Consistency with existing behavior: `SELECT {SUMMARY_COLUMNS} FROM usage_turns` (`usage.rs:1216`) applies no time filter today | p90/max cover the entire log, exactly like the existing averages |
| **Edge — empty set (zero rows)** | All four fields `None` | Same rationale as absent-data above | Covered by a dedicated unit test |
| **Edge — max scale** | Budgeted: ≤ 250 ms added to `snapshot()` at 100,000 rows across ≥ 20 `(provider, model)` groups | T3 recorded that `usage_turns` has no retention (`route.md`); the table grows for the life of the install, and per-group ranking sorts every partition | A timed test against a generated grouped 100k-row fixture is a success criterion |
| **Edge — null / missing field** | Excluded before ranking (see NULL row above) | SQL `AVG` consistency | `ttft_ms` ranks over its own non-NULL subset |
| **Edge — concurrent writes** | N/A — `snapshot()` is a read-only aggregation and the existing `BUSY_TIMEOUT` (`usage.rs:51`) already governs contention with the writer; this change adds no new write path | No new concurrency surface | No new decision needed |
| **Edge — permission denied / unauthenticated** | N/A — `usage.sqlite3` is a local file opened by the same process; there is no authentication boundary in the usage log | No auth surface | No new decision needed |
| **Edge — partial failure (one of N succeeded)** | N/A — the statistics come from a single read query that either returns a row or errors; there is no multi-step operation to partially fail | No partial state | No new decision needed |
| **Edge — retries / idempotency** | N/A — `snapshot()` is a pure read with no side effects; repeating it yields the same result for unchanged data | Naturally idempotent | No new decision needed |
| **Edge — soft-deleted records** | N/A — `usage_turns` has no soft-delete column and no delete path exists (`route.md` T3 evidence: no retention or pruning) | Every row is live | No new decision needed |
| **Edge — multi-tenancy boundaries** | N/A — the usage log is a single local file for one operator; there is no tenant dimension | No tenancy surface | No new decision needed |
| **Edge — time-zone / DST** | N/A — `duration_ms` and `ttft_ms` are elapsed-millisecond measurements, not wall-clock instants, so no zone conversion applies. The all-time window applies no date predicate | Zone-independent | No new decision needed |
| **Edge — replication lag** | N/A — single local SQLite file with no replica | No replication | No new decision needed |
| **Edge — cache invalidation** | N/A — `snapshot()` computes from the table on each call; no cached summary is stored | No cache | No new decision needed |

## Approval

Requester approval (verbatim): "yes"
Date: 2026-08-28

Re-approval after revision (verbatim): "yes"
Date: 2026-08-28

Second re-approval after revision (verbatim): "Raise budget + ticket the refresh"
Date: 2026-08-28
Revision covered: the quantitative latency criterion moved from ≤ 250 ms to ≤ 700 ms added at 100,000 rows, after the build measured 600 ms and found ~342 ms of pre-existing baseline cost on the same path. Presented with the measurement table and four options; the requester chose to raise the budget and file the always-on refresh defect separately (cyril-nanu).
Revision covered: the nearest-rank formula was corrected from `floor((N-1)×0.9)` to `ceil(0.9 × N)` after the design stage's cheapest falsifier showed the two agree only at N=1 and N=10. Presented with `design.md` and approved in the same response. The decision (nearest-rank) was never in question — only its transcription.
