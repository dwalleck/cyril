# Design: p90 and max latency in the usage summary

## Route and inputs

| Input | Source | Extraction |
|---|---|---|
| Route | `.cyril-9kyk/route.md` | **Structural** (T1 no — SQLite percentile premise verified during routing, corrected to include window functions; T2 yes — `UsageSummary` public fields cross into `cyril-ui`; T3 yes — unbounded `usage_turns`; T4 no — five unresolved decisions) |
| Behavior set | `.cyril-9kyk/spec.md` | Five given/when/then behaviors: p90+max for duration; p90+max for ttft; absent data → `None`; every `UsageSummary`-carrying rollup reports its own values; tools rollup unaffected |
| Empirical premises | — | `N/A — T1 verdict no`: no `evidence.md`/`probe.*` required. The one external premise (SQLite percentile support) was verified in `route.md` T1 |
| Approval carried in | `spec.md` Approval section | "yes", 2026-08-28 — with the nearest-rank **formula corrected** after this stage's cheapest falsifier (see Falsifier run log); the correction is re-presented for sign-off with this design |

## Input shapes

| # | Shape | Status |
|---|---|---|
| S1 | `usage_turns` empty (0 rows) | Covered — C5 |
| S2 | Single row (n=1) | Covered — C6 |
| S3 | Many rows, all-distinct values | Covered — C1 |
| S4 | Many rows with duplicate values | Covered — C1 (verified `[5,5,5,5,9]` → 9) |
| S5 | `duration_ms` = 0 (boundary) | Covered — C1 |
| S6 | `duration_ms` NULL | `N/A — unreachable`: schema declares `duration_ms INTEGER NOT NULL` (`usage.rs` CREATE TABLE) |
| S7 | `duration_ms` negative | `N/A — unreachable`: both writers are non-negative by construction — the observer's elapsed comes from a monotonic `Instant`, and the KAS path reads `elapsedTime` as `u64` (`convert/kas.rs:393`) |
| S8 | `ttft_ms` NULL on some rows (mixed) | Covered — C2 |
| S9 | `ttft_ms` NULL on every row | Covered — C5 |
| S10 | `ttft_ms` non-NULL on every row | Covered — C2 |
| S11 | Grouping key NULL (`model`, `provider` are nullable TEXT) | Covered — C4 |
| S12 | Grouping key non-NULL (`folder`, `agent_type` are NOT NULL) | Covered — C3 |
| S13 | Exactly one group | Covered — C3 |
| S14 | Several groups with distinct distributions | Covered — C3 |
| S15 | A group holding exactly one row | Covered — C6 |
| S16 | Max scale: 100,000 rows across ≥ 20 `(provider, model)` groups | Covered — C9 |
| S17 | Rows feeding the tools rollup | Covered — C10 |

## Removed-invariant sweep

The change is **not purely additive**. It restructures all four `SUMMARY_COLUMNS` call sites, which puts one existing invariant at risk:

- **Constraint**: every rollup query produces a column list that the single mapper `summary_from_row(row, offset, …)` (`usage.rs:1749`) reads **by positional index**, with `offset` 0 for the overview and 1 for grouped queries (the group key occupies column 0).
- **What it silently guarantees**: adding, reordering, or conditionally emitting a column in one site but not another makes `summary_from_row` read a *different field* at that index. `rusqlite`'s `row.get(i)` is positional, and `INTEGER`/`REAL` columns coerce, so a misalignment does not error — it returns a plausible wrong number.
- **Now possible**: wrapping three of four sites in a `CUME_DIST()` subquery while leaving the fourth flat, or appending the four new columns at different positions per site.

This becomes **C7**, phrased as the property that must still hold.

## Placement

**Capability: per-group nearest-rank p90 and max over turn latencies.**

