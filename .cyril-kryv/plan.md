# Budgeted plan: cyril-kryv

## Inputs and partition

Approved inputs: `.cyril-kryv/route.md`, `.cyril-kryv/spec.md`, `.cyril-kryv/design.md` (`Requester approval: "Approve design"`, 2026-08-21). Repository upstream was discovered as `origin/main`; this stacked change is based on the required Phase 1 implementation at `feat/cyril-gfkm-usage-observer` / `f513e45` because that dependency is not yet on upstream.

Projected slice diffs:

- Slice 1: 700 lines
- Slice 2: 1,100 lines
- Slice 3: 1,200 lines
- Slice 4: 1,000 lines
- Slice 5: 600 lines
- Slice 6: 900 lines
- Sum: 5,500 changed lines
- Churn margin: 20% = 1,100 lines. Rationale: three existing public domain interfaces, a live SQLite schema, and the App/UI wiring all require caller/test migrations; fixture-driven Rust changes in this repository routinely add more fence code than implementation.
- Review budget: 6,600 changed lines.

The total exceeds 4,000, so the plan has three independently mergeable increments:

1. **Live metering substrate** — Slices 1–2, projected 1,800 lines. Mergeable onto the Phase 1 dependency branch: KAS/v2 metering becomes typed and durable, the existing modal still compiles and standard-ACP snapshots remain unchanged. Verified without later slices by conversion, observer, migration, and Phase 1 equivalence fences.
2. **Usage detail and enrichment** — Slices 3–4, projected 2,200 lines. Mergeable after increment 1: detailed tool/context/compaction aggregation and bounded current-session enrichment are durable and queryable even before the new UI page lands. Verified without increment 3 by raw SQLite, parser, attribution, and scale fences.
3. **Account and modal completion** — Slices 5–6, projected 1,500 lines. Mergeable after increment 2: async KAS account state and the complete Costs/Tools/Context presentation ship together with App refresh wiring and live acceptance.

## Slice 1: Type and convert Kiro metering without lifecycle confusion

**Claim IDs:** C1, C2

**Expected behavior:** Captured KAS `turn_completion` and existing v2 metadata produce validated typed non-money charges, status/request/tool metadata, and explicit metric availability; standard ACP money/tokens remain unchanged and KAS metering never completes a turn.

**Oracle:** Python's direct raw-JSONL tuple extraction for C1; a literal adapter/domain variant matrix for C2, independent of Rust conversion and persistence.

**Stress fixture:** A synthetic presence/type matrix: absent/empty/multi unlike units, zero/negative/non-finite usage, missing/unknown status, absent/empty/1/3 request IDs, duplicate/Unicode used-tools. Expected: valid fields preserve exact values; invalid fields warn/drop without defaults; unlike units stay distinct; no cell emits `TurnCompleted`.

**Regression fence:** `crates/cyril-core/src/protocol/convert/kas.rs::tests::captured_turn_completion_maps_exactly_and_is_not_terminal` and `crates/cyril-core/src/usage.rs::tests::metric_source_matrix_preserves_typed_values_and_absence`, created in this slice.

**Named mutation:** In `convert/kas.rs`, map `turn_completion` to `turn_end` or drop frame four's third request ID; C1 turns red. In `types/usage.rs`, encode credits as `Money("USD")` or default absent tokens to zero; C2 turns red.

**Complexity/production scale:** KAS conversion is O(S + U + R), where S is prompt summaries, U used-tool names, and R request IDs. Stress 10,000 total elements (orders above captures); accepted maximum is 25 ms and O(total input string bytes) allocation, because conversion runs on the inbound turn path and must stay below one 50 ms fast-render tick.

**Wall budget/phase:** Always-on once per Kiro turn; ≤25 ms at the 10,000-element stress size for the reason above.

**Files:** `crates/cyril-core/src/types/usage.rs`, `crates/cyril-core/src/types/session.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/types/mod.rs`, `crates/cyril-core/src/protocol/convert/kas.rs`, `crates/cyril-core/src/protocol/convert/kiro.rs`, `crates/cyril-core/src/session.rs`, `crates/cyril-core/tests/fixtures/kas/session_info_update_turn_completion.json`.

**Estimate:** 1 implementation day.

**Diff estimate:** 700 changed lines: 320 implementation, 330 tests, 50 fixture.

**PR increment:** Live metering substrate.

