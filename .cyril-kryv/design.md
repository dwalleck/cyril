# Falsifiable design: cyril-kryv

## Route and inputs

Route: **Structural**. Source: `.cyril-kryv/route.md`.

Behavior source: approved `.cyril-kryv/spec.md` (requester approval: “Approve specification”, 2026-08-21). The complete observable set is:

1. Live Kiro v2/KAS turns produce one durable engine-neutral record with typed credits, completed-turn/outcome counts, client duration/TTFT, optional provider requests/retries, portable tools, identity, and any standard token/money values actually supplied.
2. Missing Kiro token/money/cache/speed values render `n/a (backend-gated)`, never zero.
3. Only the current session's sidecars may enrich the already-persisted turn: at most 3 attempts/1 second and 64 MiB per file, atomic/idempotent, with portable live data retained on every failure.
4. Tool metrics group by exact name when enriched and portable kind otherwise; credits split equally per distinct call ID; arguments/results count Unicode scalar characters; billed model wins over requested model when available.
5. `/usage` opens immediately and queries KAS account usage once per open; the Costs page separates credits and money and retains only last-known in-process account state with explicit freshness/error status.
6. A combined Context page shows latest scalar/breakdown and sample-backed compaction reduction.
7. Existing standard-ACP/omp fidelity and the bounded 100,000-record snapshot contract remain unchanged; aggregation and rendering have no engine branch.

Empirical inputs: `N/A — Structural route; current repository captures and approved prior decisions cover the premises.` `spec.md` adopts `experiments/conductor-spike/kas-turn-2.16.0.jsonl`, the KAS context fixture, v2 metadata tests, the `_kiro/account/getUsage` workflow fixture, and the Phase 1 captured standard-ACP fixture as current evidence.

## Input shapes

| Input | Production-reachable shapes | Status |
|---|---|---|
| KAS `turn_completion` | `promptTurnSummaries` absent/empty/single/multi; unit singular/plural absent/empty/valid; usage zero/positive/negative/non-finite/wrong type; `usedTools` absent/empty/multi/duplicate/Unicode; elapsed zero/positive/max/wrong type; status missing/`success`/`aborted`/unknown; request IDs absent/empty/one/many/duplicate | Covered by C1, C2, C3 |
| v2 metadata metering | credits absent/explicit zero/positive/invalid; one/many metering entries with same/different units; duration/context absent/present across interleaved retain-last frames; standard token fields absent/present | Covered by C2 and C3 |
| Standard ACP usage | absent; present with optional cache/thought fields; zero counts; maximum `u64`; cumulative money absent/same currency/reset/currency change | Covered by C2 and C11 |
| Turn lifecycle | fresh/loaded; first text absent/present after thought/tool traffic; success/cancel/refusal/max-turn; one/many bridge errors; KAS metering before `turn_end`; trailing context after completion | Covered by C3 and C8 |
| Tool lifecycle | no calls; one/many; duplicate start/update IDs; repeated exact name; every portable kind; failed/success; raw input/output absent/null/scalar/object/array/Unicode; large values | Covered by C4 and C5 |
| Sidecar selection | v2 deterministic UUID JSON/JSONL; KAS zero/one/many workspace-hash candidates; new/loaded session; file absent/late/permission denied; relative/path-separator session ID; non-Unicode path; current/prior session files | Covered by C5; non-Unicode display is `N/A — same lossy path-display contract approved by cyril-gfkm` |
| Sidecar content | empty/partial/malformed; ≤64 MiB/exactly 64 MiB/>64 MiB; KAS streamed JSONL; v2 monolithic JSON; zero/one/many turns; current turn exact tools/billed model absent/present; concurrent append/replace | Covered by C5 and C6 |
| Model identity | requested absent/bare/provider-model/`auto`; billed absent/bare/provider-model; Unicode; both present and different | Covered by C6 |
| Metric availability | observed; unreported; backend-gated; mixed histories with any combination | Covered by C2, C7, C10, C11 |
| Context | scalar 0/inside/100/out-of-range/non-finite; breakdown absent/full/malformed with each bucket zero/nonzero; v2 scalar-only; KAS scalar then full trailing frame | Covered by C8 and C10 |
| Compaction | no baseline; start with/without baseline; completed/failed; after-sample missing/equal/lower/higher; repeated sequential compactions; unrelated unknown status | Covered by C8; unknown/new status parsing is `N/A — cyril-0f4e owns operator-visible status expansion` |
| Account query | no active session; active v2/KAS; success false/true; data absent; empty/multi breakdowns; bonus credits empty/multi; overages on/off; invalid numeric/date/unit fields; response after modal close/reopen; repeated opens | Covered by C9 and C10 |
| Persistence/migration | fresh DB; Phase 1 DB with zero/many rows; migration interrupted/reopened; concurrent readers/writer; enrichment before/after snapshot; forced tool replacement failure | Covered by C5, C7, C12 |
| Aggregation | Kiro-only/full-fidelity-only/mixed; no/single/multi unit and currency; no/single/many groups; exact-name and kind fallback tools; missing provider request counts | Covered by C4, C7, C10, C11 |
| Modal | every existing page plus Context; empty/full/mixed snapshot; loading/success/stale/error account state; long/Unicode labels; 60×16 floor; scroll/page wrap; refresh while open | Covered by C9 and C10 |
| Scale | 100,000 turns; high tool cardinality; 20+ recent/errors; current sidecar at/over cap | Covered by C5 and C12 |

