# cyril-j1b3 design: session-scoped approval preview joins

## Purpose

KAS sends `session/request_permission` with a stub `toolCall`; the full tool-call payload arrives in an earlier `session/update`. Cyril must carry the full payload into the approval surface without moving human decision-making into the bridge loop or changing permission responses.

The prove-it probe established four ordered Write File pairs in the committed KAS fixture. Cyril's current conversion helper recovers each `rawInput`; the approval widget currently does not render its `tool_call` fields. This design deepens the existing conversion and approval modules rather than introducing a second permission path.

## Input shapes

The implementation and fences cover these production-reachable shapes:

- **Identity**: matching `(sessionId, toolCallId)`, same tool-call ID with a different session, missing session ID, empty tool-call ID, and a request whose matching record is absent.
- **Notification kind**: initial `tool_call` and partial `tool_call_update`; an update may carry title, kind, status, raw input, content, locations, and raw output independently.
- **Optional payload fields**: raw input present or absent; content present or absent; locations present or absent; raw output present or absent. A permission request may carry only the required stub fields.
- **Collections**: empty, one, many distinct, and repeated content/location entries.
- **Tool kinds**: Read, Write (ACP Edit/Delete/Move), Execute, Search, Think, Fetch, SwitchMode, and Other.
- **Statuses**: InProgress, Pending, Completed, and Failed.
- **Strings and paths**: empty and non-empty values; ASCII, Unicode, spaces, relative paths, absolute paths, and malformed non-string JSON values.
- **Request timing**: recorded notification before request, request before notification, subsequent update while the approval is open, and two requests sharing one tool-call ID.
- **Host state**: target file present, absent, changed, or inaccessible. The preview path never reads the host filesystem.

The design does not add support for arbitrary binary content blocks because the current `ToolCallContent` projection already ignores non-text ACP content; the approval surface reports `Preview unavailable` when no displayable path/content/diff remains.

## Claims

1. A permission request joins only to the tool-call record with the same ACP `SessionId` and non-empty `ToolCallId`.
2. The request-time joined snapshot preserves the latest non-empty title, raw input, content, locations, and raw output fields while retaining request fields only where the snapshot has no value.
3. The approval surface shows the joined path and displayable proposed content or diff using the existing bounded presentation rules.
4. Missing, malformed, incomplete, cross-session, and out-of-order joins leave the approval options actionable and expose `Preview unavailable` without waiting.
5. Each permission request owns one snapshot and responder; duplicate requests do not share approval state.
6. The join is generic across approval tool kinds, while option selection, trust persistence, response serialization, and host execution remain behaviorally unchanged.
7. The bridge remains non-blocking: the bridge loop forwards the already-built `PermissionRequest` and never waits for the operator's approval.

## Architecture and placement

### Owner

- **`cyril-core::protocol::client::KiroClient`** owns a private session-scoped tool-call ledger because the same protocol adapter receives both `session/update` notifications and `session/request_permission` requests, and the ledger lifetime is exactly one ACP client/agent subprocess.
- **`cyril-core::protocol::convert`** owns ACP-to-domain conversion and the ledger's field-merge helper. The conversion seam already owns `acp::*` imports and keeps them out of UI crates.
- **`cyril-ui::state::UiState` and `cyril-ui::widgets::approval`** own approval state and rendering. The widget consumes the enriched domain `ToolCall` through `TrackedToolCall` accessors and applies the established path/content/diff projection.

### Seam

No external interface is added. The existing `KiroClient` → `PermissionRequest` → `UiState::show_approval` interface remains the seam. A private `ToolCallLedger` implementation hides keying, snapshot, and merge mechanics behind two operations: observe a notification and snapshot a permission request. Its interface is private because only one adapter currently fills the slot.

### Forbidden placement

- `cyril-ui` must not import ACP types, parse raw JSON, perform the join, or read the host filesystem.
- `cyril::App` must not look up tool calls or reconstruct preview content; it only forwards the existing `PermissionRequest` to `UiState`.
- `bridge::run_loop` must not await the approval responder or add a pending-request queue; ADR-0004's non-blocking human-decision invariant remains intact.
- Permission response conversion, trust persistence, and host-callback adapters must not be modified to compensate for missing preview data.

## Implementation shape

