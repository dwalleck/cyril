# cyril-caar — full startup correction proposal

## Current draft publication — 2026-10-22

**Native acceptance BLOCKED by authentication; draft only.** The approved design below remains governing. Deterministic actual-request tests establish wait policy; the live fixture establishes integrated responsiveness/order. No independent native timer-fire observer or new UI timing threshold is introduced.

The [current proof index](current-main-proof/index.json) owns source/binary identities, the sealed local qualification, independent-review applicability and pending gates. See [local qualification](current-main-proof/qualification.json), [review applicability](current-main-proof/review-applicability.json), [native attempts and blocker](current-main-proof/native-consumer.json), and the [draft PR body](pr-body.md). No native/CI/final-conformance/merge-ready or issue-closure claim follows from this publication. Logical delivery remains Tqr #193 → NK #196 → Caar; the draft targets main for existing CI and explicitly depends on #196.

## Retained approved contract and canonical history

Imported from the owning canonical Caar record. Approved requirements remain governing; historical source pins, patch recipes and check statuses below remain attributed to their named earlier frames, not to the current qualified source. Archival patches/private receipts referenced below are not implicitly shipped or authorized for reapplication. Current status is the proof index above.

## Route and inputs

Empirical route in `route.md`; exact issue behavior and the now-approved absent-inventory decision in `spec.md`; current source and safe live v2 result in `evidence.md` / `startup-status-2.32.0.json`. Inspected base `8e5c325e`; implementation follows the approved tqr8 → nk4o → caar stack. P1a (actual query shape/named pending) is answered; P1b (timely effective inventory for a genuinely stalled new) remains unproven, with spec D1's explicit unknown-inventory branch approved.

Do not invent an inventory timing guarantee or pretend a constant-only patch satisfies the complete issue.

## Proposed correction

1. Introduce a dedicated new-session response budget of **at least 90s**, with the ticket's pending-OAuth/30s race reason adjacent to the constant. Leave session/load at its current independent budget and ordinary control RPCs at theirs. Prompt response wait remains unbounded and its turn/liveness owners unchanged.
2. Keep sending new and awaiting its response off the serial domain loop. SDK handlers and the already-started host drain continue processing ready/failure/OAuth/status frames during the wait. Do not register event routes only after the response or gate these progress events on an active chat turn.
3. A private startup-attempt owner in the serial DomainMediator records engine-produced expected inventory when available, reported per-server outcomes, explicit OAuth waits and true wire session identity. It retains latest facts needed for a timeout; this is not a second append-only transcript or a Kiro filesystem resolver.
4. KAS status (`nk4o`) is an authoritative received current-pool snapshot only with its true session scope. V2 command advertisements/loading rows and the observed startup-status `pending` subset are useful facts but not proven complete effective startup inventories. The live v2 query is `_kiro.dev/mcp/startup_status {sessionId}`; its nonempty `failed` shape remains unestablished. Do not add polling or assume its result arrives before new finishes.
5. Match timeout as a typed timeout outcome, not by parsing ACP error strings. The spawned waiter returns a typed startup result to the serial domain owner; that owner composes the final diagnostic from facts received up to the timeout outcome. Distinguish timeout from transport/protocol/deserialization/fingerprint failure, preserving original cause and fatal startup semantics.
6. Timeout message names reported ready/failure outcomes, explicit awaiting-OAuth servers and **known** silent/unresolved names with their correct meaning. `silent` requires an authoritative expected set minus actually reported names; an already-announced connecting row is “announced but not settled,” not never-reported. Inventory not received/determinable must be visibly unknown, never `silent:none`.
7. When no complete inventory has arrived, explicitly state that the agent supplied no complete startup inventory. This is the approved spec D1 behavior, not fabricated silent names or a duplicate Kiro config resolver. Availability of complete inventory before a genuinely stalled new is no longer an unwaived acceptance prerequisite.
8. On success, preserve ordering `UsageSessionStarted → SessionCreated → initial config → matching staged MCP state` where applicable. Preserve actual early user-visible progress and diagnostic facts across the reset; do not relabel old/foreign frames as the new root. On timeout/failure, release attempt-owned staging, keep the existing visible fatal failure and teardown path, and prevent a late reply/event from publishing an orphaned/replacement session.
9. nk4o now owns the sole root-start admission policy in its existing MCP opening slot, as Main explicitly required after independent review. caar consumes `begin_mcp_session` before `Startup::begin` records an AttemptId; the diagnostic owner has no duplicate busy/admission policy. Keep every outcome generation-tagged, visibly reject competing new/load before a second root-start send, and never discard a remotely created success as a substitute for admission. Retain load's independent budget and classification (fingerprint rejection fatal; ordinary RPC/parse/timeout errors recoverable), and new's fatal failure semantics.
10. Preserve real optional v2 MCP event session IDs through conversion/routing rather than globalizing identity-bearing events. Sessionless legacy events may be summarized as temporally observed unscoped reports, not claimed as root ownership. Before a new response supplies its ID, keep scoped evidence qualified by that actual session ID; no first-candidate or only-candidate binding. A timeout without root correlation cannot elevate another session's complete pool into root inventory.

