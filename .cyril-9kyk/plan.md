# Plan: p90 and max latency in the usage summary

Design: `.cyril-9kyk/design.md` (approved 2026-08-28, verbatim "yes", risk acceptances: None).
Design verification (step 1): Falsification table complete with no empty cells; cheapest falsifier C1 `PASS` (C2 also `PASS`); no row `FAIL`; all eight `PENDING` rows name `checkpointed-build, per-slice gate`; Approval holds verbatim words, date, and `None`. Every row is specific enough to plan against.

## Partition arithmetic

| Slice | Diff estimate (impl + tests + fixtures) |
|---|---|
| 1 — types, mapper, column positions | 220 |
| 2 — percentile computation, all four sites | 420 |
| 3 — scale budget fence | 120 |
| **Sum** | **760** |

Churn margin: **+35% (266 lines)**. Rationale: slice 2 restructures four call sites that share one column-list const and a single positional row mapper, so an off-by-one in `summary_from_row`'s offset arithmetic forces edits across every rollup test at once; historically this shape of change drifts more than a localized addition. Margin is set above a routine 15–20% for that reason.

**Total projected: 760 + 266 = 1,026 changed lines.** At or below the contract's 4,000-line review-size gate, so the plan has **a single PR increment**.

### PR increment A — "usage: p90 and max latency"

- Slices: 1, 2, 3.
- Mergeable definition: merges to the repository's default/upstream branch (discovered via `git rev-parse --abbrev-ref origin/HEAD`, not hard-coded) with `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` green.
- Verified without later increments: there are none; the increment is self-contained. Internally, slice 1 verifies against types and existing rollup tests, slice 2 against fixtures with an independent Rust oracle, slice 3 against a generated large fixture.

---

## Slice 1: Add the four latency fields and pin their column positions without computing them