1. Replace the raw-input-only cache with a private `ToolCallLedger` keyed by typed `(SessionId, ToolCallId)` values. The ledger stores a domain `ToolCall` snapshot, not an untyped JSON-only side table.
2. On each initial `ToolCall` notification, convert and insert the domain snapshot. On each `ToolCallUpdate`, convert a partial domain update and merge non-empty fields into the existing snapshot using `ToolCall::merge_update` semantics.
3. In `request_permission`, look up the exact session/tool-call key before constructing `PermissionRequest`. When found, clone the snapshot and use request fields only to fill absent values. When absent or invalid, convert the stub and log the reason; the UI receives the stub and renders the degraded line.
4. Keep the snapshot in `PermissionRequest`; do not retain a live ledger reference in UI state. This makes a pending approval immutable when subsequent notifications arrive.
5. Extend the approval widget with a compact preview section. Reuse `TrackedToolCall::primary_path()` for path precedence, existing diff rendering and its 20-line cap, and the existing five-line output-style cap for raw-input text fallback. When no displayable path/content/diff exists, render `Preview unavailable` while retaining the option list.
6. Add conversion fences for identity, merge, and fixture replay; add UI fences for path/text, diff, fallback, bounds, and snapshot behavior. Leave response and host-execution tests unchanged except for proving they still pass.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|---|---|---|---:|---|---|
| 1 | Only the exact `(SessionId, ToolCallId)` record joins. | Feed two records with the same tool-call ID but different sessions, then request the first session. A preview from the second session falsifies the claim; a string-only cache is the specific buggy implementation. | A Python raw-frame oracle keyed by the two independent wire fields. | 5m | passed for the four captured pairs; cross-session case pending implementation | `convert::probe_j1b3_identity_matrix` |
| 2 | Non-empty snapshot fields survive request construction and partial updates. | Start with raw input, content, and location, apply an update that omits them, then construct a stub permission request. Missing any original field falsifies the claim; replacing the ledger with raw-input-only storage is the specific buggy implementation. | A hand-written field-presence matrix over the raw ACP JSON, independent of the domain merge helper. | 10m | pending | `convert::tool_call_ledger_preserves_partial_fields` |
| 3 | Joined Write File data is visible as path plus proposed text or diff, with existing bounds. | Render four fixture-derived approvals and a 21-line diff/6-line text case. Missing path/text, more than 20 diff lines, or more than 5 fallback lines falsifies the claim; leaving `approval.rs` unchanged is the specific buggy implementation. | Expected strings and line counts extracted directly from fixture `rawInput`/`content`, not from Cyril's renderer. | 15m | pending | `widgets::approval::renders_joined_preview` and `widgets::approval::bounds_preview` |
| 4 | A failed join stays actionable and visible as degraded preview. | Exercise missing ID, no match, cross-session, request-first, malformed path/text, and empty content. A missing option or absent `Preview unavailable` falsifies the claim; returning `Cancel` or awaiting a queue is the specific buggy implementation. | Direct state assertions over the constructed `PermissionRequest` plus the rendered buffer's option labels. | 10m | pending | `widgets::approval::degraded_preview_keeps_options` |
| 5 | One request has one immutable snapshot and responder. | Show two requests for one ID, then mutate the ledger and confirm each approval's preview and responder remain independent. Shared preview mutation or one response resolving both falsifies the claim; a shared `Rc<RefCell<ToolCall>>` is the specific buggy implementation. | A deterministic sequence of domain values and two independent oneshot receivers. | 10m | pending | `state::approval_snapshot_is_independent` |
| 6 | Approval choice and host execution semantics are unchanged. | Replay every selectable option in existing permission captures and compare each serialized option ID with the reference output. Any changed option ID or trust metadata falsifies the claim; changing `from_permission_response` is the specific buggy implementation. | Existing cyril-qo13 reference-capture oracle, independent of preview rendering. | 10m | pending | Existing `probe_qo13` regression suite |
| 7 | The bridge does not wait for human approval. | Place a permission request, then enqueue a notification while the operator responder remains unresolved. If notification forwarding stops until the responder resolves, the claim is false; awaiting the responder in `run_loop` is the specific buggy implementation. | Channel observation with a delayed responder and no UI renderer. | 15m | pending | Existing bridge permission-forwarding test plus a no-await assertion at the request arm |

The cheapest falsifier was Claim 1's captured-pair replay. The real conversion probe passed 4/4 rows, and the independent Python oracle produced the same four IDs, paths, and byte counts row-for-row. The cross-session synthetic case remains a build-time fence because the current raw-input cache intentionally does not yet carry session identity.

## Negative space

- No permission option, trust tier, response encoding, or authorization policy changes.
- No host filesystem read, write, terminal execution, or KAS protocol emission changes.
- No approval queue, timeout, retry, or deduplication behavior.
- No live preview refresh after the request-time snapshot.
- No cache eviction redesign or new vendor-neutral host-callback interface.

## Design risks

- A ledger keyed only by `ToolCallId` would pass the captured single-session fixture and fail the cross-session collision fence; the typed pair key is mandatory.
- An approval widget that renders only diff content will miss the KAS request-time `rawInput {path,text}` shape; the raw-input text fallback must be explicit.
- Reusing a mutable ledger reference would make the operator's preview change while the cursor is on an option; `PermissionRequest` must own cloned data.
- Adding the join to `run_loop` would violate ADR-0004 and risk blocking unrelated notifications; conversion remains in `KiroClient`.
