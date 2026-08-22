# Spec: Kiro credits-degraded usage observer

## Request (verbatim)
> claim and implement cyril-kryv

## What this is
Cyril will feed Kiro v2 and KAS metering into the existing engine-neutral usage log and `/usage` modal. Kiro records use credits as their available charge unit while token, monetary-cost, cache, and token-rate metrics remain explicitly backend-gated when the wire omits them.

## Roles
- **Cyril operator**: drives a Kiro v2 or KAS session and opens `/usage` to inspect live and aggregated usage, latency, context, tools, identity groupings, and account metering.

## Behavior

### Capture live Kiro turn usage
- **Given**: A Cyril operator has an active Kiro v2 or KAS session and dispatches a prompt.
- **When**: The engine completes the turn and Cyril has received its metering, context, tool, and lifecycle frames.
- **Then**: One durable record contains the completed-turn count, success/cancelled/error outcome, credits when supplied, client-measured duration and TTFT, provider-request/retry counts when supplied, latest turn context sample, portable tool calls, folder/session identity, requested model, and any standard token/cost fields the wire supplied.

### Preserve backend-gated absence
- **Given**: A Kiro turn supplies no standard token, monetary-cost, cache, or token-rate data.
- **When**: The record is aggregated and rendered.
- **Then**: Every dependent metric displays `n/a (backend-gated)` and no missing value is stored or rendered as numeric zero.

### Enrich the current session
- **Given**: Cyril has persisted a live Kiro record and the active session's sidecar may contain exact tool and billing identity.
- **When**: The turn ends or the Cyril operator reopens `/usage`.
- **Then**: Cyril makes at most three asynchronous enrichment attempts within one second, reads no sidecar larger than 64 MiB, and updates that same record with exact tool name, argument/result character counts, last-used time, and billed model when available; failure leaves the live record intact and logs path/error context.

### Attribute tools
- **Given**: One completed turn has one or more distinct tool-call IDs and an available credit charge.
- **When**: The Tools page aggregates the turn.
- **Then**: Calls, failures, argument/result Unicode-character totals, last-used time, and by-model usage aggregate by exact tool name when enriched and otherwise by portable kind; the turn's credits divide equally across call IDs and the shares sum to the turn total within floating-point tolerance.

### Query KAS account usage
- **Given**: `/usage` opens with an active KAS session.
- **When**: The modal opens.
- **Then**: Local aggregates render immediately, one asynchronous `_kiro/account/getUsage` query runs, and the Costs page refreshes with plan, billing reset, credit used/limit, usage breakdowns, bonus credits, and overage fields; a failed refresh shows explicit status beside the last successful in-process value and never persists account data.

### Render typed charge and context metrics
- **Given**: The usage database contains monetary records, credit records, or both.
- **When**: The Cyril operator views Costs, Overview, Context, Models, Folders, Recent, Errors, or Tools.
- **Then**: Credits and real currency totals remain separate typed sections; Requests remains completed turns; Provider requests and Retries appear only when supplied; success, cancelled, and error counts remain distinct; the Context page shows the latest scalar, latest KAS five-bucket breakdown, and completed-compaction reductions with observable samples.

### Measure compaction gain
- **Given**: The same session supplies a scalar context sample immediately before a known compaction start and the first scalar sample after its completion.
- **When**: The Context page aggregates compactions.
- **Then**: That compaction contributes its non-negative percentage-point reduction to count, average, and total; a missing sample, failed compaction, or context increase contributes no fabricated gain.

### Preserve full-fidelity ACP usage
- **Given**: Standard ACP prompt usage and cumulative monetary cost are present, including an existing omp history.
- **When**: The same observer, database, aggregation, and modal handle the records.
- **Then**: Phase 1 token, cache, monetary-cost, TTFT, speed, grouping, recent, and error values remain unchanged; a future Kiro standard-usage frame activates those same fields without an engine-specific aggregation or rendering branch.

## Success criteria

- **Binary / structural**: KAS `turn_completion` fixtures map credits, elapsed time, status, request IDs, and used tools exactly, checked by captured-fixture conversion tests with literal expected values.
- **Binary / structural**: v2 metadata credits and KAS credits produce the same typed charge records while standard ACP money remains unchanged, checked by an adapter-equivalence state test.
- **Binary / structural**: aggregation and rendering contain no `AgentEngine`, `kiro`, `kas`, or `omp` decision branch, checked by crate-local identity-agnostic tests and a source scan limited to the aggregation/render modules.
- **Binary / structural**: a live record is committed before enrichment and repeated enrichment updates that record without adding a second turn or double-counting tool rows, checked by SQLite row-count and aggregate reconciliation tests.
- **Quantitative**: sidecar enrichment reads at most 64 MiB per file and performs at most 3 attempts over at most 1 second, measured by deterministic oversized, partial-write, and missing-file fixtures under a controlled clock.
- **Quantitative**: a 100,000-turn history still retains at most 20 recent and 20 error details in a snapshot, measured by the existing bounded-history stress check extended with credit/context/tool fields.
- **Binary / structural**: the Costs page visibly separates credits from currency and shows `n/a (backend-gated)` for unavailable Kiro monetary/token metrics, checked by TUI buffer tests for Kiro-only, full-fidelity-only, and mixed snapshots.
- **Binary / structural**: the combined Context page renders scalar, all five KAS buckets, and only sample-backed compaction gains, checked by TUI buffer and state-transition tests.
- **Binary / structural**: opening `/usage` never waits for `_kiro/account/getUsage`; exactly one query is dispatched per open with an active KAS session and the response refreshes the open panel, checked by App/bridge channel-order tests.
- **Binary / structural**: driving one real v2 turn and one real KAS turn shows credits, completed turns, outcomes, client duration/TTFT, context, tools, folder/model grouping, and backend-gated placeholders while the existing omp live acceptance remains unchanged, checked by the actual TUI/bridge smoke harnesses.

