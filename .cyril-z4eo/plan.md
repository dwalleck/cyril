# cyril-z4eo checkpointed implementation plan

Approved design: `.cyril-z4eo/design.md` (2026-08-09). Cheapest falsifier passed at `client.rs:209`. Each slice keeps the workspace compiling, lands as one conventional commit, and runs its named fixture plus the relevant probe/oracle before the next slice.

## Slice 1: Preserve wire-origin session identity

**Claims:** Design claims 1–2: `PermissionRequest` carries the exact `SessionId` already derived from the ACP request.

**Oracle:** The literal `sessionId` in `approval_join_tests::stub_permission`; no tool-call, ledger, main-session, or UI value participates in the expected answer.

**Stress fixture:** Send a stub permission for `sess-b` with a `toolCallId` also present under `sess-a`. Assert the forwarded request says `sess-b` while the preview does not join `sess-a`. A reconstruction from tool-call identity or active state fails.

**Loop budget:** No new loops. One existing owned `SessionId` moves into the request; zero additional allocation, lookup, syscall, or asymptotic work.

**Wall budget:** Request ingress remains one bounded-channel send plus the existing responder wait. This slice adds no wait, blocking operation, or render work.

**Files:**
- `crates/cyril-core/src/types/event.rs`
- `crates/cyril-core/src/protocol/client.rs`

**Smallest change:** Add the typed field, move the existing local into the sole production request literal, and extend the real client-channel fixture to assert exact provenance.

**Branches/error paths:** ACP session id non-empty/empty both move unchanged; ledger join hit/miss/cross-session arms already exist and the stress fixture covers cross-session miss. No new error return.

**Verification:**
- [ ] `approval_join_tests` passes, including exact origin and cross-session collision
- [ ] Stress fixture forwards `sess-b` and refuses the `sess-a` preview join
- [ ] `.cyril-z4eo/run-probe.sh` and `.cyril-z4eo/oracle.py` still agree on the current single-slot UI behavior
- [ ] No extra allocation or loop appears in `request_permission`

## Slice 2: Replace the approval slot with FIFO ownership

**Claims:** Design claims 3–5 and 7, plus the mutation half of claim 8: FIFO order, independent snapshots/responders, terminal promotion, trust-phase head retention, and unchanged empty/single behavior.

**Oracle:** Updated `.cyril-z4eo/oracle.py` derives the ownership result from `push_back`, `front`, and `pop_front`; `.cyril-z4eo/probe.rs` continues to call only public `UiState` methods with real oneshots. Expected first-resolution output is `head1=first`, first `selected`, second `pending`, `head2=second`.

**Stress fixture:** Enqueue 64 requests, deliberately repeating session/tool-call ids while giving each a unique message, option id, raw input, and oneshot. Mix immediate selection, phase-1 Esc, invalid selection, closed receivers, and one AllowAlways trust enter/back/confirm sequence. Every open receiver must resolve in enqueue order with its own data; trust enter/back must not expose entry 2.

**Loop budget:** New queue operations are `VecDeque::{push_back,front,front_mut,pop_front}` at $O(1)$ each. No queue scan is permitted. The stress depth is 64; production scale is $N$ concurrently outstanding permission requests, with $O(N)$ retained request memory and $O(1)$ work per operator event.

**Wall budget:** The always-on App receive path adds one $O(1)$ enqueue and no syscall. Only the active head renders; queued entries add zero per-frame work.

**Files:**
- `crates/cyril-ui/src/state.rs`
- `.cyril-z4eo/oracle.py`

**Smallest change:** Replace the private `Option` with `VecDeque`; map getters and selectors to the front; consume the front only on terminal paths; restore phase transitions at the front; reverse the cyril-j1b3 displacement assertion into a FIFO fence; update the static ownership oracle for the fixed shape.

**Branches/error paths:** `approval_confirm`: no head, immediate Selected, AllowAlways→trust, invalid/empty→Cancel, trust confirm, responder send failure. `approval_cancel`: no head, trust→option, option→Cancel, send failure. Each arm gets a named assertion; no error is suppressed beyond existing debug logging for a receiver already gone.

**Verification:**
- [ ] Focused `cyril-ui` approval lifecycle tests pass
- [ ] 64-request stress fixture preserves FIFO, payloads, and all open responders
- [ ] `.cyril-z4eo/run-probe.sh` output equals updated `.cyril-z4eo/oracle.py`
- [ ] No queue scan or per-frame work proportional to queue depth exists

## Slice 3: Render current foreign-session attribution

**Claims:** Design claim 6 and the read-only half of claim 8: the active approval retains exact origin; current-main requests omit attribution; foreign/pre-main requests render it in both phases; queue internals stay behind `UiState`.

**Oracle:** Literal main/origin ids rendered through Ratatui `TestBackend`. Expected buffer text is independent of queue implementation: `peer-α` appears for a foreign origin, never for equal main origin, and an empty foreign origin appears as `unknown session`.

**Stress fixture:** Render option and trust phases for equal main, foreign Unicode `peer-α`, pre-main, 256-character id, empty id, and a main-session replacement while the same approval stays active. Assert no extra body row, selected option remains visible under the smallest existing clamped popup, Unicode is preserved, long text clips, and replacement reclassifies attribution on the next frame.

**Loop budget:** No new queue loop. Title work is $O(L)$ in active session-id bytes only; queued depth $N$ contributes zero render operations. Fixture bound $L=256$, $N=64$; at most one title value is constructed per frame, with zero syscalls.

