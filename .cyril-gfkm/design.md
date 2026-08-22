# Falsifiable design: cyril-gfkm

## Route and inputs

Route: **Structural**. Source: `.cyril-gfkm/route.md`.

Behavior source: `route.md` T4 (`spec.md: N/A — behavior fully explicit`). The complete behavior set is:

1. Given an ACP prompt response with standard `Usage` and cumulative `usage_update` context/cost, completing the turn records one typed turn with optional token/cache counts, per-turn cost delta, session/folder/model/provider/agent type, duration, TTFT, tool calls, stop/error outcome, and timestamp.
2. Given persisted records, opening `/usage` shows overview, costs, providers, models, tools, recent, errors, and folders; overview formulas match omp stats within rounding for the captured fixture.
3. Given records from any engine, aggregation has no engine-specific branch.
4. Given absent or malformed optional wire fields, absence remains explicit and invalid values never become plausible zeroes.
5. Given a large history, opening and paging the view retains only aggregate groups plus 20 recent and 20 error details; the persistent log, not the UI, owns the full history.

Empirical inputs: `evidence.md` and a Gilfoyle probe are N/A — Structural route. Current repository evidence still grounds the wire adapter: `experiments/conductor-spike/omp-usage-update-2turn.jsonl`, `experiments/conductor-spike/probe-omp-usage-update.py`, `docs/usage-observer-design.md`, and the pinned ACP schema's `PromptResponse.usage` / `UsageUpdate.cost` types.

## Input shapes

| Input | Production-reachable shapes | Status |
|---|---|---|
| Prompt response usage | absent; present with required totals; each thought/cache-read/cache-write field absent or present; zero counts; counts through `u64::MAX`; totals not arithmetically self-consistent | Covered by C1 and C11 |
| Session gauge | `size` zero/nonzero; `used` below/equal/above size; cost absent/present; amount zero/positive/negative/non-finite; empty/invalid/valid currency | Covered by C2 and C11 |
| Cumulative cost sequence | fresh session; loaded session with a pre-turn baseline; loaded session whose first gauge arrives during the first resumed turn; equal/increasing/decreasing amount; currency unchanged/changed; multiple gauges per turn | Covered by C2, C13, and C14 |
| Turn lifecycle | no first text; one/many thought frames before text; one/many text chunks; every `StopReason`; bridge error absent/present/multiple; completion with and without usage; completion with no observer-owned pending turn | Covered by C3 and C15 |
| Tool lifecycle | empty; one; many distinct; duplicate start/update IDs; every `ToolKind`; pending/completed/failed status; multiple calls of one kind | Covered by C4 |
| Identity | model absent; `provider/model`; bare model; empty segment; multiple `/`; ASCII/Unicode; folder absolute with spaces/Unicode; main/subagent/advisor agent type | Covered by C5 |
| Folder encoding | non-Unicode host path | N/A — permanent non-goal: the terminal and SQLite text view cannot render a lossless cross-platform non-Unicode key; the record uses the same lossy display contract as the TUI rather than inventing a non-portable byte encoding |
| Persistence | new/empty DB; existing single/many records; simultaneous Cyril processes; interrupted write; busy/corrupt/unwritable DB; token value above SQLite signed-integer range | Covered by C6 and C11 |
| Aggregation | empty; one; many; repeated equal values; missing tokens/cost/timing; zero duration; multiple currencies; errors; no tools; many tools; each grouping key absent/present | Covered by C7 and C8 |
| Scale | 100,000 records, many sessions, bounded and unbounded group cardinality | Covered by C12 |
| Usage modal | empty snapshot; all eight pages; no/single/many rows; long/Unicode labels; narrow/short terminal; scroll bounds; page wrap; input area present | Covered by C9 and C10 |

## Removed-invariant sweep

Purely additive. The change consumes data Cyril already receives and adds a local observer/store/modal; it removes no serialization point, validation, ownership stamp, routing rule, or at-most-one-turn guard.

## Placement

### ACP usage adapter

- **Owner:** `cyril-core::protocol::convert`; it is the existing module that may see both `acp::` and Cyril domain types. `bridge.rs` calls the prompt-usage adapter because the prompt response terminates there; `convert_session_update` maps gauge cost.
- **New seam:** none. It slots behind the existing conversion seam.
- **Forbidden:** no `acp::` type outside `cyril-core::protocol`; no `_kiro`, omp, provider, or model-price branch in conversion.