## Removed-invariant sweep

The change is additive except for one schema interpretation cutover: Phase 1's implicit `Option == generic absence` becomes explicit observed/unreported/backend-gated metric state, and tool grouping broadens from closed `ToolKind` to exact-name-or-kind identity. Existing values remain losslessly representable. C2/C7/C11 fence the old token/money semantics; C4 fences fallback grouping so an unenriched record cannot disappear or double-count.

## Placement

### Kiro wire adapters

- **Owner:** `cyril-core::protocol::convert::kiro` maps v2 metadata; `cyril-core::protocol::convert::kas` maps KAS `turn_completion` and account responses. These are the existing modules allowed to know dialect field names and `acp::` types.
- **New seam:** none. Both emit typed domain notifications through the existing conversion seam. KAS adds a response-carrying `BridgeCommand::QueryUsageAccount`, following `ListKasHooks`/`Workflow`; v2 metadata continues through its existing notification.
- **Forbidden:** no raw JSON parsing in App/UI/usage aggregation; no `acp::` outside protocol; no silent default for malformed numeric/unit/status fields; no conflation of `turn_completion` with lifecycle `turn_end`.

### Metric and record domain

- **Owner:** `cyril-core::types::usage` owns `ObservedMetric<T>`, `UnavailableReason`, validated `MeteredAmount`/unit, outcome/provider-request fields, exact-or-portable tool identity, requested/billed model identity, context/account snapshot types, and `UsageRecordId`.
- **New seam:** the existing `UsageRecord` interface is deepened rather than wrapped. `Money` remains money; non-money metering is a separate typed collection. Optional fields whose absence reason matters become `ObservedMetric`, while fields with ordinary optionality remain `Option`.
- **Forbidden:** no synthetic three-letter currency for credits; no engine enum/name in a persisted record or snapshot; no invalid state containing both a metric value and an unavailable reason; no zero sentinel.

### Turn/context/compaction observer

- **Owner:** `cyril-core::usage::UsageObserver` remains the pure correlation module. It consumes typed generic notifications, retains per-session latest context and compaction state, and emits one `UsageWrite` action at a time (`Turn`, `ContextLatest`, or `Compaction`).
- **New seam:** `UsageObserver::apply -> Option<UsageWrite>` replaces `Option<UsageRecord>` so non-turn usage facts do not masquerade as turns. The App still supplies clock/context and performs persistence; the observer never receives a store.
- **Forbidden:** App cannot compute charges, outcome categories, provider retries, availability, or compaction deltas; observer cannot inspect provider/model/engine strings; trailing KAS context cannot create a second turn.

### Current-session sidecar enrichment

- **Owner:** new private module `cyril-core::usage::kiro_sidecar` owns validated path location, v2 JSON/JSONL and KAS JSONL adapters, session-start cursors, bounded retry/read policy, and `UsageEnrichment` output. It never queries SQLite.
- **New seam alternatives:** (A) App synchronously reads sidecars after `TurnCompleted`: minimal interface but blocks the event loop and spreads retry/path/format knowledge into orchestration. (B, chosen) a sequential enrichment worker exposes `session_started` and `enrich(record_id, hint)` commands plus a result receiver; two real private adapters implement v2 and KAS shapes behind it. The worker owns cursors and retries, so callers learn neither paths nor formats.
- **Forbidden:** no archive directory scan beyond locating the named active KAS session; no prior-turn import; no file over 64 MiB; no unvalidated session ID in path construction; no unbounded watcher/task; no SQLite connection or UI state in the worker.