**Commands and expected results:**
- `cargo test -p cyril-core --features kas captured_turn_completion_maps_exactly_and_is_not_terminal` → all four captured tuples equal the Python oracle, including request counts `[2,2,2,3]`, retries `[1,1,1,2]`, and no lifecycle completion.
- `cargo test -p cyril-core metric_source_matrix_preserves_typed_values_and_absence` → every typed-value/availability cell matches the literal matrix; unlike units remain separate.
- Apply each named mutation, rerun its focused fence, restore, rerun → the mutation is red for the named claim and the restored implementation is green.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings` → both default and KAS workspaces remain behaviorally green with no warning suppression.

## Slice 2: Persist one neutral turn with outcomes, requests, and Phase 1 parity

**Claim IDs:** C3, C7, C11

**Expected behavior:** The observer records exactly one turn at the true lifecycle terminal with client timing, typed charge/availability, success/cancel/error, and optional provider requests; a versioned SQLite migration preserves every legacy Phase 1 value and neutral summaries never combine unlike units.

**Oracle:** Hand-authored event timelines/outcome tables for C3; raw `PRAGMA`/`SELECT` inspection and test-local reductions for C7; the Phase 1 Python capture oracle and before/after snapshot tuple for C11.

**Stress fixture:** Fresh and loaded sessions with v2/KAS event orderings, thought/tool before first text, no text, abort/cancel/error, absent/many request IDs, future standard usage on a credit turn; a copied populated v1 database migrated twice with unlike credits/currencies and forced transaction failure. Expected: one row per turn, exact outcomes/timing, idempotent migration, old snapshots unchanged.

**Regression fence:** `usage::tests::kiro_turn_timeline_matrix_records_once`, `usage::tests::v1_migration_is_lossless_idempotent_and_enrichment_atomic` (the migration/append portion in this slice; enrichment extension lands in Slice 4), and `usage::tests::phase1_snapshot_is_unchanged_and_observed_wins_coverage`, created in this slice.

**Named mutation:** In `usage.rs`, finish on metering as well as lifecycle or fall back absent request IDs to one; C3 turns red. Set all migrated availability rows to backend-gated or commit child rows outside the parent transaction; C7 turns red. Mark credit-bearing records gated even with observed tokens; C11 turns red.

**Complexity/production scale:** Notification application is O(1) per non-tool frame; completing a turn sorts T distinct calls in O(T log T), at T ≤10,000. SQLite append is O(T) in one transaction. Accepted maximum: 25 ms observer CPU and 250 ms append at 10,000 tools; the append maximum matches the existing busy timeout and keeps persistence failures visible rather than stalling indefinitely.

**Wall budget/phase:** Always-on per turn; observer ≤25 ms and append ≤250 ms at the stress size.

**Files:** `crates/cyril-core/src/types/usage.rs`, `crates/cyril-core/src/usage.rs`, `crates/cyril-core/src/protocol/bridge.rs`, `crates/cyril/src/app.rs`, existing usage tests/fixtures under the same files.

**Estimate:** 2 implementation days.

**Diff estimate:** 1,100 changed lines: 600 implementation, 500 tests/legacy fixture setup.

**PR increment:** Live metering substrate.

**Commands and expected results:**
- `cargo test -p cyril-core kiro_turn_timeline_matrix_records_once` → each v2/KAS timeline yields one row with the literal outcome/request/timing tuple.
- `cargo test -p cyril-core v1_migration_is_lossless_idempotent_and_enrichment_atomic` → migration twice preserves every legacy column/value; append transaction failure leaves no partial row.
- `cargo test -p cyril-core phase1_snapshot_is_unchanged_and_observed_wins_coverage` → captured omp snapshot is field-for-field equal and synthetic Kiro standard usage is observed, not gated.
- Apply each Slice 2 mutation and perform red/restore/green for its owning fence.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings` → increment 1 is independently green in both feature configurations.

## Slice 3: Add detailed tools, context, compaction, and bounded aggregation

**Claim IDs:** C4, C8, C12

**Expected behavior:** Tools aggregate per call ID with exact-or-kind identity, failures/chars/last-used/by-model and additive token/money/charge shares; latest context and sample-backed compaction gain persist; a 100,000-turn snapshot remains bounded.

**Oracle:** Test-local call-ID map and rational-share arithmetic for C4; hand-authored context/compaction state table for C8; direct SQL counts/sums and collection-length checks for C12.