**Claim IDs:** C7, C8, C10
**Expected behavior:** `UsageSummary` carries `p90_duration_ms`, `max_duration_ms`, `p90_ttft_ms`, `max_ttft_ms` as `Option<f64>`, all `None`, on the overview and on every grouped rollup. Every pre-existing summary field still reports its correct value at every one of the four query sites.
**Oracle:** A hand-computed expected `UsageSummary` literal built in the test from the fixture without executing the rollup SQL (design row C7).
**Stress fixture:** A fixture whose every summary field holds a distinct value (requests 7, successes 5, cancelled 1, errors 1, distinct token and duration totals), so any positional misalignment in `summary_from_row` surfaces as a wrong field rather than a coincidental match. Expected outcome, written now: the overview and all three grouped rollups each return that exact literal with the four new fields `None`.
**Regression fence:** `crates/cyril-core/src/usage.rs` — `usage::tests::all_rollup_sites_map_every_summary_field`; `crates/cyril-ui/src/widgets/usage_panel.rs` — `cyril_ui::tests::ui_does_not_compute_percentiles`; `crates/cyril-core/src/usage.rs` — `usage::tests::tool_usage_group_has_no_latency_fields`
**Named mutation:** C7 — in `usage.rs`, append the four new columns to `SUMMARY_COLUMNS` for the overview query only, leaving grouped sites emitting them at a different index; expected red: a grouped rollup reports a token or cost count where a latency belongs. C8 — in `cyril-ui/src/widgets/usage_panel.rs`, add a local function that sorts durations and indexes at 0.9; expected red: the grep assertion fails. C10 — in `types`, add `p90_duration_ms: Option<f64>` to `ToolUsageGroup`; expected red: the exhaustive construction fails to compile.
**Complexity/production scale:** `N/A — reason: no new loop; this slice adds four constant `NULL` columns to an existing aggregate query and four `Option<f64>` reads to the existing row mapper.`
**Wall budget/phase:** `N/A — reason: no new runtime phase; the four constant columns do not change `snapshot()`'s cost class.`
**Files:** `crates/cyril-core/src/types/usage.rs` (or the module declaring `UsageSummary`); `crates/cyril-core/src/usage.rs` (`SUMMARY_COLUMNS`, `summary_from_row`); `crates/cyril-ui/src/widgets/usage_panel.rs` (struct literals + new guard test); `crates/cyril-ui/src/floor_tests.rs` (struct literal)
**Estimate:** 2h
**Diff estimate:** 220
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-core usage::tests::all_rollup_sites_map_every_summary_field` → the overview and all three grouped rollups each equal the hand-built expected literal field-for-field, with the four new fields `None`
- `cargo test -p cyril-ui ui_does_not_compute_percentiles` → no percentile computation site found in `crates/cyril-ui/src/`, and `cyril-ui/Cargo.toml` declares no statistics dependency
- `cargo test -p cyril-core tool_usage_group_has_no_latency_fields` → compiles and passes, proving `ToolUsageGroup`'s field set is unchanged
- Apply C7's named mutation, re-run the first command → the grouped rollup assertion goes red naming a mismatched field; restore → green
- `cargo clippy --workspace -- -D warnings` → no warnings

---

## Slice 2: Compute nearest-rank p90 and max per group across all four rollup sites

**Claim IDs:** C1, C2, C3, C4, C5, C6
**Expected behavior:** Every `UsageSummary`-carrying rollup reports p90 at 1-based position `ceil(0.9 × N)` and max over its own rows; `ttft_ms` ranks over non-NULL rows only; NULL grouping keys form their own group; absent data yields `None`, never `Some(0.0)`; a one-row group reports that row's value.
**Oracle:** Rust `oracle(v) = { v.sort_unstable(); v[ceil(0.9·len)-1] }` over an explicitly-built `Vec<i64>` — an in-memory sort with no SQL involvement, and no shared code with the production path (design rows C1–C6).
**Stress fixture:** Group `a` = durations `[1..9, 100]` with `ttft_ms` NULL on four rows and `[50,60,70,80,90,1000]` on six; group `b` = durations `[50,60]` (n=2, where `ceil(p·N)` and `floor((N-1)·p)` disagree — 60 vs 50); a group whose `provider IS NULL`; a group of exactly one row; and a duplicate-heavy group `[5,5,5,5,9]`. Expected outcomes written now: `a` p90 9 / max 100, ttft p90 1000; `b` p90 60 / max 60; NULL-provider group present with its own p90; single-row group p90 == max == its value; duplicate group p90 9.
**Regression fence:** `crates/cyril-core/src/usage.rs` — `usage::tests::p90_matches_sorted_oracle_per_group` (C1), `ttft_p90_excludes_nulls_from_denominator` (C2), `grouped_p90_is_group_local` (C3), `null_group_key_gets_its_own_p90` (C4), `absent_latency_data_is_none_not_zero` (C5, with its positive control), `single_row_group_reports_its_own_value` (C6)
**Named mutation:** C1 — change the predicate `cd >= 0.9` to `cd > 0.9`; expected red: group `b` yields `None` instead of 60. C2 — delete `WHERE ttft_ms IS NOT NULL` from the ttft subquery; expected red: asserted 1000, got 90. C3 — drop `PARTITION BY` from the `CUME_DIST()` OVER clause; expected red: every group reports the global p90 (100). C4 — change the percentile subquery key to `WHERE provider IS NOT NULL`; expected red: NULL-provider group reports `None` despite holding rows. C5 — replace the `Option<f64>` mapping with `unwrap_or(0.0)`; expected red: asserted `None`, got `Some(0.0)`. C6 — add `HAVING COUNT(*) >= 10` to the percentile subquery; expected red: single-row group reports `None`.
**Complexity/production scale:** One new sort per partition from `CUME_DIST() OVER (PARTITION BY … ORDER BY …)`, plus a second for the filtered `ttft_ms` subquery: **O(n log n)** in total rows per grouping dimension, evaluated once per rollup. Production-scale input: 100,000 rows across ≥ 20 `(provider, model)` groups. Resulting bound: the two ordered passes dominate, giving ~2·O(n log n) per dimension. **Maximum accepted cost: ≤ 250 ms added to `snapshot()` at that scale**, the value approved in `spec.md`'s quantitative criterion; rationale: it is the bound the requester signed off and the one C9's fence asserts.
**Wall budget/phase:** **always-on.** `snapshot()` runs from `refresh_usage_panel_from_log()` (`crates/cyril/src/app.rs:936`), which fires on every usage-record append (`:1016`), every context sample (`:1041`), and every sidecar enrichment (`:968`) — each guarded by `has_usage_panel()`. So while the usage panel is open, ordinary operation re-runs the rollup repeatedly during a turn, not once per command. **Wall-clock budget at production scale: ≤ 250 ms**, per the approved criterion. Rationale and its limit: 250 ms is a ceiling at the 100,000-row worst case, and a typical log is orders of magnitude smaller, so the realistic per-refresh cost is single-digit milliseconds; the growth that would push a real install toward the ceiling is unbounded-table growth, tracked at **cyril-b163**. This slice does not add debouncing or snapshot caching — see Deferred below.
**Files:** `crates/cyril-core/src/usage.rs` (`SUMMARY_COLUMNS`, the four query sites, the ttft subquery, new tests and fixtures)
**Estimate:** 5h
**Diff estimate:** 420
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-core p90_matches_sorted_oracle_per_group` → group `a` p90 9 / max 100 and group `b` p90 60 / max 60, each equal to the Rust sorted oracle item-by-item
- `cargo test -p cyril-core ttft_p90_excludes_nulls_from_denominator` → ttft p90 1000, equal to the oracle over the six non-NULL values
- `cargo test -p cyril-core grouped_p90_is_group_local` → `a` and `b` differ from each other and from the overview
- `cargo test -p cyril-core null_group_key_gets_its_own_p90` → the NULL-provider group is present with its own p90
- `cargo test -p cyril-core absent_latency_data_is_none_not_zero` → positive control first reports `Some`, then the empty and all-NULL cases report `None`
- `cargo test -p cyril-core single_row_group_reports_its_own_value` → p90 == max == that row's duration
- Apply each of the six named mutations in turn, re-run the owning test → the stated red output; restore → green
- `cargo clippy --workspace -- -D warnings` → no warnings