- **Owner** — `cyril-core::usage` (`UsageLog`, `SUMMARY_COLUMNS`, `summary_from_row`) computes; `cyril-core::types::UsageSummary` carries the fields. It wins because every rollup already embeds `UsageSummary`; no other module can add a field to a type it does not own, and no other module holds the SQL.
- **New seam** — none. The capability slots behind the existing `UsageLog::snapshot()` interface; callers see four more fields on a type they already consume.
- **Forbidden**
  - `cyril-ui` must not compute, re-derive, or interpolate percentiles — it renders what `UsageSummary` carries. No statistics dependency may be added to `cyril-ui`.
  - Grouped rollups must not inherit the overview's values.
  - `summary_from_row` must remain the **single** mapper for all four sites; no site may hand-roll its own row mapping.
  - No new column, index, or migration on `usage_turns` (the change is read-side only).
  - No new crate dependency for statistics.

## Claims

- **C1** — SQL nearest-rank p90 and max equal an independent sorted-slice oracle at 1-based position `ceil(0.9 × N)`, for distributions where p90, mean and max differ, including duplicates.
- **C2** — `ttft_ms` p90 and max rank over only non-NULL rows; NULL rows never enter the rank denominator.
- **C3** — Each group's p90/max are computed over that group's rows alone and are never inherited from the overview or a sibling group.
- **C4** — Rows whose grouping key is NULL form their own group and are ranked normally, not dropped.
- **C5** — With zero rows, or with every `ttft_ms` NULL, the corresponding fields are `None` and never `Some(0.0)`.
- **C6** — A group of exactly one row reports p90 equal to that row's value; no minimum-sample threshold suppresses it.
- **C7** — All four rollup queries emit a column layout that `summary_from_row` maps correctly at its declared offset.
- **C8** — `cyril-ui` contains no percentile computation; it only reads the new fields.
- **C9** — At 100,000 rows across ≥ 20 `(provider, model)` groups, the new statistics add ≤ 250 ms to `snapshot()`.
- **C10** — `ToolUsageGroup` gains no fields.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Nearest-rank p90/max match an independent oracle | S3, S4, S5 | Build a fixture with per-group values `a=[1..9,100]`, `b=[50,60]`; assert SQL p90/max equal the oracle per group. Falsified if any group differs. Other causes producing a match: a shared helper computing both sides — excluded because the oracle sorts a `Vec<i64>` in Rust with no SQL involvement; and coincidental agreement at n=10 — excluded by including n=2, where the two candidate formulas disagree (60 vs 50) | Rust `oracle(v) = { v.sort(); v[ceil(0.9·len)-1] }` — different mechanism (in-memory sort, not SQL window) | In `usage.rs`, change the percentile predicate from `cd >= 0.9` to `cd > 0.9`. Expected red: group `b` yields NULL/None instead of 60 | `usage::tests::p90_matches_sorted_oracle_per_group` | minutes | **PASS** |
| C2 | ttft ranks over non-NULL rows only | S8, S10 | With group `a` holding 4 NULL and 6 non-NULL `ttft_ms`, assert p90 equals the oracle over the 6 non-NULL values. Falsified if it equals the value obtained with NULLs in the partition. Other cause: `MIN(CASE …)` already skips NULL *values* — excluded because the mutation changes the *denominator*, and the probe shows the two answers differ (1000 vs 90), so the assertion cannot pass for that reason | Rust oracle over the explicitly-filtered 6-element vector | In `usage.rs`, delete `WHERE ttft_ms IS NOT NULL` from the ttft percentile subquery. Expected red: asserted 1000, got 90 | `usage::tests::ttft_p90_excludes_nulls_from_denominator` | minutes | **PASS** |
| C3 | Per-group values are group-local | S12, S13, S14 | Two groups with disjoint distributions; assert each group's p90 equals its own oracle and that the two differ from each other and from the overview. Falsified if any group carries another's value. Other cause: both groups coincidentally equal — excluded by choosing `a`→9 and `b`→60 | Per-group Rust sorted oracle | In `usage.rs`, drop `PARTITION BY` from the `CUME_DIST()` OVER clause. Expected red: every group reports the global p90 (100) | `usage::tests::grouped_p90_is_group_local` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C4 | NULL group keys form a real group | S11 | Insert rows with `provider IS NULL` alongside non-NULL providers; assert the NULL-keyed group appears with its own p90 equal to its oracle. Falsified if the group is absent or its p90 is NULL while rows exist. Other cause: the existing `GROUP BY` already emits NULL groups — this is the point; the fence guards the new columns joining correctly for that key, so the mutation targets the join, not the grouping | Rust oracle over the NULL-provider subset | In `usage.rs`, change the percentile subquery join/partition key to `WHERE provider IS NOT NULL`. Expected red: NULL-provider group reports `None` p90 despite holding rows | `usage::tests::null_group_key_gets_its_own_p90` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C5 | Absent data yields None, never zero | S1, S9 | Two cases — an empty table, and a table whose every `ttft_ms` is NULL. Assert all four (resp. the two ttft) fields `is_none()`. This asserts **absence**, so it carries a positive control: the same test first inserts data and asserts the fields are `Some`, proving the fields can be populated at all | Direct `Option` inspection in Rust, independent of any SQL aggregate | In `usage.rs`, replace the `Option<f64>` mapping with `unwrap_or(0.0)`. Expected red: asserted `None`, got `Some(0.0)` | `usage::tests::absent_latency_data_is_none_not_zero` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C6 | n=1 reports that value | S2, S15 | A single-row group; assert p90 == max == that row's `duration_ms`. Falsified if `None` or any other value. Other cause: a threshold returning the mean instead — excluded because mean == the value at n=1, so the fixture uses a group of 1 *alongside* a larger group, and asserts the small group's value is its own row, not the other group's | Rust oracle: `ceil(0.9·1)-1 = 0` → the single element | In `usage.rs`, add `HAVING COUNT(*) >= 10` to the percentile subquery. Expected red: single-row group reports `None` | `usage::tests::single_row_group_reports_its_own_value` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C7 | All four sites keep a layout `summary_from_row` maps correctly | S12, S13 | Populate a fixture with values that make every summary field distinct, then assert the overview and all three grouped rollups return the correct value in **every** `UsageSummary` field — not only the new ones. A positional misalignment shifts an unrelated field, so the assertion covers the whole struct. Other cause: a coincidentally-correct read — excluded by making each field's expected value unique | A hand-computed expected `UsageSummary` literal, built in the test from the fixture without executing the rollup SQL | In `usage.rs`, append the four new columns to `SUMMARY_COLUMNS` for the overview query only, leaving the grouped sites' subquery wrapper emitting them at a different index. Expected red: a grouped rollup reports a token or cost count where a latency belongs | `usage::tests::all_rollup_sites_map_every_summary_field` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C8 | cyril-ui computes no percentiles | Placement decision (step 4 Forbidden) | Mechanical: `grep -rE 'CUME_DIST|percentile|\bp90\b.*(sort|cmp)' crates/cyril-ui/src/` returns no computation site, and `cyril-ui/Cargo.toml` gains no statistics dependency. Falsified if either appears. Not "the reviewer will notice": the check is a script assertion | The grep/manifest check runs outside the Rust type system, a different mechanism from the production code path | In `cyril-ui/src/widgets/usage_panel.rs`, add a local function that sorts durations and indexes at 0.9. Expected red: the grep assertion fails | `cyril-ui::tests::ui_does_not_compute_percentiles` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C9 | ≤ 250 ms added at 100k rows / ≥20 groups | S16 | Generate 100,000 rows across 20 `(provider, model)` groups; time `snapshot()` with the new statistics and with them compiled out; assert the delta ≤ 250 ms. Falsified above the bound. Other cause: a warm page cache flattering the run — excluded by timing both variants in the same process after an identical warm-up query | Wall-clock measurement of the two variants; independent of the correctness oracle | In `usage.rs`, replace the single grouped percentile subquery with a correlated per-row subquery. Expected red: the measured delta exceeds 250 ms | `usage::tests::grouped_percentile_stays_within_budget` (deterministic assertion of the measured bound) | ~10 min incl. fixture generation | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |
| C10 | ToolUsageGroup gains no fields | S17 | Assert the existing tools-rollup tests pass unchanged and that `ToolUsageGroup` has no latency field, by constructing it with its current field set in a test that would fail to compile if a field were added. Falsified by a compile error or a changed tools assertion. Other cause: an unrelated tools regression — excluded because the construction check is compile-time and names the struct directly | Compile-time struct construction, a different mechanism from the runtime rollup query | In `types`, add `p90_duration_ms: Option<f64>` to `ToolUsageGroup`. Expected red: the exhaustive construction in the test fails to compile | `usage::tests::tool_usage_group_has_no_latency_fields` | minutes | PENDING — checkpointed-build, per-slice gate (slice assigned in plan.md) |