## Placement / ownership

| Owner | Decision | Forbidden |
|---|---|---|
| `domain_mediator/commands/mod.rs` | Separate new/load/control constants; keep established generic helpers where applicable. | Inflating all command waits; timeout string matching; changing prompt liveness. |
| `domain_mediator/commands/session.rs` | Build/send request, spawned new-session await, typed result classification and response metadata extraction. | Shared mutable startup ledger captured by async wait, UI message formatting, new config resolver. |
| Cohesive private startup owner under `domain_mediator/` | After nk4o admission, record a diagnostic generation and scoped known inventories/progress; produce the user-facing failure summary at serial outcome application. Consume only existing ordered domain ingress and actual engine facts. | A second busy/admission policy; UI types; SDK handlers executing domain state; polling/retry subsystem; duplicated Kiro config/governance resolution; arbitrary attribution across attempts. |
| Existing `domain_mediator/inbound.rs` / outcome application | Observe recognized raw/typed MCP facts in serial order, forward early events unchanged, promote only matching session snapshots after reset. | Second event loop, broadening ordinary turn ownership or accepting foreign data into root state. |
| Existing v2 converter and new `nk4o` KAS family | Preserve ready/failure/OAuth handling; typed KAS snapshots and any established v2 inventory conversion stay at their dialect seam. | Generic ACP/parser or App wire parsing. |
| App/UI | Existing startup failure/progress presentation via typed notifications; retain visible facts through established reset handling. | Timer policy, raw inventory calculation, pretending an acknowledgement means auth completion. |

This is a concrete private module, not a generic startup backend/trait. Removing it would spread attempt-fact joining and timeout diagnostics across the waiter/inbound/outcome paths, so the seam earns depth. App, generic HostMediator and SDK handlers are protected wiring parents. Compiler/dependency checks plus independent review establish placement; no custom source-parser fence is needed.

## Claims / falsification

All implementation/consumer checks are **PENDING — parent execution**, not performed by this worker.

| ID | Claim and covered cases | Falsifier / verification | Regression coverage |
|---|---|---|---|
| C1 | New has its own >=90s budget and ordinary/load/prompt behavior is unchanged. | Existing domain/SDK fake-agent harness with controllable time: new still pending past30s and succeeds at31–35s; genuine new timeout only at its dedicated deadline; control/load waits remain independent; no prompt response timeout introduced. Assert actual request path, not only constant value. | Parent runs added regression against base (red at30s), then repaired code green. Use virtual clock/observable waiter where supported; full real delayed-response consumer fixture also required. |
| C2 | Pre-response ready/failure/OAuth events continue through SDK ingress and remain consumer-visible through success/failure reset boundaries. | Fake agent emits ready at0.4s, failure, delayed OAuth after30s, then response; consumer sees correct events/URL without waiting for SessionCreated. Replay actual source/current envelopes, including true sessionId where present. Drop/gated route/reset erasure falsifies. | Domain serial/SDK and UI lifecycle tests; real consumer protocol fixture. Historical KiroCrew timing is input context, not this change's observed PASS. |
| C3 | Timeout diagnostics correctly distinguish reported, awaiting OAuth, known never-reported and announced-unsettled names; missing inventory is explicit and never fabricated. | Fake agent supplies an established inventory then settles only some entries; force actual new timeout and assert names/categories/cause in final visible disconnect. Repeat no-inventory branch: explicit agent-supplied-inventory absence, never false empty-silent. | New startup owner behavioral tests plus actual failing SDK request→consumer message test. Both complete and absent inventory branches implement approved spec D1. |
| C4 | Engine inventory acquisition uses established shapes and is not assumed earlier than observed. | Replay durable live v2 request/result and early ready frames, preserve their true optional session IDs, and replay KAS status before response. Do not upgrade v2 advertised/loading names or named pending subset into a complete expected set. A fabricated complete pre-start list or premature `failed` element parser fails review/tests. | Source-grounded captured fixtures; complete KAS pool only with correct scope, otherwise approved explicit unknown inventory. No new status polling dependency. |
| C5 | Failure/late replies/competing root starts do not create an orphan active slot or leak prior attempt/foreign session facts; transport/protocol causes remain visible. | Fake agent withholds response through deadline, then replies late; assert exactly one fatal startup failure, no SessionCreated from late reply and owned bridge/process cleanup. Competing new/new, new/load and load/new are visibly rejected before a second root-start request is sent. Include mismatch/closed transport and separately scope-qualified unrelated-session progress. | Existing serial/fail-stop harness plus lifecycle consumer teardown. No helper-only test claims process cleanup or invents root correlation. |
| C6 | Typed boundaries and prior v2/KAS behavior remain intact. | Default/KAS repository gates and independent review of assembled owners/callers; KAS callback continues while new waits, new SessionCreated/config ordering preserved. | Existing engine/SDK/domain/session suites and issue-specific regression. |

