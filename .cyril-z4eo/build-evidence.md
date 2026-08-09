# Checkpointed-build evidence

## Slice 1 — exact permission origin

- **Impact analysis:** `PermissionRequest` has 24 direct callers across 7 files (`tethys callers PermissionRequest --lsp`). Production construction is only `KiroClient::request_permission`; core bridge channels move the whole struct; UI has four test literals. `UiState::show_approval` has 15 callers across App plus state tests.
- **RED:** `cargo test -p cyril-core approval_join_tests` failed with E0026 because the test destructured the not-yet-existing `session_id` field.
- **GREEN/stress:** both real client-channel tests passed; the `sess-b`/shared-`toolCallId` fixture preserved `sess-b` and refused the `sess-a` ledger join.
- **Plan adaptation:** the public-field cutover required four `cyril-ui` test literals and the committed probe literal to gain explicit synthetic main ids in this slice. These are compile-only fixture migrations; no UI production behavior changed.
- **Oracle:** runtime probe and static ownership oracle both emitted `head1=second`, first `closed`, second `selected`, `head2=none`.
- **Budget:** no new loop, allocation, lookup, syscall, wait, or render work; the already-owned `SessionId` moves into the request.
- **Regression fence:** `approval_join_tests` passes for exact same-session and cross-session origins.
- **Full gates:** 1,234 default nextest tests passed (1 skipped); 1,468 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt and default/KAS doctests passed.

## Slice 2 — FIFO permission ownership

- **RED:** `approval_snapshot_is_independent` observed active message `second` instead of `first` after two `show_approval` calls.
- **GREEN:** the reversed cyril-j1b3 fence resolves request 1, leaves request 2 pending, promotes request 2, then resolves it without cross-talk.
- **Stress:** `approval_queue_resolves_all_responders_in_order` enqueues 64 requests with repeated session/tool ids and unique messages/options/raw inputs. It exercises immediate selection, phase-1 Esc, invalid selection, two dropped receivers, and AllowAlways enter/back/re-enter/confirm; every open future receiver remains pending until its head turn.
- **Oracle:** the runtime probe uses repeated identities and independent payloads; the static oracle requires `push_back`, `front`, `pop_front`, and trust-phase `push_front`. Both emitted `head1=first`, first `selected`, second `pending`, `head2=second`.
- **Budget:** production queue access is limited to `VecDeque::{push_back, front, front_mut, pop_front, push_front, is_empty}`. No queue scan, syscall, wait, or per-frame work proportional to queue depth exists; each operator event is $O(1)$.
- **Full gates:** 1,235 default nextest tests passed (1 skipped); 1,469 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt and default/KAS doctests passed.

## Slice 3 — active approval session attribution

- **Impact analysis:** rust-analyzer found 24 `ApprovalState` references. Production construction remains only `UiState::show_approval`; the atomic public-field cutover updated four test fixture sites and three direct widget calls without adding a second rendering seam.
- **RED:** `approval_attribution_tracks_current_main_session` failed to compile because `ApprovalState` had no `session_id`; after the field cutover, the behavioral fence distinguished the modal title from the toolbar label.
- **GREEN/stress:** equal-main omits attribution; foreign Unicode and pre-main states show the exact origin; changing the current main reclassifies the same approval on the next frame; empty origin shows `unknown session`. Both option and trust titles retain selected actions under a 24-column, three-row clamp with a 256-byte origin.
- **Fixture correction:** floor-test approval fixtures now model a known `main` session consistently in base and overlay frames. The roomy approval snapshot intentionally changes only its toolbar fixture from `No session` to `main`; the modal remains unattributed.
- **Oracle:** the repeated-id runtime probe now exposes `origin1=repeated-session`; the independent static oracle emits the same line and still agrees on FIFO ownership.
- **Budget:** render compares one active `SessionId` to one current label. Queued entries are not traversed or copied; title construction is $O(L)$ for the active attribution only, with no I/O or async work.
- **Full gates:** 1,237 default nextest tests passed (1 skipped); 1,471 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt and default/KAS doctests passed.