**Stress fixture:** Duplicate tool updates, repeated names, exact/fallback identities, failed calls, Unicode JSON, 1/2/3-call shares; scalar/full/malformed/trailing context, missing/failed/equal/lower/higher compactions across two sessions; 100,000 mixed turns with 30 errors and high group cardinality. Expected: no duplicates/lost fallbacks/fabricated gain; shares reconcile; details stay 20/20.

**Regression fence:** `usage::tests::tool_call_instance_attribution_matches_oracle`, `usage::tests::context_and_compaction_state_matrix`, and `usage::tests::kiro_snapshot_remains_bounded_at_100k`, created in this slice.

**Named mutation:** In `usage.rs`, divide shares by unique names or count UTF-8 bytes; C4 turns red. Use absolute compaction difference or clear full breakdown on scalar-only update; C8 turns red. Load per-turn detail vectors into `UsageSnapshot`; C12 turns red.

**Complexity/production scale:** Per notification tool map update O(1) average; snapshot SQL is O(N + G log G) in SQLite for N=100,000 rows and genuine group cardinality G, retaining O(G + 40) Rust rows. Explicit maximum accepted snapshot wall cost: 2 seconds for 100,000 turns, inherited from the approved Phase 1 scale oracle. Context/compaction state is O(active sessions) with O(1) updates.

**Wall budget/phase:** Tool/context observation is always-on and ≤25 ms per 10,000-call completion; `/usage` snapshot is one-off per command with `N/A — reason: one-off phase; its 2-second production-scale maximum is enforced under Complexity`.

**Files:** `crates/cyril-core/src/types/usage.rs`, `crates/cyril-core/src/usage.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/convert/kiro.rs`, `crates/cyril/src/app.rs`.

**Estimate:** 2 implementation days.

**Diff estimate:** 1,200 changed lines: 600 implementation, 600 tests/stress fixtures.

**PR increment:** Usage detail and enrichment.

**Commands and expected results:**
- `cargo test -p cyril-core tool_call_instance_attribution_matches_oracle` → calls/errors/Unicode-character totals and every share equal the independent per-call map.
- `cargo test -p cyril-core context_and_compaction_state_matrix` → every state row matches the literal retain-last/gain table; missing/increased samples yield no gain.
- `cargo test -p cyril-core kiro_snapshot_remains_bounded_at_100k -- --ignored` → direct SQL totals match, recent/errors are ≤20, and release-mode snapshot completes within 2 seconds.
- Apply each Slice 3 mutation and perform red/restore/green for its owning fence.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings` → the workspace remains green after the detail schema/observer cutover.

## Slice 4: Enrich only the current Kiro session within hard bounds

**Claim IDs:** C5, C6

**Expected behavior:** A sequential worker snapshots new/loaded session cursors, parses only appended/current-turn v2 or KAS sidecar data, retries at most 3 times/1 second and 64 MiB, validates paths, and atomically updates exact tools and billed identity on the existing record; every failure preserves portable live data.

**Oracle:** Raw fixture byte sizes/line offsets, a controlled clock, literal requested/billed identity table, and direct SQLite row counts from a second connection; production uses independent worker/parser/query paths.

**Stress fixture:** Tempdir v2 JSON+JSONL and KAS workspace-hash JSONL for new/loaded sessions, prior history, absent/late/partial/malformed/permission denied, traversal session IDs, Unicode payloads, 64 MiB boundary and over-cap, repeated `/usage` attempt, forced replacement failure. Expected: only the post-cursor current turn enriches; no escape/over-read/duplicate/partial replacement; billed-else-requested grouping exact.

**Regression fence:** `usage::kiro_sidecar::tests::bounded_current_turn_enrichment_matrix`, the completed enrichment branch of `usage::tests::v1_migration_is_lossless_idempotent_and_enrichment_atomic`, and `usage::tests::billed_model_wins_grouping_matrix`, created in this slice.

**Named mutation:** In `usage/kiro_sidecar.rs`, initialize a loaded cursor at zero or advance it before a successful parse; C5 imports history/cannot retry and turns red. In `usage.rs`, group only requested model columns; C6's billed group disappears.

**Complexity/production scale:** KAS streaming is O(appended bytes) memory O(longest line); v2 monolithic parsing is O(file bytes) memory O(file bytes), both hard-capped at 64 MiB. At most 3 attempts within 1 second per turn, sequential per process. Maximum accepted resource cost: ≤64 MiB input plus parser overhead ≤192 MiB peak RSS and ≤1 second elapsed; rationale is the requester-approved cap/retry contract and nonblocking worker isolation.

**Wall budget/phase:** Always-on asynchronous once per Kiro turn; ≤1 second elapsed and ≤3 attempts. It never occupies the terminal event loop.

**Files:** create `crates/cyril-core/src/usage/kiro_sidecar.rs`; modify `crates/cyril-core/src/usage.rs`, `crates/cyril-core/src/types/usage.rs`, `crates/cyril/src/app.rs`; add v2/KAS sidecar fixtures under `crates/cyril-core/tests/fixtures/usage/`.

**Estimate:** 2 implementation days.

**Diff estimate:** 1,000 changed lines: 500 implementation, 500 tests/fixtures.

**PR increment:** Usage detail and enrichment.

**Commands and expected results:**
- `cargo test -p cyril-core bounded_current_turn_enrichment_matrix` → every path/format/timing/cap cell matches the raw byte/cursor oracle; no prior row imports.
- `cargo test -p cyril-core billed_model_wins_grouping_matrix` → every identity row groups billed-else-requested exactly.
- Apply loaded-cursor, early-advance, and requested-only mutations; owning fences turn red, then green after restore.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings` → increment 2 is independently green with path/error branches covered.

