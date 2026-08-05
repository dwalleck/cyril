# cyril-j1b3 budgeted plan

Approved design: `.cyril-j1b3/design.md`. Plan unit: one falsifiable hypothesis per slice. The executor may deviate from advisory code when the oracle still agrees and budgets hold.

## Slice 1: Identity-safe tool-call ledger

**Claim:** A permission request joins only to the tool-call record with the same ACP `SessionId` and non-empty `ToolCallId`; each request receives a stable snapshot whose non-empty fields survive partial updates.

**Oracle:** The Python fixture oracle keyed independently by `(sessionId, toolCallId)`, plus hand-written expected values for cross-session and missing-field fixtures.

**Stress fixture:** Two tool calls with identical tool-call IDs in different sessions; a request with no record; an empty tool-call ID; a missing session ID; and a partial update that omits title, content, locations, and raw output after they were present.

**Loop budget:** Ledger lookup is `O(1)` expected and merge is `O(c + l)` for a bounded tool-call payload (`c` content items, `l` locations). Production scale is bounded by KAS tool payload size, far below 10^6 operations. No always-on wall phase is added.

**Files:**
- `crates/cyril-core/src/protocol/convert/mod.rs`
- `crates/cyril-core/src/protocol/client.rs`

**Code (advisory):**
- Introduce a private domain `ToolCallLedger` beside the client conversion seam.
- Key the ledger by typed `(SessionId, ToolCallId)` values.
- Observe initial calls and partial updates, using `merge_update` semantics.
- Convert permission requests by cloning the snapshot; on absence, keep the converted stub and log a structured reason.
- Remove the raw-input-only lookup path rather than keeping two joins.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture rejects cross-session, missing-key, empty-key, and lost-field behavior
- [ ] prove-it-prototype oracle still agrees with binary
- [ ] Loop and wall budgets hold at fixture scale

## Slice 2: Render joined approval previews

**Claim:** The approval surface shows the joined path and displayable proposed text or diff under the existing bounded presentation rules, while missing or unusable payloads remain actionable with `Preview unavailable`.

**Oracle:** Expected path, text, and diff-line counts extracted directly from the fixture's `rawInput` and `content` frames, independent of the widget renderer.

**Stress fixture:** Four captured KAS Write File approvals; a 21-line diff; a 6-line raw-input fallback; Unicode and path-with-spaces values; a payload with no usable preview fields; and option labels that must remain present.

**Loop budget:** Preview extraction is `O(v)` for visible bounded lines (`v ≤ 20` diff or `v ≤ 5` fallback lines) plus `O(p)` for path lookup. Diff construction remains the renderer's existing `O(old + new)` behavior on a bounded approval payload; production scale is one request payload and far below 10^6 operations. No always-on wall phase is added.

**Files:**
- `crates/cyril-ui/src/widgets/approval.rs`
- `crates/cyril-ui/src/traits.rs`

**Code (advisory):**
- Wrap `ApprovalState.tool_call` in `TrackedToolCall` or expose equivalent domain accessors.
- Add a compact preview section above options in the select-option phase.
- Render `primary_path()` when present.
- Render existing diff projection when diff content exists; otherwise render raw-input `text` or `content`, capped to five lines.
- Render `Preview unavailable` when no displayable path/content/diff exists.
- Preserve existing option and trust-phase rendering.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture proves path, content, bounds, fallback, and options
- [ ] prove-it-prototype oracle still agrees with binary
- [ ] Loop and wall budgets hold at fixture scale

## Slice 3: Snapshot, parity, and non-blocking integration fences

**Claim:** Duplicate requests are independent snapshots and responders; response/trust/host behavior remains unchanged; and the bridge loop forwards the request without awaiting the operator's decision.

**Oracle:** Independent deterministic construction of two permission requests and two receivers; the existing qo13 reference-response oracle; and channel-order observation with a deliberately unresolved responder.

**Stress fixture:** Two requests for one ID followed by a ledger mutation before either resolves; every selectable option from captured requests; and a permission request followed immediately by a notification while the responder is not consumed.

**Loop budget:** Integration fences iterate `O(n)` captured permission requests (`n = 4` fixture writes plus existing traces) and no new production loop is added. Production bridge behavior remains one `O(1)` forward per request. No always-on wall phase is added.

**Files:**
- `crates/cyril-ui/src/state.rs`
- `crates/cyril-core/src/protocol/bridge.rs`

**Code (advisory):**
- Add a UI state test proving two approvals do not share preview mutations or responders.
- Keep existing response and trust conversion untouched and run the qo13 parity suite.
- Add a focused bridge test proving the loop forwards the permission request and continues processing an inbound notification with the responder unresolved.
- Avoid adding a queue, timeout, or response ownership in `run_loop`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture proves snapshot independence, exact option parity, and no-await forwarding
- [ ] prove-it-prototype oracle still agrees with binary
- [ ] Loop and wall budgets hold at fixture scale

## Plan self-review

### Loops

- Slice 1: `O(1)` expected lookup; `O(c + l)` bounded merge. No gap.
- Slice 2: `O(v)` visible-line extraction plus existing bounded diff behavior. No gap.
- Slice 3: `O(n)` replay with `n` bounded by committed captures; no new production loop. No gap.

### Fixtures

- Slice 1: identity collision, missing identity, empty identity, and partial-update field loss. More than happy path.
- Slice 2: four real captures, over-limit diff/text, Unicode/spaces, no usable fields, and option survival. More than happy path.
- Slice 3: duplicate request lifecycle, exact response parity, and unresolved-responder ordering. More than happy path.

### Doc-comment preconditions

- Ledger join identity is load-bearing for correctness: empty tool-call ID and missing session ID are refused at runtime through the lookup shape and logged; no doc-only precondition.
- Approval snapshot immutability is structural: `PermissionRequest` owns cloned domain data; no runtime precondition is documented.
- Renderer bounds are explicit constants enforced by tests; no caller precondition is added.

### Write targets

- Approval text is render output consumed by the terminal UI, not process stdout.
- Probe rows remain deterministic test artifacts; no new production stdout/stderr stream is introduced.
- Structured log messages are diagnostics and go through `tracing`.

### Tracker references

- No new deferral, follow-up, or out-of-scope tracker reference is introduced by this plan. Existing qo13 parity refers to the already-committed regression suite.
