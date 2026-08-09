# Design: FIFO permission approvals with session attribution

## Purpose

Prevent concurrent ACP permission requests from destroying one another, preserve arrival order until every live request receives a terminal response, and identify approvals originating outside the main session.

The prove-it prototype established the current failure: two `show_approval` calls leave request 2 visible, close request 1's responder, resolve request 2, and leave no pending approval. Probe and independent ownership oracle agreed on all four observations; see `findings.md`.

## Decisions

1. `PermissionRequest` owns the exact typed `SessionId` created from `RequestPermissionRequest.session_id` in `KiroClient::request_permission`.
2. `UiState` stores approvals in a private `VecDeque<ApprovalState>`; the front is the only rendered and interactive approval, and `show_approval` appends at the back.
3. FIFO order is the order in which `App` receives permission-channel items and calls `show_approval`. No timestamp or cross-producer fairness layer is added.
4. A terminal response removes the front: immediate option selection, trust-tier confirmation, invalid/empty selection falling back to `Cancel`, or phase-1 Esc cancellation. A failed responder send is still terminal and also removes the front.
5. Entering trust selection and Esc from trust selection back to option selection are non-terminal; both keep the same front request and do not expose the next request.
6. Every `ApprovalState` retains its typed originating `SessionId`. At render time, the current `TuiState::session_label()` is compared with the active approval: equality omits attribution, inequality or an unknown main id displays the origin, and an invalid empty id renders as `unknown session` rather than a blank title. Re-evaluating each frame keeps queued attribution correct if the main session changes.
7. Duplicate session ids or tool-call ids do not deduplicate requests. Every request owns its request-time `TrackedToolCall` snapshot and responder and occupies one FIFO position.
8. The queue is process-local UI state. Dropping `UiState` closes all remaining responders as part of application teardown; this change does not invent persistence for live ACP request futures.
9. Trust confirmation returns the originating `SessionId` with the chosen `TrustOption`. `App` persists only a main-origin grant; a foreign-origin grant receives the correct wire response but is explicitly reported as session-scoped instead of being written into the main agent's config. Durable foreign-session persistence is tracked by `cyril-ufld`.

## Input shapes

### Session attribution

- Main id known; request id equals main: no overlay session label.
- Main id known; request id differs: raw originating `SessionId` appears in the overlay title.
- Main id absent; request id present: label the origin rather than silently treating it as main.
- Non-ASCII or long request id: preserve Unicode text exactly; rely on the existing Ratatui block clipping at narrow widths rather than mutate the identity.
- Empty wire session id: retain the exact empty `SessionId` in the core request, preserve the existing client warning, and render `unknown session` when attribution is required; do not reconstruct a plausible identity.

### Queue cardinality and identity

- Empty queue: `approval()` is `None`; `has_approval()` is false.
- One request: existing selection, trust, cancellation, preview, and responder behavior remains.
- Two or more distinct requests: FIFO head remains stable until terminal resolution; each resolution exposes the next request.
- Repeated session id, repeated tool-call id, or both: requests remain independent FIFO entries with independent snapshots and responders.

### Selection collections

- Empty `options`: confirmation emits `Cancel` and advances.
- One option: existing immediate selection semantics remain.
- Multiple options with distinct or repeated kinds: the exact selected `PermissionOptionId` remains authoritative.
- Empty `trust_options`: `AllowAlways` resolves immediately; non-empty trust options enter the second phase.

### Approval lifecycle

- `SelectOption` with a normal selected option: send `Selected`, then remove the head.
- `SelectOption` with `AllowAlways` plus trust options: enter `SelectTrust`, retain the head.
- `SelectTrust` confirmation: send the chosen phase-1 option id plus trust label, return the origin with the chosen trust tier, then remove the head.
- `SelectTrust` Esc: return to `SelectOption`, retain the head and restore its selected option.
- `SelectOption` Esc: send `Cancel`, then remove the head.
- Empty options or an out-of-bounds selection: warn, send `Cancel`, then remove the head.
- Responder open or already closed: attempt the same response; send failure is logged under existing behavior and never blocks queue promotion.

## Removed-invariant sweep

The subtractive move removes the `UiState` invariant “at most one permission approval is owned at a time.” That single slot previously guaranteed these facts for free:

- Every approval mutator addressed the only approval. The replacement invariant is that every mutator addresses only `VecDeque::front_mut()` or an owned `pop_front()` value.
- Rendering could expose at most one approval. This still holds because `TuiState::approval()` returns only `front()`.
- A terminal response left no approval. This no longer holds; callers may immediately observe the next head. `App` does not assume emptiness after confirm/cancel, and render/key dispatch already query `has_approval()` each event/frame.
- A trust-phase transition restored the single owned approval. It must now restore the same item at the front, never append it behind later requests.
- Request-time preview snapshots could not cross-talk. Queue entries retain whole `ApprovalState` values, so the cyril-j1b3 snapshot-stability invariant remains per entry.
- The single slot never had to survive a main-session identity change. Each queued item now retains its origin, and the render path compares it with the current main label instead of freezing a main/foreign decision at enqueue time.
- Trust confirmation previously returned only a `TrustOption`, implicitly attributing every persistence side effect to the main session. The enriched result preserves the popped head's origin so `App` can reject that false attribution.