### Usage domain and turn observer

- **Owner:** new `cyril-core::types::usage` owns `TokenUsage`, validated `Money`, `SessionOrigin`, `AgentType`, `UsageRecord`, and snapshot types. New `cyril-core::usage::UsageObserver` owns pending-turn correlation, clocks, gauge baselines, tool deduplication, and record completion.
- **Competing seam A:** App reads `SessionController` and private `UiState` fields at turn end and assembles a record. Small initial diff, but duplicates state-machine logic in the orchestrator and makes UI internals part of the observer interface.
- **Competing seam B — chosen:** App supplies a small `TurnContext`, monotonic timestamps, and routed notifications to `UsageObserver`; the observer returns zero or one completed `UsageRecord`. More core implementation, but a smaller interface, engine neutrality, deterministic state tests, and no UI dependency.
- **Forbidden:** App may coordinate but may not compute TTFT, deltas, tool shares, or aggregation; UI may not mutate records; the observer may not inspect engine/provider names to select behavior.

### Persistent log and aggregation

- **Owner:** `cyril-core::usage::UsageLog`, a concrete SQLite-backed deep module with `open`, `append`, and `snapshot`. Writes are one transaction; reads aggregate in SQL and retain only group rows plus bounded details. The database lives at the existing Cyril config root as `usage.sqlite3`.
- **Competing seam A:** append JSONL and rescan every record for every `/usage`. Simple persistence, but unbounded UI memory/latency and no transactional multi-row tool write.
- **Competing seam B — chosen:** SQLite WAL with indexed records/tools and direct aggregate queries. One adapter exists, so no repository trait is introduced. This matches the scale of the problem without exposing SQL to callers.
- **Forbidden:** no file scan/backfill, no SQL in App/UI, no in-memory copy of the complete history, no sum across currencies, and no zero substituted for absent tokens/cost/timing.

### Usage command and modal

- **Owner:** builtin `/usage` in `cyril-core::commands` returns `CommandResultKind::ShowUsage`; App asks `UsageLog` for a snapshot; `cyril-ui::state` owns page/scroll state; `widgets::usage_panel` renders the immutable snapshot through `TuiState`.
- **New seam:** `ShowUsage` follows the existing `ShowPicker`/`ToggleVoice` command-result seam; `TuiState::usage_panel` follows existing modal read interfaces. No second control interface.
- **Forbidden:** the command layer cannot open UI or read SQLite; renderer cannot query/mutate; the modal must join both key-priority and mouse-scroll guards and must never cover the input rows.

## Claims