## Slice 4 — trust persistence origin guard

- **Impact analysis:** rust-analyzer found one production `approval_confirm` consumer (`App::handle_approval_key`) plus state tests. The existing method now returns one `(SessionId, TrustOption)` pair; no parallel API or reconstructed identity was added.
- **RED:** `foreign_approval_trust_is_not_persisted_to_main_agent` observed the foreign grant rewriting `<cwd>/.kiro/agents/myagent.json`.
- **GREEN/stress:** a foreign approval followed by an identical main approval resolves both wire responders in FIFO order. Foreign config bytes remain exact and a session-scoped notice names `peer-session`; main confirmation adds exactly `echo safe` and preserves unrelated JSON. A separate pre-main case also leaves config bytes exact and names `early-session`.
- **Oracle/smoke:** the public-API runtime probe and independent static FIFO oracle still agree, including exact `origin1=repeated-session`.
- **Budget:** App performs one typed-id comparison. Foreign/pre-main paths skip the persistence adapter entirely; the main path retains its existing single read/merge/atomic-write behavior.
- **Plan adaptation:** the App integration fixture required the existing workspace `tempfile` dependency as a `cyril` dev-dependency; this adds one package edge to `Cargo.lock` but no runtime dependency or new locked package.
- **Full gates:** 1,239 default nextest tests passed (1 skipped); 1,473 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt and default/KAS doctests passed.

## Pre-PR review corrections

- **Standards:** historical prototype findings now pin commit `955a1a3`; unattributed modal titles borrow static strings instead of allocating every frame.
- **Spec:** two external `compile_fail` doctests fence E0616 private queue access and E0507 responder consumption through `TuiState::approval()`.
- **Final gates:** 1,239 default nextest tests passed (1 skipped); 1,473 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt passed; default and KAS doctests each executed both compile-fail fences successfully.

## Post-PR review feedback

- **Assessment:** all eleven findings/observations were verified independently and recorded in `review-decisions.md` rows 6–16. Accepted/modified: typed main-session identity, shared invalid-origin presentation, non-empty persistence authority, one modal-title helper, an approval input-batch boundary, exact named lifecycle fences, and the truncated comment. Rejected with evidence: moving the co-located privacy fences, wrapping one conventional result pair in a public type, removing the necessary trust-persistence guard, removing the title allocation fix, and removing the approved compile fences.
- **Input safety:** `buffered_input_stops_before_promoted_approval` feeds Enter plus a buffered Enter to two immediate approvals. Request 1 resolves, request 2 is promoted but remains pending, and the buffered event is left for the outer loop after redraw.
- **Identity:** render compares `ApprovalState::session_id` with typed `TuiState::main_session_id`, independently of the human-facing toolbar label. A shared `approval_origin_label` projects an invalid empty id as `unknown session`; an empty origin cannot authorize main-config persistence even if the main controller also contains an empty id.
- **Focused fences:** 22 `cyril-ui` approval tests passed; the App buffered-input and empty-origin tests passed; the session-created integration test passed.
- **Runtime oracle:** `.cyril-z4eo/run-probe.sh` and `.cyril-z4eo/oracle.py` both emitted `head1=first`, `origin1=repeated-session`, first `selected`, second `pending`, and `head2=second`.
- **Full gates:** 1,243 default nextest tests passed (1 skipped); 1,477 KAS nextest tests passed (5 skipped); default and KAS all-target Clippy passed with `-D warnings`; fmt passed; default and KAS doctests each passed both compile-fail fences.
- **Post-fix recheck:** the parallel Spec recheck passed. The Standards recheck found one empty-main/empty-origin equality corner that suppressed honest attribution; render now requires a non-empty approval origin before omitting attribution, and the render fence exercises equal empty ids.
