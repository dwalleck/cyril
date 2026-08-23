# Budgeted plan: cyril-gfkm

Approved design: `.cyril-gfkm/design.md` (`"Approve design"`, 2026-08-21). Route: Structural.

## Partition arithmetic

| Slice | Diff estimate |
|---|---:|
| 1. Core capture, observer, persistence, aggregation | 1,800 lines |
| 2. Usage modal state and rendering | 700 lines |
| 3. Command/App wiring and live acceptance | 500 lines |
| **Sum** | **3,000 lines** |
| Churn margin | **750 lines (25%)** — notification plumbing and SQLite query row mapping commonly expand during exhaustive callsite/error-path work |
| **Projected total** | **3,750 lines** |

Review-size gate: 3,750 ≤ 4,000, so one PR increment is permitted.

### PR increment: Live usage observer

Slices 1–3. Mergeable definition: the standard ACP bridge captures live per-turn usage into the engine-neutral durable log, `/usage` renders every accepted panel from persisted data, focused tests and the live omp smoke pass, and the repository is green. There is no later increment on which its verification depends.

## Slice 1: Build the engine-neutral core capture, observer, durable log, and rollups

**Claim IDs:** C1, C2, C3, C4, C5, C6, C7, C8, C11, C12, C13, C14, C15

**Expected behavior:** Standard prompt usage and cumulative gauges become validated domain events; the observer emits exactly one record per started turn, including usage-less errors; SQLite atomically persists records/tools; snapshots reproduce the approved formulas and bounded detail sets.

**Oracle:** The committed omp capture plus independent literal/Python arithmetic for C1/C2; hand-authored timelines, identity tables, set/share arithmetic, raw SQL through a second connection, and pure fixture arithmetic from `design.md` C3–C15.

**Stress fixture:** Captured two-turn omp frames; thought/tool-before-text sequence; duplicate tool IDs with a failed update; bare and `provider/model` identities; fresh and loaded session cost sequences; invalid/non-finite money and `u64::MAX`; forced child-row constraint failure; mixed currencies/missing optionals; 100,000 persisted turns with 30 errors. Expected outcomes are the exact C1–C15 values in `design.md`, atomic zero-or-all writes, two currency totals, and at most 20 recent plus 20 errors.

**Regression fence:** `crates/cyril-core/src/protocol/convert/mod.rs::tests::captured_omp_prompt_usage_maps_exactly`; `crates/cyril-core/src/usage.rs::tests::{cumulative_cost_delta_matrix,timing_uses_first_agent_text_only,tool_calls_dedupe_and_shares_add_up,identity_snapshot_matrix,append_is_atomic_across_record_and_tools,overview_matches_independent_omp_formula_oracle,breakdowns_are_identity_agnostic,invalid_values_fail_without_defaulting,snapshot_is_bounded_for_large_history,loaded_session_requires_cost_baseline,fresh_session_attributes_first_cost,usage_less_error_turn_is_persisted}`.

**Named mutation:** C1 swap input/output conversion; C2 store cumulative rather than delta; C3 trigger TTFT on thought; C4 append every update; C5 split a bare model; C6 commit parent before tools; C7 divide cache-read by total; C8 special-case `openai-codex`; C11 default invalid cost to zero; C12 load every record into the snapshot; C13 initialize loaded cost at zero; C14 initialize fresh cost unknown; C15 drop usage-less completions. Each mutation must make its named fence red, then restoring it must return green.

**Complexity/production scale:** Notification correlation is expected O(1) per frame and O(T) pending memory, with T ≤ 100 concurrent observed sessions and ≤1 ms accepted per frame excluding persistence. Tool finalization is O(K) for K distinct calls in one turn, expected K ≤ 100 and ≤1 ms. SQLite append is O(K) in one transaction and must complete within 10 ms for K=100 on the reference workstation. Snapshot SQL scans N matching rows and returns O(G+40) memory, where N=100,000 and G is genuine distinct group count; accepted one-shot wall cost is ≤2 s because `/usage` is explicit and 2 s is the approved responsiveness ceiling, while recent/error details remain capped at 20 each.

**Wall budget/phase:** Observer application and turn append are always-on: ≤1 ms per frame and ≤10 ms per completed 100-tool turn on the reference workstation, so normal streaming remains below a 16 ms frame. Database open/migration and `/usage` snapshot are one-off phases; no always-on wall budget applies to them, while snapshot's explicit one-shot maximum is recorded above.