- **C1.** Prompt-response usage converts every standard token field exactly, and an absent usage remains `None`.
- **C2.** A valid same-currency cumulative gauge yields the non-negative per-turn delta from the turn-start baseline; absent, invalid, reset, or currency-changing cost yields `None` while context counts still flow.
- **C3.** An observed turn records duration from dispatch to completion and TTFT from dispatch to the first agent-message chunk, without treating thought/tool traffic as first text.
- **C4.** Tool calls are deduplicated by `ToolCallId`, final failure state is retained, and each turn's token/cost share is divided equally across its distinct calls so shares remain additive.
- **C5.** Record identity is snapshotted at dispatch: session, display folder, agent type, and model; only a non-empty `provider/model` splits into provider plus model.
- **C6.** One completed record and its tool rows commit atomically to SQLite under WAL/busy-timeout, and concurrent readers never observe a partial turn.
- **C7.** Overview matches omp formulas: cache rate is cache-read divided by uncached-input plus cache-read; token rate is the mean of per-turn output/duration; duration and TTFT are means of present values; errors are records with an error; costs sum only within currency.
- **C8.** Provider, model, folder, agent-type, and tool rollups use only `UsageRecord` fields and produce the same arithmetic for arbitrary identities, with no engine-specific branch.
- **C9.** The modal renders Overview, Costs, Providers, Models, Tools, Recent, Errors, and Folders pages, handles empty/long/narrow/many-row inputs, and never overwrites input rows.
- **C10.** `/usage` opens locally without an active session; Esc closes; Tab/BackTab/Left/Right cycle pages; arrows/page keys scroll; all other keys are consumed before normal input.
- **C11.** Invalid wire money and SQLite-out-of-range integers fail visibly, while optional missing values remain absent instead of becoming zero-valued successes.
- **C12.** A 100,000-record history produces aggregate groups plus at most 20 recent and 20 error details; no snapshot collection grows with total record count except genuine distinct group keys.
- **C13.** A loaded session never attributes its historical cumulative cost to its first resumed turn: a missing pre-turn baseline makes that turn's cost unknown and establishes the next baseline.
- **C14.** A fresh session starts from a zero cumulative baseline, so its first valid gauge is attributable to its first turn.
- **C15.** A bridge-error turn is persisted and appears in Recent and Errors even when the prompt response carries no token usage.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Exact optional token conversion | Captured T1 no-cache and T2 cache-read; usage absent | Replay both captured prompt results; any converted field differs or absence becomes a record falsifies C1 | Python independently reads raw JSON and literal expected tuples | `protocol/convert/mod.rs`: swap `input_tokens` and `output_tokens`; captured-fixture test reports C1 mismatch | `convert::tests::captured_omp_prompt_usage_maps_exactly` plus absent case | <1s | PASS |
| C2 | Safe cumulative cost delta | Two captured costs; absent/reset/currency change | Compute capture deltas and synthetic invalid sequences; a negative/cross-currency/fabricated delta falsifies C2 | Python subtraction over raw capture for valid sequence; literal tables for invalid sequences | `usage.rs`: use current cumulative amount directly after the first turn; two-turn test expects `0.0004418` and turns red | `usage::tests::cumulative_cost_delta_matrix` | <1s | PASS |
| C3 | Correct duration and TTFT boundaries | Thought/tool before first text; no text; multiple text chunks | Feed synthetic instants; TTFT before first agent text, changing on later chunks, or present with no text falsifies C3 | Hand-authored event timeline and duration arithmetic independent of observer state | `usage.rs`: set TTFT on `AgentThought`; timeline test reports early TTFT | `usage::tests::timing_uses_first_agent_text_only` | <1s | PENDING — checkpointed-build, observer slice |
| C4 | Deduplicated additive tool allocation | Duplicate IDs, two kinds, failed update | Feed duplicate starts/updates; count other than distinct IDs or token/cost shares not summing to turn totals falsifies C4 | Independent map/set and rational-share arithmetic in test | `usage.rs`: push every update as a call; allocation test reports three calls instead of two | `usage::tests::tool_calls_dedupe_and_shares_add_up` | <1s | PENDING — checkpointed-build, observer slice |
| C5 | Dispatch-time identity | absent/bare/provider model; Unicode folder; mid-turn model change | Start turns across identity table; any wrong split or later model mutation changing record falsifies C5 | Literal identity table | `usage.rs`: split bare model into provider with empty model; identity table turns red | `usage::tests::identity_snapshot_matrix` | <1s | PENDING — checkpointed-build, observer slice |
| C6 | Atomic durable append | record plus two tools; second connection; forced constraint failure | Append, inspect through an independent SQLite connection, then force a tool-row failure; partial parent/tools or committed failed transaction falsifies C6 | Raw SQL count/foreign-key queries from a second connection | `usage.rs`: commit record before tool insert; forced failure leaves parent count 1 | `usage::tests::append_is_atomic_across_record_and_tools` | <2s | PENDING — checkpointed-build, store slice |
| C7 | Omp-compatible overview arithmetic | mixed cache, errors, missing timing, two currencies | Aggregate a fixed corpus; any overview field differs from hand arithmetic or currencies combine falsifies C7 | Pure test oracle computes formulas directly from fixture records; production uses SQL | `usage.rs`: divide cache-read by total tokens; oracle reports cache-rate mismatch | `usage::tests::overview_matches_independent_omp_formula_oracle` | <2s | PENDING — checkpointed-build, aggregation slice |
| C8 | Engine-neutral breakdowns | arbitrary provider/model/folder/agent/tool identities | Store isomorphic records under unrelated identities; any arithmetic changes beyond grouping labels falsifies C8 | Same unlabeled numeric corpus grouped by a test-local map | `usage.rs`: special-case provider `openai-codex`; arbitrary-provider equivalence turns red | `usage::tests::breakdowns_are_identity_agnostic` | <2s | PENDING — checkpointed-build, aggregation slice |
| C9 | Complete input-safe rendering | eight pages; empty/many/long/Unicode; 30x10 and 80x24 | Render each page to `TestBackend`; missing heading/data or cells written at/under input top falsifies C9 | TestBackend cell coordinates and literal labels | `widgets/usage_panel.rs`: use full frame height instead of `input_top`; floor test detects overwritten input row | `usage_panel::tests::all_pages_render_and_clamp_above_input` | <2s | PENDING — checkpointed-build, UI slice |
| C10 | Local modal interaction | no session; every modal key; normal input behind modal | Execute `/usage`, dispatch key table; wire send, page/scroll mismatch, or leaked character falsifies C10 | Fake bridge send count plus literal state-transition table | `app.rs`: place usage modal after normal-input dispatch; character-leak assertion turns red | `app::tests::usage_modal_command_and_key_priority` | <2s | PENDING — checkpointed-build, wiring slice |
| C11 | No fabricated defaults | invalid money matrix; absent fields; `u64::MAX` persistence | Convert/append invalid values; accepted invalid amount, zero replacing absence, or silent overflow falsifies C11 | Literal validity matrix and SQLite signed-range boundary | `types/usage.rs`: `unwrap_or(0.0)` invalid cost; matrix sees `Some(0)` instead of error/absence | `usage::tests::invalid_values_fail_without_defaulting` | <1s | PENDING — checkpointed-build, domain/store slices |
| C12 | Bounded snapshot at 100k | 100,000 records, 30 errors, bounded/distinct groups | Bulk seed then snapshot; recent/errors above 20, retained per-turn vector, wrong totals, or query over 2s on reference workstation falsifies C12 | Direct SQL `COUNT/SUM` and collection-length checks; wall clock is a one-shot measurement | `usage.rs`: load all records into `UsageSnapshot`; snapshot type/length fence and stress run expose 100k details | `usage::tests::snapshot_is_bounded_for_large_history`; one-shot release stress run retained in checkpoint | ~1 min | PENDING — checkpointed-build, aggregation slice |
| C13 | Loaded-session baseline safety | no pre-turn gauge vs pre-turn gauge | Mark session loaded and complete first turn; any historical cumulative amount becomes first-turn cost falsifies C13 | Explicit baseline state table | `usage.rs`: initialize every session to zero; loaded-session test sees historical amount | `usage::tests::loaded_session_requires_cost_baseline` | <1s | PENDING — checkpointed-build, observer slice |
| C14 | Fresh-session first cost | fresh first turn and valid first gauge | Mark session fresh; missing or non-equal first-turn delta falsifies C14 | Zero-baseline arithmetic | `usage.rs`: treat fresh as unknown; test sees missing first cost | `usage::tests::fresh_session_attributes_first_cost` | <1s | PENDING — checkpointed-build, observer slice |
| C15 | Error turn without tokens | BridgeError then usage-less completion | Complete error turn without usage; missing persisted/recent/error row falsifies C15 | Independent raw SQL and snapshot membership checks | `usage.rs`: early-return when `usage.is_none()`; error fixture produces zero rows | `usage::tests::usage_less_error_turn_is_persisted` | <2s | PENDING — checkpointed-build, observer/store slice |

