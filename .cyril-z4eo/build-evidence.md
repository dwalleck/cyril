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