The 90s budget is a product requirement, not a microbenchmark threshold. Deterministic clock tests establish the wait policy; live fixture establishes integrated responsiveness/order. Do not run a repeated arbitrary latency benchmark, invent a second output oracle or widen the required budget until a test passes.

## Safe fixture / evidence

`probe-startup.py --mock-backend --native-auth-readonly` is the already-exercised raw v2 instrument for source premise P1. It uses read-only native auth through bubblewrap and writable owned roots only, with no model prompt/account write. It is not an implementation smoke or full acceptance oracle. The loopback fixture in `.cyril-tqr8/fixture.py` adds controlled slow, failed, pending-OAuth and optionally silent server behavior for parent-owned real consumer acceptance. Parent records exact source/version/environment/inputs and process teardown. Neither fixture's own manifest is evidence that Cyril correctly inferred an engine-resolved production inventory.

## PR order / boundary

Approved stack **tqr8 → nk4o → caar**. caar retains a substantive independent commit/PR for the separate response budget, private startup owner, early-event/timeout behavior and consumer regression/qualification. It may consume nk4o's typed KAS state but must not claim the panel or callback work as its whole issue. Each issue ends with independent code review and its own PR; parent owns builds/checkpoints, commits/push and publication.

No commits, tracker changes or PRs by worker. Permanent usage/changelog updates follow successful parent-owned consumer smoke as required by assignment; they are not prewritten claims of working behavior.

## Approval / unresolved gates

- **D1 APPROVED** — exact absent-inventory behavior is recorded once in `spec.md`; P1b is not misrepresented as observed.
- **Main APPROVED the full stack with this truthful inventory fallback and existing ownership/verification boundaries.** Routine same-contract implementation may proceed; no producer-owned inventory guarantee is invented.
- **Source implementation delivered under approved scope; parent red-before/green-after, real consumer/repository proof, independent review and PR remain pending.** The separate caar patch boundary retains budget, generation/fact diagnostics and consumer qualification; nk4o owns admission.

## Approved replacement-root progress repair

Main's review DG permits typed startup observation on existing RoutedNotification, stamped only by the admitted/current-generation startup owner. Distinguish UnboundNew from BoundRequestedLoad using existing typed generation/newtype conventions. Keep the actual notification/session scope, with no notification clone, App correlation ledger or inferred root identity.

For New, display only qualified unowned scoped progress outside known current/peer/workflow ownership. For Load, only the explicit requested target is eligible; that target may already be a tracked peer/workflow, so known-owner exclusion must not hide it. Qualified visibility does not promote that target or its registry. Existing domain startup begin(kind,target) owns this distinction; existing App durable presentation owns display/history.

Main750's corrected actual SDK→App12-case fixture establishes meaningful pre-fix sensitivity: ten intended durable-line failures, two foreign/post-completion passes; no fixture timeouts. Main then authorized production. Keep the12 behavior oracles, FIFO and success/failure history unchanged. Historical original446-line deadline source remains immutable at9c399949…; later Proof/F3's technical V2 prompt-fixture correction preserves C1 and both deadline case bodies, as recorded in review-decisions.md. F2's fingerprint preflight order remains intact. Post-fix default/KAS/App/repository/live/platform proof and independent re-review remain Main-owned.
