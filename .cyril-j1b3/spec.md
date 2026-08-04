# Feature: Joined KAS approval previews

## What this is

When a KAS permission request carries only a stub `toolCall`, Cyril enriches the approval preview from the earlier tracked tool-call notification with the same `toolCallId` and ACP `sessionId`. The approval surface uses the existing tracked tool-call presentation rules without changing permission choices, response serialization, or host execution.

## Users

- **Cyril TUI operator**: reviews an agent action before selecting an allow or reject option and needs the same path, content, diff, title, and other preview data that the tracked tool call contains.

## Behavior

### Matched permission request

- **Given** a tracked tool-call notification and a permission request share the same non-empty `toolCallId` and ACP `sessionId`, and the tracked notification precedes the request,
- **When** Cyril handles the permission request,
- **Then** the approval preview uses a snapshot of the tracked tool call as its source of truth; request fields fill only fields absent from that snapshot.

### Generic approval enrichment

- **Given** any approval request has a matching tracked tool call,
- **When** Cyril constructs the approval state,
- **Then** the same join and precedence rules apply regardless of tool kind; existing non-write approval presentation remains unchanged except for receiving the richer tool-call data.

### Missing or invalid join

- **Given** a permission request has an empty or missing `toolCallId`, lacks a matching tracked tool call, lacks the required matching `sessionId`, or arrives before its tracked notification,
- **When** Cyril handles the request,
- **Then** the approval remains actionable, shows the request's valid title/message plus an explicit `Preview unavailable` line, logs the reason with the identifiers available, and does not wait for a later notification.

### Incomplete or malformed preview data

- **Given** a matching tracked tool call exists but has no usable path/content/diff data, or its `rawInput` has the wrong shape or non-string path/text fields,
- **When** Cyril renders the approval preview,
- **Then** it does not coerce arbitrary values; it shows the valid title/message plus `Preview unavailable`, logs the malformed or incomplete fields, and keeps the permission options actionable.

### Later tracked updates

- **Given** an approval preview was created from a matching tracked tool call,
- **When** a later notification updates that tool call before the operator confirms,
- **Then** the approval keeps the request-time snapshot; the later update may affect transcript state but does not change the pending decision's visible preview.

### Duplicate permission requests

- **Given** two permission requests reference one `toolCallId`,
- **When** they arrive,
- **Then** Cyril handles them as independent requests with independent snapshots and responders; it does not deduplicate them or share a decision.

### Bounded preview

- **Given** the joined tool call contains more preview lines than the normal tool-call renderer allows,
- **When** Cyril renders the approval preview,
- **Then** it reuses the existing omission behavior: at most 20 diff lines and at most 5 output lines where those categories apply, while keeping the approval choice actionable.

## Success criteria

- **Fixture coverage**: 4/4 `Write File` permission/tool-call pairs in `experiments/conductor-spike/kas-modes-dumps/bug-fix.json` produce a joined preview containing the tracked path and proposed content, measured by a cyril-core conversion regression test plus a cyril-ui approval-render test.
- **Fallback coverage**: 3/3 fallback classes (missing identifier, no matching call, malformed/incomplete payload) render an actionable approval with `Preview unavailable`, measured by deterministic state/render tests.
- **Identity isolation**: 100% of matched joins require both `sessionId` and `toolCallId`, measured by a cross-session collision test that refuses a same-ID call from another session.
- **Snapshot stability**: 1/1 later-update scenario leaves the approval preview byte-for-byte unchanged, measured by a state test that applies an update after showing approval.
- **Existing approval behavior**: 100% of existing approval response and selection tests pass, measured by the crate test commands covering `cyril-core` and `cyril-ui` approval behavior.

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| Zero matching tracked calls | Keep the dialog actionable and show `Preview unavailable`. | A missing preview must not strand the agent or silently remove the permission choice. |
| Maximum preview size | Reuse existing bounds: 20 diff lines and 5 output lines, including existing omission markers. | Prevents an approval overlay from expanding without bound and avoids a second presentation policy. |
| Missing or empty `toolCallId` | Degraded actionable approval; log the invalid identifier. | There is no safe join key, but the request can still be reviewed at its title/message level. |
| Missing `sessionId` or cross-session collision | Do not join; use degraded actionable approval and log the mismatch. | A tool-call ID alone is not sufficient across session scopes. |
| Permission request before tracked notification | Apply the missing-join fallback immediately; do not buffer. | The bridge contract is ordered and the feature must not add a pending-request queue. |
| Malformed `rawInput` shape or field types | Do not coerce; show degraded preview and log the malformed fields. | Arbitrary values must not be presented as a verified file path or content. |
| Matching call with no usable preview fields | Show degraded preview and keep options actionable. | The title/message remains useful, and the operator can make the existing permission decision. |
| Later tool-call update while approval is open | Keep the request-time snapshot. | The operator's pending decision must not change under their cursor. |
| Duplicate in-flight requests | Handle independently with separate responders and snapshots. | ACP requests are distinct; merging them would couple unrelated response lifecycles. |
| One failed join among multiple requests | Degrade only that request; continue handling other requests. | A local preview gap must not block unrelated approvals. |
| Permission rejection or cancellation | Preserve existing response and serialization behavior. | This ticket changes preview provenance, not authorization semantics. |
| File absent, changed, or inaccessible on the host | Do not read the filesystem for the preview; use only the tracked payload and degrade if it is unavailable. | Approval preview must represent the agent's proposed payload and avoid a second TOCTOU-sensitive read. |
| Non-write approval kinds | Apply the generic join, but preserve their existing renderer and bounds. | The join is a protocol correction, not a file-write-only special case. |
| Cache lifetime and eviction | No cache-lifecycle redesign in this ticket; the join is scoped to the live client/session identity. | Cache cleanup is separate from request-side preview correctness. |
| Time, retry, and timezone behavior | No time-based retry or expiry is introduced. | Request handling is immediate and event ordered. |