**Files:** `Cargo.toml`; `crates/cyril-core/Cargo.toml`; create `crates/cyril-core/src/types/usage.rs`; create `crates/cyril-core/src/usage.rs`; modify `crates/cyril-core/src/lib.rs`; `crates/cyril-core/src/types/mod.rs`; `crates/cyril-core/src/types/event.rs`; `crates/cyril-core/src/protocol/convert/mod.rs`; `crates/cyril-core/src/protocol/bridge.rs`; `crates/cyril-core/src/session.rs`; `crates/cyril-ui/src/state.rs` only for exhaustive new-notification no-op handling; reuse `experiments/conductor-spike/omp-usage-update-2turn.jsonl` as the captured fixture.

**Estimate:** 8–12 implementation hours.

**Diff estimate:** 1,800 changed lines including tests; no new copied wire fixture.

**PR increment:** Live usage observer.

**Commands and expected results:**
- `cargo test -p cyril-core captured_omp_prompt_usage_maps_exactly -- --nocapture` → exact T1/T2 tuples and absent-usage branch agree with the captured JSON/Python oracle; C1's swap mutation turns it red, restore returns green.
- `cargo test -p cyril-core usage::tests -- --nocapture` → every C2–C15 fixture agrees item-by-item with its independent oracle; each named mutation localizes to its named test, red under mutation and green restored.
- `cargo test -p cyril-core` → all existing core behavior plus the new core fences pass.
- `cargo clippy -p cyril-core -- -D warnings` → no warnings, no lint suppression.
- Release-mode 100k stress invocation of `snapshot_is_bounded_for_large_history` → correct direct-SQL totals, result detail caps 20/20, and reference-workstation elapsed time ≤2 s.

## Slice 2: Add the complete input-safe usage modal

**Claim IDs:** C9

**Expected behavior:** An immutable `UsageSnapshot` renders as eight pageable overlays—Overview, Costs, Providers, Models, Tools, Recent, Errors, Folders—at roomy and narrow sizes without covering input, with bounded scroll and truthful empty/absent formatting.

**Oracle:** Ratatui `TestBackend` cell coordinates and literal page/data labels, independent of widget layout code.

**Stress fixture:** Empty snapshot; 50 groups; 500-character and Unicode labels; multiple currencies; missing optional metrics; 30×10 and 80×24 terminals with a protected input row. Expected: each page heading and representative value is visible when selected, absent values render as `—`, long values clip safely, scrolling saturates, and no cell at or below `input_top` changes.

**Regression fence:** `crates/cyril-ui/src/widgets/usage_panel.rs::tests::all_pages_render_and_clamp_above_input`; `crates/cyril-ui/src/state.rs::tests::usage_panel_page_and_scroll_state`; `crates/cyril-ui/src/floor_tests.rs::usage_modal_never_covers_input`.

**Named mutation:** C9 change `usage_panel::render` to size against the full frame rather than `input_top`; the floor fence must report an overwritten input row, then pass after restoration.

**Complexity/production scale:** Render loops only over visible rows, O(min(G,H)) with H ≤ terminal rows and G potentially 100,000 distinct keys; state holds snapshot group rows but no per-turn history beyond 40 details. Accepted render cost is ≤5 ms at 240×80, preserving the existing 50 ms active tick with ample margin.

**Wall budget/phase:** Rendering is always-on while the modal is visible: ≤5 ms per 240×80 frame on the reference workstation. Page/scroll transitions are O(1) discrete events and inherit the same next-frame budget.

**Files:** create `crates/cyril-ui/src/widgets/usage_panel.rs`; modify `crates/cyril-ui/src/widgets/mod.rs`; `crates/cyril-ui/src/traits.rs`; `crates/cyril-ui/src/state.rs`; `crates/cyril-ui/src/render.rs`; `crates/cyril-ui/src/floor_tests.rs`; `crates/cyril-ui/src/theme.rs`; `crates/cyril-ui/tests/widget_theme_sources.rs` if the existing source inventory requires explicit registration.

**Estimate:** 4–6 implementation hours.

**Diff estimate:** 700 changed lines including render/state tests.

**PR increment:** Live usage observer.