---

## Slice 3: Fence the grouped-percentile cost at production scale

**Claim IDs:** C9
**Expected behavior:** With 100,000 rows across ≥ 20 `(provider, model)` groups, the latency statistics add at most 250 ms to `UsageLog::snapshot()`.
**Oracle:** Wall-clock measurement of two variants in the same process — `snapshot()` with the statistics and a control path without them — after an identical warm-up query. Independent of the correctness oracle (a timer, not a computation).
**Stress fixture:** A generated log of 100,000 rows spread across 20 `(provider, model)` pairs with skewed group sizes (one group holding ~40% of rows, the rest sharing the remainder) so a per-partition sort cannot be flattered by uniform groups; `ttft_ms` NULL on ~40% of rows so the filtered subquery does real work. Expected outcome written now: measured delta ≤ 250 ms.
**Regression fence:** `crates/cyril-core/src/usage.rs` — `usage::tests::grouped_percentile_stays_within_budget`, a deterministic assertion of the measured bound
**Named mutation:** In `usage.rs`, replace the single grouped percentile subquery with a correlated per-row subquery; expected red: the measured delta exceeds 250 ms.
**Complexity/production scale:** No new production loop — this slice adds only test-side fixture generation. The cost it measures is slice 2's. Fixture generation is **O(n)** in rows at 100,000 rows; **maximum accepted cost: 30 s** for generation, rationale: it must stay inside a routine `cargo test` run without a separate profile.
**Wall budget/phase:** `N/A — reason: one-off phase; the fixture generator and the measurement run only inside this test, never in ordinary operation.`
**Files:** `crates/cyril-core/src/usage.rs` (timed test + fixture generator)
**Estimate:** 2h
**Diff estimate:** 120
**PR increment:** A
**Commands and expected results:**
- `cargo test -p cyril-core grouped_percentile_stays_within_budget -- --nocapture` → prints both measured timings and asserts the delta ≤ 250 ms
- Apply the named mutation, re-run → the assertion goes red reporting a delta above 250 ms; restore → green

---

## Deferred (tracker taxonomy applied)

- **Debouncing or caching `snapshot()` on the always-on refresh path.** Intended future work, conditioned on a trigger (a log large enough that per-refresh cost becomes visible) — **cyril-b163** covers the root cause: `usage_turns` grows without bound, and retention is what keeps a real install away from the 100,000-row ceiling this plan budgets against. Verified: cyril-b163 exists, is open, and its scope names the retention policy plus the aggregation-honesty constraint.
- **Per-request granularity** — permanent non-goal, rationale recorded in `design.md`; no tracker issue beyond the existing cyril-uvrf.
- **p50 / p95 / p99 and interpolated percentiles** — permanent non-goals, rationale recorded in `design.md`.

## Self-review

1. Every design row assigned to exactly one slice — C7/C8/C10 → slice 1; C1–C6 → slice 2; C9 → slice 3. All ten claim IDs exist in the design table; each `PENDING` falsifier is discharged by the slice implementing its claim. **Holds.**
2. All thirteen mandatory fields present on every slice, conditional cells as `N/A — reason`. **Holds.**
3. Every claim's fence is created in the slice implementing it; every fence carries its named mutation from the design; no claim is fence-less, so no `N/A — approved risk` appears. **Holds.**
4. Slice 2's new loop states asymptotic cost, production-scale size, resulting bound, and an explicit maximum accepted cost with rationale; its always-on phase records a wall budget with rationale; slice 3's one-off phase is classified. **Holds.**
5. Partition rule applied: sum 760, documented 35% churn margin (266), total 1,026, at or below 4,000 → single increment A, with a mergeable definition and branch discovery rather than a hard-coded remote. Every slice names increment A. **Holds.**
6. Tracker taxonomy applied to every deferral phrase; the one intended-future-work item cites verified cyril-b163. **Holds.**
7. The plan declares no slice complete; completion is `checkpointed-build`'s to judge. **Holds.**