No other modal becomes concurrent or changes priority: approval remains the highest-priority overlay and only its front entry participates in that policy.

## Architecture and placement

### `cyril-core::types::event`

Owns the domain request. Add `session_id: SessionId` to `PermissionRequest`; raw strings remain forbidden for identifiers. This is an extension of the existing type interface, not a new seam.

### `cyril-core::protocol::client`

Acts as the ACP adapter. Move the already-created typed `session_id` into `PermissionRequest` after it has served the session-scoped tool-call ledger lookup. No UI comparison or display decision belongs here.

### `cyril-ui::state`

Owns approval lifecycle. Replace the private slot with `VecDeque`, move each request's typed origin into its `ApprovalState`, and keep queue advancement behind the existing `show_approval`, `approval_confirm`, and `approval_cancel` interface. `App` must not receive queue operations or attribution policy.

### `cyril-ui::traits`, `render`, and `widgets::approval`

`ApprovalState` owns the exact originating `SessionId`. `TuiState::approval()` remains the read-only rendering seam and exposes only the current head. `render` compares that origin with the current `TuiState::session_label()` on every frame and passes optional attribution to the widget. The widget adds the foreign id (or `unknown session` for an invalid empty id) to both option and trust-phase overlay titles; it does not inspect session trackers or workflow registries.

### `cyril::App`

Consumes an origin-bearing trust-confirmation result. It preserves existing persistence for a grant whose origin equals `SessionController::id()`. A foreign or pre-main origin is not written to the main agent config; App reports that only the wire/session-scoped grant was applied. This is the one cross-component attribution decision because App alone owns both `SessionController` and the persistence adapter.

### Forbidden placements

- No ACP types outside `cyril-core::protocol::convert`/client adapter code.
- No queue or overlay-display attribution policy in `App`; only the cross-component trust-persistence guard belongs there.
- No bridge access or async work in `UiState`.
- No workflow-run or subagent lookup in the approval widget.
- No second approval interface alongside the existing methods.

## Claims

1. The ACP handler already has the exact typed wire-origin session id available before `PermissionRequest` construction.
2. Every forwarded `PermissionRequest` carries that exact `SessionId`, with no downstream reconstruction.
3. Two or more requests are presented FIFO and retain independent request-time snapshots and responders until each terminal resolution.
4. Trust-phase transitions retain the same head; only trust confirmation promotes the next request.
5. Cancellation, invalid selection, and a closed responder are terminal for the current head and cannot strand the next request.
6. Main-origin approvals omit attribution; foreign-origin and main-unknown approvals render the exact originating session id in both approval phases, attribution re-evaluates after main-session changes, and an invalid empty id renders an honest `unknown session` marker.
7. Empty and single-request behavior remains compatible with the existing public UI interface and option/trust response semantics.
8. Queue mutation stays private to `UiState`; external code receives only an immutable reference to the active head, so `App`, render, and key dispatch cannot reorder entries or consume responders directly.
9. Trust confirmation preserves its originating session: main-origin grants keep existing persistence, while foreign/pre-main grants never write into the main agent config.

## Falsification