### Execution amendment: merge Slices 5–6 at their shared interface

Checkpointed-build found that account query state (Slice 5) and typed
Costs/Context rendering (Slice 6) mutate the same public `UsagePanelState`,
`Notification`, `CommandResultKind`, `UiState`, and App dispatch interface.
Landing either alone requires a temporary state/notification path that the
approved clean-cutover design forbids. Treat the two sections below as one
atomic final slice with Claim IDs C9, C10, and C13; its gate is the union of
both sections' fixtures, oracles, budgets, fences, and mutations. The final PR
increment and 1,500-line projection are unchanged.

## Slice 5: Query KAS account usage without blocking or stale masquerade

**Claim IDs:** C9

**Expected behavior:** `/usage` opens from SQLite immediately, dispatches exactly one response-carrying account request only under KAS, and applies typed success/failure to last-known in-process account state without reopening a closed modal or changing turn errors.

**Oracle:** Fake bridge send/order counter and literal no-session/v2/KAS/success/failure/late/reopen state table; raw fixture response values are compared item-by-item.

**Stress fixture:** No session, v2, KAS, two rapid opens, modal close before response, success false, data absent, empty/multi usage breakdowns/bonus credits, invalid numbers/units/date, failure after prior success. Expected: one request/open only for KAS, local render first, invalid fields visibly fail without defaults, last-known freshness preserved.

**Regression fence:** `app::tests::usage_account_query_order_and_state_matrix` plus `protocol::convert::kas::tests::account_usage_response_maps_exactly`, created in this slice.

**Named mutation:** In `commands/builtin.rs`, await the account response before returning ShowUsage or resolve the source as KAS for v2; the ordering/count matrix turns red.

**Complexity/production scale:** Response conversion is O(B + C), usage breakdowns plus bonus credits; stress 10,000 entries with ≤25 ms conversion and O(input) storage. State retains one latest response, never history.

**Wall budget/phase:** `N/A — reason: one-off asynchronous query per explicit /usage command; modal open has no response wait`. Conversion itself must stay ≤25 ms at the 10,000-entry stress size.

**Files:** `crates/cyril-core/src/types/usage.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/convert/kas.rs`, `crates/cyril-core/src/protocol/bridge.rs`, `crates/cyril-core/src/commands/builtin.rs`, `crates/cyril-core/src/commands/mod.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/traits.rs`, `crates/cyril/src/app.rs`.

**Estimate:** 1 implementation day.

**Diff estimate:** 600 changed lines: 300 implementation, 300 tests/fixture.

**PR increment:** Account and modal completion.