## Out of scope

This change does NOT include historical archive scanning or backfill; OTLP collection; fabricated token counts, monetary prices, cache metrics, or token speed; per-skill analytics; Behavior sentiment (cyril-tq2g); KAS focus/governance UI (remaining cyril-0o7e scope); new subscription fields not yet observed on wire (cyril-guml); or parsing/rendering `unsummarized_dropped` (cyril-0f4e).

## Related issues

- cyril-4h6i: parent usage-observer stage; requires engine-specific adapters with no engine-specific aggregation/viewer assumptions.
- cyril-gfkm: completed Phase 1 substrate; its standard-ACP record, SQLite aggregation, client timestamps, and eight-page modal are the extension point and must retain omp fidelity.
- cyril-0o7e: prior KAS `turn_completion` consumer ticket; this change adopts its credit/duration/used-tools shape but does not absorb its unrelated focus/governance behaviors.
- cyril-79df: `requestIds[]` capture; this change adopts request-count/correlation data for usage records.
- cyril-1gim: authoritative v2 metadata inventory and retain-last metering semantics.
- cyril-guml: watches unobserved future subscription/overage fields; no speculative parsing here.
- cyril-0f4e: separately owns operator-visible `unsummarized_dropped`; this change must not silently absorb that unrelated rendering fix.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Who consumes this feature? | The Cyril operator using the local `/usage` modal. | The issue extends the existing local usage viewer; no remote or multi-tenant surface is requested. | All behavior is local to the running Cyril process and its durable usage database. |
| Does latency depend on an OTLP collector? | No. | cyril-kryv tracker note resolves this: Cyril's prompt dispatch and first-text/turn-end timestamps are the operator-facing latency oracle. | Missing OTLP infrastructure never removes TTFT or duration. |
| What represents absent Kiro token and monetary-cost data? | `n/a (backend-gated)`, never numeric zero. | Explicit acceptance criterion and six-model evidence in `docs/kiro-2.19.1-wire-audit.md`. | Aggregation preserves absence; rendering distinguishes backend gating from real zero. |
| Is per-skill usage included? | No. | cyril-kryv evidence shows neither Kiro nor omp emits skill-invocation records. | No skills page or inferred skill attribution is added. |
| Which part of cyril-0o7e is absorbed? | KAS `turn_completion` metering only. | Focus and governance are separate observable features. | This change maps credits, elapsed time, status, request IDs, and used tools; focus/governance remain in cyril-0o7e. |
| Does this change parse `unsummarized_dropped` compaction status? | No; cyril-0f4e owns that behavior. | It is an operator-visible history-loss notification, not usage aggregation. | Compaction gain may consume known started/completed events without changing unknown-status handling. |
| How do Kiro session sidecars participate? | Current-session enrichment. | Requester selected “Current-session enrichment.” | After a live turn, Cyril reads only that session's sidecar to enrich its record with exact tool names and argument/result sizes; it does not scan archives or backfill prior sessions. |
| When is KAS account usage queried and shown? | On `/usage` open. | Requester selected “On /usage open.” | The modal opens from local data without blocking; with an active KAS session Cyril asynchronously requests account usage and refreshes the Costs page when the response arrives. |
| How do credits and monetary cost coexist? | Separate typed sections on one Costs page. | Requester selected “Separate typed sections.” | Kiro records aggregate credits without pretending they are currency; mixed histories preserve real monetary totals independently; Kiro-only monetary cost renders `n/a (backend-gated)`. |
| Is the engine-neutral Behavior sentiment page part of cyril-kryv? | No; intended future work is cyril-tq2g. | Requester selected “Exclude from this issue”; Phase 1 did not implement the presumed carry-over substrate. | cyril-kryv does not capture assistant text for sentiment or add a Behavior page. |
| Where do context usage and compaction gain render? | One combined Context page. | Requester selected “Combined Context page.” | The page shows latest scalar context percentage, latest KAS five-bucket breakdown, compaction count, and average/total percentage-point reduction only for compactions with observable before/after samples. |
| What happens when current-session sidecar enrichment is unavailable? | Persist first, then best-effort enrich. | Requester selected “Persist then best-effort enrich.” | A successful live turn is never lost or marked failed because enrichment failed; exact tool fields remain absent, portable ToolKind counts remain, and the failure is logged with path/error context. |
| How are turn credits and tool text sizes attributed? | Per distinct tool-call instance. | Requester selected “Per call instance.” | Turn credits are divided equally across distinct `ToolCallId` values and then aggregated by exact name when available, otherwise by portable `ToolKind`; raw argument/result sizes count Unicode scalar characters for every call. |
| What is the sidecar-enrichment size limit? | 64 MiB per current-session sidecar. | Requester selected “64 MiB cap.” | KAS JSONL is streamed; v2 monolithic JSON is rejected above 64 MiB; the already-persisted live record remains valid and a warning names the skipped path and size. |
| How long does sidecar enrichment wait for a writer? | At most three attempts within one second. | Requester selected “Three attempts within 1 s.” | Missing or partial sidecar data gets two bounded-backoff retries; after one second exact details remain absent. Reopening `/usage` may schedule another bounded attempt. |
| What does the Costs page show when account usage cannot refresh? | Last known in-process value with fetch time and explicit status. | Requester selected “Last known with status.” | Account data is not persisted; a failure or absent active KAS session never masquerades as current data and never clears a prior successful response silently. |
| How do KAS `requestIds[]` affect Requests? | Existing Requests remains completed turns; add Provider requests and Retries. | Requester selected “Keep turns; add provider requests.” | When IDs exist, provider requests equal their count and retries equal count minus one; absent IDs stay unavailable rather than falling back to one, preserving Phase 1 semantics. |
| How does KAS `turn_completion.status` affect outcomes? | Aggregate success, cancelled, and error separately. | Requester selected “Success, cancelled, error.” | `success` increments success; `aborted` or a cancelled stop reason increments cancelled unless a recorded bridge/tool error exists; every other non-success status increments error and remains available for diagnosis. |
| Which model identity drives Kiro grouping? | The actually billed model wins when sidecar enrichment supplies it. | Requester selected “Billed model wins.” | Records preserve requested and billed identities; Models/Costs group by billed model when present and fall back to the dispatch-time requested model. |
| What happens for an empty usage database or absent optional metric? | Render the existing empty state; render metric absence explicitly. | Existing Phase 1 behavior and the no-sentinel project invariant. | Empty collections do not become errors; absent Kiro-gated metrics use the backend-gated label, while generally unreported optional fields use `n/a`. |
| What maximum durable history remains supported? | At least 100,000 turns with bounded detail retention. | Adopted from cyril-gfkm's scale fence. | Schema migration and new joins must preserve the 20 recent/20 error detail bound and SQL aggregation shape. |
| How are invalid numeric/unit fields handled? | Reject that field with a contextual warning; never fabricate a default. | Repository trust-boundary and no-sentinel invariants. | Negative/non-finite charges, overflowed counts, empty units, and invalid percentages do not enter aggregates. |
| How are multiple charge units combined? | Only exact equal units aggregate together. | Credits are not money and currencies cannot be mixed. | Credits, USD, and any future backend unit retain separate totals; prompt-summary entries with unlike units never sum into one number. |
| What happens under concurrent sidecar and SQLite writes? | SQLite remains transactional/WAL; sidecar parsing retries a bounded partial write and enrichment is an idempotent update. | Existing store concurrency contract plus the selected enrichment retry policy. | Readers never see partial tool replacement and a concurrent writer cannot duplicate a turn. |
| What happens on filesystem permission denial or KAS account authentication failure? | The local record/modal remains usable and shows/logs the specific unavailable source. | Enrichment and account context are supplementary to the completed live turn. | No permission/auth failure is collapsed into empty success or attributed to the agent turn. |
| What happens when only some enrichment fields parse? | Apply no partial replacement for that attempt. | Atomic enrichment prevents mixing tool rows from different sidecar snapshots. | A later retry may replace the exact-tool set; otherwise the portable live fields remain authoritative. |
| Are repeated account queries and enrichment attempts idempotent? | Yes. | The modal may reopen and the writer may settle after the first attempt. | One query runs per modal open; enrichment keys the same durable record and replaces its exact details atomically. |
| How are soft-deleted usage records handled? | N/A — the append-only usage log has no soft-delete state. | No deletion behavior exists in this change or Phase 1. | No tombstone semantics or recovery interface is introduced. |
| What are the multi-tenancy boundaries? | N/A — one local Cyril process and local database, with no remote tenant surface. | The named role is the local Cyril operator. | Account responses and sidecars never cross the operating-system account boundary. |
| How do time zones and DST affect metrics? | Stored turn timestamps remain Unix milliseconds; billing reset uses the agent's date string verbatim. | Durations/gains are monotonic or numeric and do not depend on wall-clock offset. | No DST arithmetic or inferred timezone enters usage aggregation. |
| How is replication lag handled? | N/A — SQLite and in-process account state are local and unreplicated. | There is no replicated store. | No consistency window or distributed cache is introduced. |
| How are open-modal snapshots invalidated? | Successful account refresh or sidecar enrichment refreshes the open modal; every new `/usage` open rebuilds from SQLite. | The operator selected query-on-open and best-effort enrichment. | The modal never requires restart to see a completed refresh and never labels stale account data as current. |

## Approval

Requester approval (verbatim): "Approve specification"
Date: 2026-08-21
