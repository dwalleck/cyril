# cyril-0v42 — checkpointed-build audit (2026-07-05)

Four slices, one commit each, full gate battery per slice (kas + default
tests, clippy both feature sets, fmt, doctests, oracle rerun). Plan
deviation, noted in the slice-1 commit: resolver wiring + module-doc update
pulled forward from slice 3 so the helper landed warning-free under
`-D warnings` (an uncalled helper is dead_code, and `#[allow]` is
forbidden). Claim coverage unchanged.

## Claim → fence map (all green)

| Claim | Fence (permanent CI form) | Slice |
|---|---|---|
| C1 atomicity | `write_atomic_unwritable_parent_errs_target_intact` + probe S11/S12 audit trail | 2 |
| C2 mode kept | `write_atomic_preserves_existing_mode` (0755 + 0600 widening direction) | 1 |
| C3 fresh parity | `write_atomic_fresh_matches_plain_create_mode` (File::create control oracle) | 1 |
| C4 symlink | `write_through_symlink_preserves_link` (resolver level) | 3 |
| C5 byte-exact | `write_atomic_empty_and_unicode_byte_exact` + existing `write_creates_parents_and_exact_content` (unedited) | 1 |
| C6 temp-in-parent | temp-creation message assert inside the C1 fence + probe S18 audit | 2 |
| C7 inert failures | `write_atomic_directory_target_refused_inert`, `write_atomic_dangling_symlink_refused_inert`, C1 fence | 2 |
| C8 readonly refused | `write_atomic_readonly_target_refused_inert` (cross-platform) | 2 |
| C9 dep hygiene | `cargo tree -e normal`: 0 tempfile hits default / v3.27.0 under kas; Cargo.lock unchanged; CI kas legs | 4 |
| C10 non-blocking | **live smoke `kas_fs_host_io_smoke` PASSED** (real KAS turn, 9.5s: read+write resolvers served through the new atomic path) | 4 |

## Final integration check

- kas tests: 525 passed (4 skipped = the manual-gated smokes)
- default tests: 822 passed
- clippy `-D warnings`: cyril-core kas ✓, workspace default ✓, cyril kas ✓ (CI mirror)
- fmt --check ✓, doctests ✓
- prove-it oracle: **42/42** (final rerun)
- live smoke: **PASSED** against kiro-cli 2.11.0 / KAS bundle

## Notable en-route finding

clippy `permissions_set_readonly_false` caught the readonly-fence teardown
making the file world-writable on unix — fixed by restoring the captured
original permissions (also the correct Windows story, where a readonly file
blocks unlink). A gate-hygiene reminder: an earlier battery run piped
through `tail` masked this failure's exit code; reran bare per the
ship-skill convention.