**Commands and expected results:**
- `cargo test -p cyril-ui usage_panel -- --nocapture` → all eight pages, absent values, long/Unicode labels, page wrap, and scroll bounds match literal/cell oracles; full-frame mutation turns the floor fence red and restoration returns green.
- `cargo test -p cyril-ui floor_tests::usage_modal_never_covers_input -- --exact` → every cell at/under the protected input top remains unchanged.
- `cargo test -p cyril-ui` → existing UI state/render behavior and the new modal fences pass.
- `cargo clippy -p cyril-ui -- -D warnings` → no warnings, no lint suppression.

## Slice 3: Wire `/usage`, prompt lifecycle capture, persistence, and live acceptance

**Claim IDs:** C10

**Expected behavior:** `/usage` is always registered and opens from persisted history without an active ACP session; App starts/aborts observer turns at every real `SendPrompt` path, applies routed notifications before their normal consumers, persists completed records, surfaces store errors, and gives the usage modal input priority.

**Oracle:** Fake bridge send counts and a literal key/state transition table for local behavior; the committed capture-derived expected totals and omp's own `stats --json`/overview for live comparison.

**Stress fixture:** No active session; store-open/query/append failure; ordinary typed prompt; one-shot startup prompt; deferred prompt; send failure; usage modal active while typing characters and using every navigation key; two live omp turns matching the committed prompts. Expected: zero wire sends for `/usage`, no leaked character, pending observer state aborts on send failure, every real prompt begins once, and live panel totals/costs match omp within display rounding.

**Regression fence:** `crates/cyril/src/app.rs::tests::usage_modal_command_and_key_priority`; `crates/cyril/src/app.rs::tests::all_prompt_paths_start_and_failed_send_aborts_usage`; `crates/cyril/tests/event_routing.rs::usage_notifications_reach_observer_and_store`.

**Named mutation:** C10 move usage-modal dispatch below normal input handling; the key-priority fence observes leaked text and turns red, then passes after restoration.

**Complexity/production scale:** App adds O(1) work per prompt and notification outside the core costs budgeted in Slice 1. Modal key dispatch is O(1). No new unbounded loop.

**Wall budget/phase:** Prompt/notification coordination is always-on and must add ≤1 ms per event excluding the separately budgeted SQLite append. `/usage` snapshot is a one-off command with Slice 1's ≤2 s 100k-record maximum.

**Files:** `crates/cyril-core/src/commands/builtin.rs`; `crates/cyril-core/src/commands/mod.rs`; `crates/cyril/src/app.rs`; `crates/cyril/src/main.rs`; `crates/cyril/tests/event_routing.rs`; any existing App constructor tests in `crates/cyril/src/app.rs` migrated in the same slice.

**Estimate:** 4–6 implementation hours plus live smoke.

**Diff estimate:** 500 changed lines including wiring tests.

**PR increment:** Live usage observer.

**Commands and expected results:**
- `cargo test -p cyril usage -- --nocapture` → `/usage` is local, every prompt path starts exactly once, send failure aborts, routed records persist, and modal keys match the literal transition oracle; input-priority mutation turns the named fence red and restoration returns green.
- `cargo test` → the complete workspace passes after the clean notification/constructor cutover.
- `cargo clippy -- -D warnings` → the complete workspace has no warnings or suppressions.
- `cargo fmt --check` → all Rust files are formatted.
- Interactive `cargo run -p cyril -- --agent-command omp acp`: send the two committed probe prompts, open `/usage`, inspect all eight pages, then compare tokens/cost/cache-rate to `omp stats --json` for those turns → per-turn and aggregate values agree within rendered rounding; TTFT/duration/tool/error fields are present when the corresponding live events occur.

## Tracker taxonomy

No intended future work is introduced. The design's four permanent non-goals remain unchanged and are not plan slices.

## Self-review

- [x] Every design claim is assigned exactly once: C1–C8/C11–C15 to Slice 1, C9 to Slice 2, C10 to Slice 3.
- [x] Every slice records all thirteen mandatory fields.
- [x] Every claim's fence and exact named mutation land in its owning slice.
- [x] Every new loop records complexity, production input, bound, and accepted maximum; every always-on phase has a wall budget.
- [x] Projection arithmetic plus 25% churn margin remains below the 4,000-line review-size gate; all slices belong to one independently mergeable increment.
- [x] Tracker taxonomy is applied; no accepted behavior is postponed.
- [x] No slice is declared complete; checkpointed-build owns completion.