## Out of scope

This change does NOT include:

- changing permission option labels, ordering, selection, or wire serialization;
- changing trust-option persistence or authorization policy;
- changing KAS or ACP protocol emission;
- changing host filesystem or terminal execution;
- adding a pending-request queue or delayed approval timeout;
- redesigning the approval overlay beyond reusing existing tool-call presentation rules;
- reading the target file from disk to reconstruct a preview;
- redesigning cache eviction or session lifecycle.

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| Join identity | Exact ACP `sessionId` plus exact non-empty `toolCallId` | Conversion tests with matching, missing, and cross-session IDs |
| Request latency | 0 added wait time for a missing join | State test verifies the fallback is constructed in the same request handling path |
| Preview size | ≤20 diff lines and ≤5 output lines | Existing renderer-bound tests and approval rendering assertions |
| Decision safety | 0 permission choices lost because preview data is missing | Fallback tests assert the options remain present and confirmation still resolves |
| Scope | 1 approval request → 1 snapshot and responder | Duplicate-request state test |
| Filesystem access | 0 host reads during preview construction | Conversion/UI tests use only captured wire data and no filesystem fixture |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | Which named role defines acceptance? | Cyril TUI operator. | The runtime need is inspecting an agent action before approval. |
| 2 | What preview representation should be used? | Existing tracked tool-call presentation. | Avoids a second representation with divergent path/content/diff rules. |
| 3 | What if the tracked call is absent? | Keep approval actionable, show `Preview unavailable`, and log. | Missing preview is degraded observability, not a reason to block the agent indefinitely. |
| 4 | Which source wins when both contain a field? | Tracked notification wins; request fields only fill absent fields. | The tracked call carries the complete producer payload; the request is a KAS stub. |
| 5 | Does the join apply to all approvals? | Yes, generically. | The protocol shape is not limited to filesystem writes. |
| 6 | What if request order is reversed? | Immediate degraded fallback; no buffering. | The captured protocol is ordered and no new timing surface is needed. |
| 7 | What if the matched call lacks usable data? | Degraded actionable approval with logging. | Do not fabricate or coerce preview data. |
| 8 | How are large previews bounded? | Reuse current limits: 20 diff lines and 5 output lines. | Preserve established rendering behavior. |
| 9 | What if `toolCallId` is empty or missing? | Degraded actionable approval with logging. | There is no valid join key. |
| 10 | Should later updates refresh the dialog? | No; snapshot at request time. | A pending decision must remain stable. |
| 11 | How are duplicate requests handled? | Independently, without deduplication. | Each request owns a separate responder and option set. |
| 12 | What is the identity boundary? | Require both `sessionId` and `toolCallId`; missing session identity falls back. | Prevents cross-session joins when IDs collide. |
| 13 | What is out of scope? | Preview provenance only; no authorization, response, host-execution, protocol-emission, queue, or cache redesign. | Keeps the ticket to the request-side preview defect. |

## Sign-off

Agent's summary of the decisions:

> The Cyril TUI operator receives an approval-preview join only when both the ACP `sessionId` and non-empty `toolCallId` match. The tracked notification supplies the authoritative request-time snapshot; missing, malformed, incomplete, cross-session, or out-of-order data produces an actionable approval with `Preview unavailable` and a log. The change reuses existing preview bounds and does not alter authorization, responses, protocol emission, host execution, queues, or cache lifecycle.

The requester agreed: "I agree: match by session and tool-call ID, preserve the tracked snapshot, degrade visibly when unavailable, and keep the change preview-only."

Date: 2026-08-03