**Wall budget:** Per-frame addition is one equality check and title projection for the active head. No tracker lookup, allocation proportional to queue depth, I/O, or async work.

**Files:**
- `crates/cyril-ui/src/traits.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril-ui/src/render.rs`
- `crates/cyril-ui/src/widgets/approval.rs`
- `crates/cyril-ui/src/floor_tests.rs`
- `crates/cyril-ui/tests/modal_theme.rs`

**Atomic-cutover note:** Production logic changes in `traits.rs`, `state.rs`, `render.rs`, and `widgets/approval.rs`; `floor_tests.rs` and `modal_theme.rs` are fixture-only field additions. Adding a required public struct field is compile-atomic across existing literals. Splitting this cutover would require a temporary optional/sentinel origin or a no-op constructor parameter, both forbidden by repository rules; the six-file slice remains under 50 production lines and has one behavior.

**Smallest change:** Put exact `SessionId` on `ApprovalState`, move it from each request, derive optional current attribution in render from `TuiState::session_label`, and parameterize both widget titles without changing height or theme roles. Update every struct literal cleanly.

**Branches/error paths:** main equal, foreign, main unknown, Unicode/long, empty invalid; option/trust phase. Empty is an honest marker, never a fabricated id. No error return or documented caller precondition is introduced.

**Verification:**
- [ ] State/render/widget attribution tests pass for every named case
- [ ] Clamp stress fixture preserves the selected actionable row and existing modal height
- [ ] `.cyril-z4eo/run-probe.sh` still equals the fixed oracle and now retains origin internally
- [ ] Render work is independent of queued depth; no session-id copy occurs for queued entries per frame

## Slice 4: Guard trust persistence by approval origin

**Claim:** Design claim 9: trust confirmation preserves origin; main grants retain existing durable persistence, while foreign/pre-main grants never write into the main config and are reported as session-scoped.

**Oracle:** Isolated temporary agent-config contents before and after identical main/foreign confirmations. The literal request origin decides the expected file delta; the UI queue and current option labels do not.

**Stress fixture:** Create a main session using a writable temporary custom-agent config, enqueue a foreign approval first and a main approval second, choose the same AllowAlways trust tier for both, and confirm in order. Foreign confirmation must produce no file delta and one user-visible session-scoped notice; main confirmation must add exactly one expected pattern while preserving unrelated config fields.

**Loop budget:** No new loops. Confirmation returns one owned `(SessionId, TrustOption)`; App performs one $O(1)$ typed-id comparison. Foreign handling skips filesystem work. Existing persistence complexity is unchanged for main.

**Wall budget:** Foreign confirmation adds no syscall and one system-message append. Main confirmation retains the existing single read/modify/atomic-write path; no extra I/O.

**Files:**
- `crates/cyril-ui/src/state.rs`
- `crates/cyril/src/app.rs`

**Smallest change:** Enrich the existing optional trust result with the popped approval's origin; compare it to `SessionController::id()` in App; preserve current persistence only on equality; report foreign/pre-main session scope. Durable foreign-config mapping stays tracked by verified issue `cyril-ufld`.

**Branches/error paths:** no trust result; main equal; foreign; main unknown; existing built-in/no-config expected skip; existing genuine persistence error. The new foreign branch must not enter the persistence adapter. User-visible notice is diagnostic TUI state, not stdout/stderr data.

**Verification:**
- [ ] Main and foreign App trust tests pass with isolated config contents
- [ ] Stress fixture proves foreign zero-write, main one-write, and unrelated-field preservation
- [ ] `.cyril-z4eo/run-probe.sh` still equals the fixed oracle
- [ ] Foreign path performs zero filesystem operations; main path adds none

## Final claim coverage

| Design claim | Slice |
|---|---|
| 1–2 exact ACP origin | 1 |
| 3 FIFO ownership | 2 |
| 4 trust-phase head retention | 2 |
| 5 terminal promotion | 2 |
| 6 current foreign attribution | 3 |
| 7 empty/single compatibility | 2–3 |
| 8 private mutation/read-only render seam | 2–3 |
| 9 origin-safe trust persistence | 4 |

## Plan self-review

1. **Loops:** Slice 1 none; slice 2 only $O(1)$ deque operations (no scans); slice 3 title projection $O(L)$ for one active id with $L=256$ stress bound; slice 4 none. All are below $10^6$ operations and zero added syscalls except unchanged main persistence.
2. **Fixtures:** Each names a plausible bug: cross-session reconstruction, overwrite/LIFO/cross-talk/phase leakage, stale or malformed attribution with clamp pressure, and foreign-to-main config corruption. No fixture is happy-path only.
3. **Doc-comment preconditions:** No new caller precondition is planned. Empty session ids are handled at runtime with an honest display marker; invalid selections keep the existing runtime Cancel fallback.
4. **Write targets:** Core/UI queue/render slices write nothing. Trust persistence is user-requested config **data** through the existing atomic adapter; the foreign session-scoped notice is a TUI **diagnostic**, not process stdout/stderr.
5. **Tracker references:** `cyril-ufld`, `cyril-jxfu`, `cyril-kbgo`, `cyril-gn07`, and `cyril-sive` were verified. The plan defers only durable foreign-config ownership to `cyril-ufld`; no untracked deferral remains.
6. **Platform assumptions:** No platform-specific path/process behavior is added. Existing cross-platform config persistence is exercised through its isolated adapter tests, unchanged.
7. **Error paths:** Every new match/comparison outcome is enumerated above. No error type is added or flattened; responder-send failure retains its existing debug diagnostic and still promotes FIFO.