### Durable log and aggregation

- **Owner:** `cyril-core::usage::UsageLog` owns schema migration, append/upsert/enrich transactions, and all SQL aggregation. It returns `UsageRecordId` from turn append; exact enrichment atomically updates billed identity and replaces that turn's tool rows.
- **New seam:** none. `UsageLog` remains the concrete SQLite deep module. Schema versioning uses `PRAGMA user_version`; new metered charges use a child table rather than overloading `cost_*`; context-latest and compactions use separate tables; legacy token/money columns remain authoritative.
- **Forbidden:** no vendor column or SQL branch; no persisted account-plan response; no loading full history in memory; no partial enrichment commit; no sum across different unit/currency; no destructive migration of Phase 1 rows.

### Account query orchestration

- **Owner:** the builtin usage command dispatches the response-carrying query only when its resolved `UsageAccountCommandSource` is KAS, then returns `ShowUsage { account_query_started }`. The bridge awaits the short RPC and emits exactly one typed success/failure notification. App routes that notification into `UiState` and refreshes the open modal.
- **New seam:** `UsageAccountCommandSource::{Kas,None}` follows existing hooks/workflow source resolution and keeps engine selection out of rendering. A generic raw `ExtMethod` is rejected because it discards responses and would force JSON parsing into App.
- **Forbidden:** no query for v2/standard agents; no blocking modal open; no account response in SQLite; no failure routed as a turn `BridgeError` that would corrupt usage error counts.

### Modal state and rendering

- **Owner:** `cyril-ui::state` owns Context page navigation and ephemeral account loading/fresh/stale/error state; `widgets::usage_panel` renders immutable snapshot/account values. Core snapshot types carry metric coverage, not display strings.
- **New seam:** the existing `UsagePanelState` is deepened with `account` state and a ninth `Context` page. No second overlay or renderer is introduced.
- **Forbidden:** renderer cannot infer Kiro from labels/model/provider, query the bridge/store, combine charge types, or substitute generic em-dashes for an explicit backend-gated state; modal refresh cannot reset the current page/scroll unless bounds require clamping.

### App orchestration

- **Owner:** App coordinates `UsageWrite` persistence, schedules enrichment using the returned record ID/hint, receives enrichment/account outcomes, refreshes an open snapshot, and otherwise remains calculation-free.
- **New seam:** one enrichment-result receiver joins the existing event loop; slow file work stays in the worker.
- **Forbidden:** no sidecar parsing, SQL, charge arithmetic, or compaction math in App; no awaited file/account work on the terminal event loop.

## Claims

