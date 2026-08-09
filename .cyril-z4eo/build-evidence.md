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