| # | Claim | Falsifier | Independent oracle | Cost | Status | Regression fence |
|---|---|---|---|---|---|---|
| 1 | Exact typed wire id is already available | AST-query `KiroClient::request_permission` for `SessionId::new(args.session_id.to_string())`; absence in that handler falsifies the placement | The ACP input field named by the typed request contract | <1 min | **passed**: structural match at `client.rs:209` | Client forwarding test below for claim 2 |
| 2 | Forwarded request carries exact id | Feed `sessionId = "peer-session"` through `request_permission`, receive the channel item, and compare its field; any other value falsifies the claim | Literal session id in the input ACP fixture | 2 min | pending | `approval_join_tests::permission_request_preserves_originating_session_id` |
| 3 | FIFO snapshots/responders | Show requests 1 and 2 with distinct messages/raw inputs, resolve twice, and record heads plus both receiver values; head 2 before resolution 1, a closed receiver, or crossed snapshot falsifies | Explicit expected sequence `first → second → empty` and per-request payloads | 2 min | pending | rewrite `state::tests::approval_snapshot_is_independent` as FIFO regression |
| 4 | Trust transition retains head | Queue a second request, enter trust phase on the first, Esc back, re-enter, confirm; any visible second request before final confirm or wrong option id falsifies | The selected phase-1 id and fixed event sequence in the fixture | 3 min | pending | `state::tests::approval_trust_phase_keeps_queue_head_until_confirmed` |
| 5 | Every terminal path promotes | Exercise phase-1 Esc, invalid selection, and an already-closed receiver with a second queued request; any second head not exposed after each case falsifies its named subcase | Receiver state plus the literal second message for each table-driven case | 4 min | pending | `state::tests::approval_terminal_paths_promote_next_request` |
| 6 | Attribution is foreign-only, exact, and current | Render main, peer, pre-main, Unicode, empty-id, and main-session-replacement cases in both phases; missing peer/pre-main labels, a main label, changed Unicode text, stale classification after replacement, or a blank empty-id title falsifies a named case | Literal fixture ids, the explicit `unknown session` fallback, and rendered buffer text | 4 min | pending | `render::tests::approval_attribution_tracks_current_main_session` plus `widgets::approval::tests::renders_foreign_session_in_both_phases` |
| 7 | Empty/single behavior remains | Run existing approval state/widget tests and focused compatibility cases after storage changes; any changed response id/trust label, preview, or empty getter falsifies the relevant test | Existing accepted test fixtures from cyril-qo13 and cyril-j1b3 | 5 min | pending | existing approval test modules plus a new empty-queue assertion |
| 8 | Queue stays behind the UI seam | Compile a negative probe from outside `cyril-ui::state` that attempts to reorder the private queue and consume a responder through `TuiState::approval()`; either operation compiling falsifies the placement | Expected Rust privacy/move diagnostics for the two literal forbidden operations | 5 min | pending | private `UiState` field plus immutable `TuiState::approval() -> Option<&ApprovalState>` make actual external mutation a compile error |
| 9 | Trust persistence respects origin | Confirm the same trust tier once from main and once from a foreign session; a missing main write, any foreign write, or an origin-less confirmation result falsifies a named case | Literal request origins plus isolated temp agent-config contents before/after each case | 5 min | pending | `app::tests::foreign_approval_trust_is_not_persisted_to_main_agent` and existing main persistence tests |

### Cheapest falsifier result

Ran an AST structural query against the feature worktree:

```text
crates/cyril-core/src/protocol/client.rs:209
let session_id = SessionId::new(args.session_id.to_string());
```

The typed wire id is already computed inside `request_permission` and remains in scope through `PermissionRequest` construction. Claim 1 survived; direct move is structurally available and no reconstruction seam is needed.

### Non-vacuity mutations

| Claim | Buggy implementation that must fail its fence |
|---|---|
| 1 | Delete the typed capture or move session conversion downstream into UI code. |
| 2 | Populate the new field from `tool_call_id`, a constant, or `UiState`'s current main id. |
| 3 | Use `push_front`, overwrite the queue, pop twice, or reuse one snapshot/responder. |
| 4 | `pop_front` when entering trust selection or `push_back` when restoring it. |
| 5 | Return early after a failed send, or forget promotion on Esc/invalid selection. |
| 6 | Always show a label, never show one, compare against a subagent tracker, or label with the current main id. |
| 7 | Change exact option-id/trust semantics while refactoring storage, or report an empty queue as active. |
| 8 | Expose `VecDeque` through `TuiState`, move queue mutation into `App`, or add a second queue-control interface. |
| 9 | Drop the origin from `approval_confirm`, compare the wrong id, or call `persist_trust_grant` for every confirmed foreign grant. |

## Negative space

- Workflow run/node friendly attribution is tracked by `cyril-jxfu`; this change displays the raw peer `SessionId` available today.
- General modal lifecycle/priority consolidation is tracked by `cyril-kbgo`; approval remains on the existing highest-priority modal path.
- KAS consent-scope and v2 trust-option response-shape work is tracked by `cyril-gn07` and `cyril-sive`; response conversion is unchanged here.
- Durable persistence into a foreign session's owning agent config is tracked by `cyril-ufld`; this change prevents the known-wrong main-config write and reports the grant as session-scoped.
- Approval persistence across process teardown is not part of a live ACP request contract: the underlying responder future dies with the process, so persisting only the visual queue would create an unanswerable prompt.
- No proactive pruning or new capacity policy is introduced for responders that close while queued. Existing terminal-send logging remains the observable failure path, and FIFO presentation remains deterministic.

## Scale and complexity budget

- Storage: one `VecDeque<ApprovalState>`; $O(1)$ enqueue, head access, and promotion.
- Per-request memory: exactly one existing `ApprovalState`; no copied tool payload or responder.
- Interface growth: one `SessionId` field on each of `PermissionRequest` and `ApprovalState`; `approval_confirm` enriches its existing optional trust result with the origin; no new public method.
- Rendering: one optional session-id title fragment for the active head only; queued items do no render work.