**Commands and expected results:**
- `cargo test -p cyril-core --features kas account_usage_response_maps_exactly` → plan/reset/usage/bonus/overage values equal the raw response and every malformed branch returns a typed error.
- `cargo test -p cyril --features kas usage_account_query_order_and_state_matrix` → local snapshot precedes one KAS request/open; v2/no-session sends zero; late/failure rows match the state oracle.
- Apply the blocking/wrong-source mutation; the ordering/count fence turns red, then green after restore.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings` → response-carrying bridge and callers are green.

## Slice 6: Render typed Costs, detailed Tools, Context, and refresh wiring

**Claim IDs:** C10, C13

**Expected behavior:** The nine-page modal visibly separates credits/money, names backend-gated metrics, shows provider requests/retries/outcomes, detailed tools and combined context/compaction state, refreshes without unwanted page reset, protects input at 60×16, and contains no Kiro/provider decision outside adapters.

**Oracle:** Ratatui `TestBackend` literal labels/cell coordinates and page/scroll state table; a file ownership/source allowlist independently scans forbidden decision strings.

**Stress fixture:** Empty, Kiro-only, standard-only, mixed, multi-unit, account loading/fresh/stale/error, full/malformed-absent context, detailed/fallback tools, long Unicode labels, all nine pages at 60×16, refresh while scrolled. Expected: required labels/data visible, no charge mixing/input overwrite/page reset, fallback tools retained.

**Regression fence:** `usage_panel::tests::kiro_full_mixed_pages_render_at_floor` and `usage::tests::usage_layers_are_engine_neutral`, created in this slice.

**Named mutation:** In `widgets/usage_panel.rs`, render backend gating as an em dash or append credits to monetary totals; C10 turns red. Branch on provider `"kiro"` in renderer/aggregation; C13 source ownership fence turns red.

**Complexity/production scale:** New Context/account sections are O(1) plus O(B) account breakdowns; Tools retains existing O(G) genuine group rendering. No new per-turn history vector. Maximum accepted render cost: one 60×16 or 120×40 TestBackend frame ≤16 ms with 10,000 aggregate groups, maintaining terminal responsiveness; rows outside the viewport must not allocate presentation strings eagerly if this budget fails.

**Wall budget/phase:** Always-on each redraw while modal is open; ≤16 ms at the 10,000-group stress size so the renderer stays within a 60 Hz frame.

**Files:** `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/widgets/usage_panel.rs`, `crates/cyril-ui/src/widgets/mod.rs`, `crates/cyril/src/app.rs`, relevant colocated tests.

**Estimate:** 2 implementation days.

**Diff estimate:** 900 changed lines: 450 implementation, 450 state/render/structural tests.

**PR increment:** Account and modal completion.

**Commands and expected results:**
- `cargo test -p cyril-ui kiro_full_mixed_pages_render_at_floor` → every snapshot/page state contains the literal typed/backend/context/tool labels and writes no cell at or below input top.
- `cargo test -p cyril-core usage_layers_are_engine_neutral` → the allowlist reports no engine/provider decision in aggregation/render files.
- Apply em-dash/charge-mixing and provider-branch mutations; C10/C13 fences turn red, then green after restore.
- Run the actual TUI against one v2 and one KAS turn and open `/usage` → credits, outcomes, timing, requests/retries where present, context/tools/model/folder, account status, and backend-gated labels are visible; existing omp smoke values remain unchanged.
- `cargo test && cargo test --features kas && cargo clippy -- -D warnings && cargo clippy --features kas -- -D warnings && cargo fmt --check` → increment 3 and the complete issue are green.

## Tracker taxonomy

- Permanent exclusions are fixed by the approved spec/design: no historical backfill, OTLP dependency, synthetic metrics, skill inference, or credit-as-currency.
- Intended future work is already tracked and verified: Behavior sentiment cyril-tq2g; remaining focus/governance cyril-0o7e; subscription vocabulary watch cyril-guml; `unsummarized_dropped` cyril-0f4e.

## Self-review

- [x] C1–C13 are each assigned exactly once; every PENDING design row is discharged by its owning slice.
- [x] Every slice has all thirteen mandatory fields and every conditional field has an explicit `N/A — reason` where applicable.
- [x] Each claim's permanent fence and mechanical mutation land in the same slice.
- [x] Every new loop records asymptotic cost, production input, explicit maximum, and every always-on phase has a wall budget.
- [x] Projected 5,500 + 20% churn = 6,600 lines triggered three dependency-ordered, independently mergeable increments.
- [x] Every deferral is a permanent non-goal or cites a verified tracker ID.
- [x] No slice is declared complete; checkpointed-build owns completion.
