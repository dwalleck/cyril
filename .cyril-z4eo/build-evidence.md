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