- **C1.** KAS `turn_completion` converts every captured credit, elapsed, status, tool-name, and request-ID entry exactly without becoming `TurnCompleted`.
- **C2.** v2/KAS non-money metering and standard ACP money remain distinct typed values, and Kiro absence carries backend-gated availability without changing observed standard values.
- **C3.** One lifecycle completion produces one turn with correct client timing, success/cancel/error outcome, and optional provider-request/retry totals; interleaved/trailing usage frames never duplicate it.
- **C4.** Distinct call IDs remain additive under exact-name-or-kind grouping: failures and character counts sum per call and every token/money/charge share sums back to its turn total.
- **C5.** Current-session enrichment is path-safe, current-turn-only, at most 64 MiB and 3 attempts/1 second, and atomically/idempotently updates an already-persisted record while every failure preserves live data.
- **C6.** Model grouping uses validated billed provider/model when present and otherwise the unchanged dispatch-time requested identity.
- **C7.** Schema migration preserves every Phase 1 row and metric, and engine-neutral SQL aggregates observed values, coverage, outcomes, charges, and tools without combining unlike units.
- **C8.** Context retains the latest valid scalar/full breakdown, and compaction gain records only a completed same-session before/after reduction; missing/failed/increased samples never fabricate savings.
- **C9.** Each `/usage` open renders local data before at most one KAS account request, then applies one typed success/failure while preserving last-known in-process freshness state.
- **C10.** The modal renders separate Credits/Monetary sections, explicit backend-gated metrics, detailed Tools, and one Context page correctly for empty, Kiro-only, full-fidelity-only, mixed, narrow, and refreshed states.
- **C11.** Existing captured standard-ACP/omp records produce byte-equivalent Phase 1 token/money/cache/timing/group totals after the migration and light up the same fields for future Kiro standard usage.
- **C12.** A 100,000-turn snapshot retains at most 20 recent and 20 error details and no collection proportional to turns beyond genuine aggregate group cardinality.
- **C13.** Kiro field/path knowledge exists only in protocol converters and the private sidecar module; aggregation/rendering contain no engine/provider-name branch.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Exact KAS metering conversion | Four live 2.16.0 frames; missing/invalid synthetic matrices | Parse raw capture and compare literal tuples; any credit/elapsed/status/tool/request mismatch or emitted lifecycle completion falsifies C1 | Python reads raw JSONL directly and computes tuples/list lengths; production uses Rust typed conversion | `convert/kas.rs`: treat `turn_completion` as `turn_end` or omit the third request ID in frame four; fixture test reports C1 tuple/lifecycle mismatch | `convert::kas::tests::captured_turn_completion_maps_exactly_and_is_not_terminal` | <1s | PASS |
| C2 | Typed charge/availability separation | v2/KAS credit, standard money, absent/future standard token, unlike units | Feed adapter matrix; synthetic currency, merged units, zero-for-absence, or changed standard values falsifies C2 | Literal domain-value table independent of converters/SQL | `types/usage.rs`: encode credits as `Money("USD")` or default absent tokens to zero; adapter matrix reports C2 variant mismatch | `usage::tests::metric_source_matrix_preserves_typed_values_and_absence` | <1s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C3 | Exactly one correctly classified/timed turn | v2/KAS ordering, thought/tool before text, cancelled/aborted/error, request IDs absent/many | Replay hand timelines; duplicate/missing record, wrong TTFT, wrong outcome, or fallback provider request falsifies C3 | Test-local event timeline and literal outcome/request table | `usage.rs`: finish on KAS metering as well as turn end or count absent IDs as one; timeline reports two turns/wrong request total | `usage::tests::kiro_turn_timeline_matrix_records_once` | <1s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C4 | Additive per-call tool attribution | duplicate updates, repeated names, exact/fallback identity, Unicode JSON, failed call, 1/2/3 calls | Aggregate fixed corpus; any call/error/char mismatch or shares not summing to turn totals falsifies C4 | Test-local map keyed by call ID plus direct rational division; production uses observer+SQL | `usage.rs`: divide by unique names instead of call IDs or count UTF-8 bytes; oracle reports C4 credit/character mismatch | `usage::tests::tool_call_instance_attribution_matches_oracle` | <2s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C5 | Bounded safe idempotent current-session enrichment | new/loaded v2/KAS; late/partial/malformed/denied/oversized; traversal ID; repeated attempt | Temp fixtures and controlled clock; prior-turn import, >cap read, >3 attempts/>1s, escaped path, duplicate row, partial replacement, or lost live row falsifies C5 | Raw file sizes/line offsets and direct SQLite counts from a second connection; production worker/parser has different mechanism | `usage/kiro_sidecar.rs`: initialize loaded cursor at zero or advance cursor before parse succeeds; fixture imports history or cannot retry and C5 fails | `usage::kiro_sidecar::tests::bounded_current_turn_enrichment_matrix` plus `usage::tests::enrichment_replace_is_atomic_and_idempotent` | <5s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C6 | Billed-model precedence | absent/bare/provider requested and billed identities, conflicting `auto` | Enrich identity table; grouping key differing from billed-else-requested literal falsifies C6 | Literal table with no production split helper | `usage.rs`: SQL groups only existing requested model columns; expected billed group is absent | `usage::tests::billed_model_wins_grouping_matrix` | <2s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C7 | Lossless migration and neutral aggregation | fresh/legacy populated DB; mixed units/currencies/coverage/outcomes; forced enrichment error | Copy legacy fixture, migrate twice, compare old columns/totals and new raw SQL; data loss, non-idempotency, unit mixing, or partial tools falsifies C7 | Raw rusqlite PRAGMA/SELECT checks and independent test-local reductions; production migration/queries are separate | `usage.rs`: set new availability columns uniformly to backend-gated or commit tool delete before replacement inserts; legacy/atomic checks fail | `usage::tests::v1_migration_is_lossless_idempotent_and_enrichment_atomic` | <5s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C8 | Honest context and compaction gain | scalar/full/malformed; start/complete/fail; missing/equal/lower/higher; trailing KAS frame | Replay state table; stale bucket clearing, cross-session pairing, negative gain, or gain from missing sample falsifies C8 | Hand-authored before/after table and arithmetic | `usage.rs`: use absolute difference or clear breakdown on scalar-only update; table reports fabricated gain/lost buckets | `usage::tests::context_and_compaction_state_matrix` | <1s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C9 | Nonblocking one-query account lifecycle | no session/v2/KAS; success/failure/late/reopen; modal closes mid-response | Drive fake bridge/UI timeline; modal delayed, wrong engine queried, >1 query/open, last-known cleared, or late response reopening modal falsifies C9 | Channel send/order counter plus literal state table; production bridge uses ext RPC | `commands/builtin.rs`: await account response before returning ShowUsage or dispatch for V2; ordering/count assertion fails | `app::tests::usage_account_query_order_and_state_matrix` | <2s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C10 | Correct complete modal presentation | empty/Kiro/full/mixed, Context/account states, long labels, 60×16, refresh | Render every page/state; missing explicit labels/data, combined charge types, overwritten input, or page reset on refresh falsifies C10 | Ratatui `TestBackend` cell coordinates and literal expected labels | `widgets/usage_panel.rs`: reuse em dash for backend gating or concatenate credits into money; buffer assertion misses required labels | `usage_panel::tests::kiro_full_mixed_pages_render_at_floor` | <3s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C11 | Phase 1 fidelity and forward activation | captured omp two turns before/after migration; synthetic Kiro standard usage | Compare snapshots field-for-field; changed Phase 1 value or Kiro standard values still gated falsifies C11 | Existing Python capture oracle from `.cyril-gfkm` plus serialized snapshot tuple | `usage.rs`: mark every credit-bearing record token-gated even when tokens are observed; synthetic Kiro row fails activation | `usage::tests::phase1_snapshot_is_unchanged_and_observed_wins_coverage` | <3s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C12 | Bounded 100k snapshot | 100,000 mixed records, 30 errors, high tool/group cardinality | Seed and snapshot; details >20, retained turn vector, or aggregate mismatch falsifies C12 | Direct SQL COUNT/SUM and collection-length checks | `usage.rs`: load per-turn context/tool details into `UsageSnapshot`; bounded type/length check fails | `usage::tests::kiro_snapshot_remains_bounded_at_100k` | ~1 min | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |
| C13 | Structural locality | all new symbols and decision sites | Compile/privacy/source scan; Kiro literal/AgentEngine/provider-name branch in aggregation/rendering or raw JSON outside adapters falsifies C13 | File ownership allowlist and dependency direction, independent of runtime implementation | `widgets/usage_panel.rs`: branch on provider == "kiro" for placeholders; source scan reports forbidden literal | `usage::tests::usage_layers_are_engine_neutral` plus crate visibility | <1s | PENDING — checkpointed-build, per-slice gate assigned in `plan.md` |