## Non-goals and future work

- **Permanent non-goal:** scanning omp/Kiro transcript files for historical backfill. This issue explicitly makes Cyril the live client; file scanners duplicate vendor storage semantics and violate the engine-neutral seam.
- **Permanent non-goal:** recomputing cost from a bundled model price table when the standard wire already supplies cumulative cost. The wire remains authoritative; agents that omit cost produce `None`, not an estimated vendor branch.
- **Permanent non-goal:** exact backend tool function names, argument/result character counts, cache savings, and premium-request counts. Standard ACP exposes portable `ToolKind`, lifecycle, and token/cost totals but not those backend-internal fields; the panel labels the breakdown “tool kinds”.
- **Permanent non-goal:** a local HTTP server. A TUI modal satisfies the viewer requirement inside Cyril without adding a listening socket, browser assets, authentication policy, or server lifecycle.
- Intended future work: N/A — the design does not defer accepted behavior.

## Falsifier run log

- 2026-08-21 — Python independently parsed `experiments/conductor-spike/omp-usage-update-2turn.jsonl`, asserted each prompt total equals uncached input + output + optional cache fields, and compared literal token tuples and cumulative-cost deltas. Result: `PASS C1/C2 {'tokens': [(19428, 18, 0, 19446), (259, 5, 19200, 19464)], 'cost_deltas': [0.0039072, 0.0004418]}`.

## Approval

Requester approval: "Approve design"
Date: 2026-08-21
Approved risk acceptances: None.