## Non-goals and future work

**Permanent non-goals** (rationale recorded, no tracker issue):

- **Latency statistics on the tools rollup.** `ToolUsageGroup` carries its own flat field set and no `UsageSummary` member; a turn-level latency has no meaning attributed to a single tool within that turn.
- **p50, p95, p99.** Declined at spec time: p99 needs ~100 samples per group to mean anything, and the avg/p90/max triple is the decision on record.
- **Linear-interpolation percentiles.** Nearest-rank is the approved definition; an interpolated p90 reports a latency no turn took.
- **Per-provider-request granularity.** Unobtainable from the ACP layer — cyril-uvrf.

**Intended future work** (verified tracker IDs):

- **cyril-b163** — `usage_turns` grows without bound; no retention, pruning or vacuum. Filed while writing this design: C9 pins a *budget* at 100k rows, but a budget is not a bound on the table, and per-group ranking now sorts every partition so cost scales with total rows.
- **cyril-uefh** — persist provider request IDs (adjacent; not touched here).

## Falsifier run log

Cheapest falsifier (C1) run before approval, per the stage rule. C2 ran in the same pass at no extra cost.

```
$ cargo test -p cyril-core --test tmp_design_falsifier -- --nocapture     # first attempt
C1 g=a sql_p90=9  oracle_p90=9  sql_max=100 oracle_max=100 MATCH
C1 g=b sql_p90=60 oracle_p90=50 sql_max=60  oracle_max=60  MISMATCH
C1 => FAIL
C2 filtered=1000 oracle=90 unfiltered(buggy)=90
C2 => FAIL
```

**The first run failed, and the defect was in the spec, not the SQL.** A formula sweep over n = 1, 2, 3, 5, 6, 10 showed `CUME_DIST() >= 0.9` implements classical nearest-rank `ceil(p·N)` on all six, while the spec's `floor((N-1)·0.9)` agrees only at n=1 and n=10 — coincidentally, because the worked example in the spec used n=10. `spec.md` was corrected to `ceil(0.9 × N)` and the oracle rebuilt:

```
$ cargo test -p cyril-core --test tmp_falsifier3 -- --nocapture           # after correction
C1 g=a sql_p90=9  oracle=9  sql_max=100 oracle_max=100 MATCH
C1 g=b sql_p90=60 oracle=60 sql_max=60  oracle_max=60  MATCH
C1 => PASS
C2 filtered=1000 oracle=1000 unfiltered(mutation)=90 => PASS
test result: ok. 1 passed; 0 failed
```

Both throwaway test files were removed after each run; the permanent fences named in the table are written during the build.

## Approval

Requester approval (verbatim): "yes"
Date: 2026-08-28
Approved risk acceptances: None — every claim carries a deterministic regression fence; no row uses `N/A — approved risk`.