## Non-goals and future work

- **Permanent non-goal:** synthesize credits as currency or estimate token/money/cache metrics. Those values would look authoritative while having no source.
- **Permanent non-goal:** scan or backfill prior Kiro sessions. The requester selected current-session enrichment; the live observer remains the durable source.
- **Permanent non-goal:** use OTLP for this dashboard. Client timestamps are the chosen operator-facing latency measure.
- **Permanent non-goal:** infer skill use from prompts, steering documents, or tool prose; no invocation event exists.
- **Intended future work:** engine-neutral Behavior sentiment is cyril-tq2g.
- **Intended future work:** focus/governance behaviors remain in verified cyril-0o7e.
- **Intended future work:** backend subscription vocabulary remains watched by verified cyril-guml.
- **Intended future work:** `unsummarized_dropped` parsing/rendering is verified cyril-0f4e.

## Falsifier run log

- 2026-08-21 — Python independently parsed all four `turn_completion` frames in `experiments/conductor-spike/kas-turn-2.16.0.jsonl`. The first assertion intentionally assumed two request IDs for every frame and failed because frame four carries three; the claim/oracle expectation was corrected to the literal captured vector `[2,2,2,3]` with retries `[1,1,1,2]`. Re-run result: `PASS C1`; credits, elapsed, status, used-tools lists, request counts, and retries matched all four raw frames.

## Approval

Requester approval: "Approve design"
Date: 2026-08-21
Approved risk acceptances: None
