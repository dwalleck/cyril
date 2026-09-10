# Checkpoint records — cyril-v19o

Gate states for each completed slice, in `checkpointed-build`'s nine-item order.
Every item is `PASS`, `FAIL`, or `N/A — reason` with the evidence it rests on.

## Slice 1 — KAS powers push becomes a typed domain catalog

Commit: `<filled by the commit>` · Claims: C1, C2, C3 (+ C0's provenance artifacts)

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-core --features kas` → 1008 passed, 0 failed; `cargo test -p cyril-core -p cyril-ui --all-features` and `cargo test -p cyril --features kas` → all suites ok; `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0; `cargo fmt --check` → clean |
| 2 | Falsifiers (C1, C2, C3 were `PENDING`) | PASS | Discharged: the three claims' falsifier experiments are their fence tests, which this slice ran green — `powers_frame_maps_every_field_from_the_capture`, `empty_catalog_is_loaded_not_dropped`, `malformed_powers_frames_drop_and_never_clear` (plus the engine half `kas_engine_dispatches_powers_and_v2_does_not` and the session half `powers_catalog_replaces_and_survives_malformed_pushes`) |
| 3 | Stress fixture | PASS | Malformed shapes (absent / `null` / object / string / non-object item / missing `name` / non-string `name` / second item unnamed) all drop; unknown extra keys convert with known fields only; 200-item catalog converts whole (no truncation, no dedup); absent/empty optional fields fall back without placeholders |
| 4 | Implementation vs independent oracle | PASS | Throwaway dump test emitted the converted catalog as TSV; `jq -r '.powers[] \| [.name, .displayName, .description, (.mcpServerNames\|join(",")), (.hasSteeringFiles\|tostring)] \| @tsv'` over the same fixture produced the oracle rows; `diff` → **identical, 3 rows × 5 fields**. Harness removed after the run |
| 5 | Module shape | PASS (slice scope) | `python3 .cyril-v19o/oracles/module_shape.py --slice 1` → `PASS C8 module ledger holds`, `PASS C7 no powers request string in cyril-core`, `PASS C1 wire keys confined to convert/kas/powers.rs`, plus `PENDING C8 widgets/powers_panel.rs owned by slice 2`. Protected parents: `app.rs` 0 lines; `state.rs` +6 lines for the exhaustive-match arm (allowed region) |
| 6 | Production-scale budget | PASS | 1000-item push: **931 µs** average over 100 frames (test profile, `opt-level = 1`) against the plan's 1 ms ceiling; production N ≈ 3 → ~3 µs. One `O(N)` pass per push, no per-session work |
| 7 | Regression fence green | PASS | The five fences above are green on the assembled slice; `cargo test -p cyril-core --features kas` runs them in the same suite that gates the crate |
| 8 | Named mutation red | PASS | `bash .cyril-v19o/oracles/mutations.sh` → C1 swap-name-and-display-name RED, C2 empty-catalog-becomes-a-drop RED, C3 malformed-frame-becomes-an-empty-catalog RED |
| 9 | Fence restored green | PASS | The same script restores each file from a byte-exact backup and re-runs the fence: `fence GREEN after restore` for all three; `trap` guarantees restoration even on interrupt |

### Deviations from plan.md (all recorded in the plan's amendments)

1. **Fixture path moved** to `tests/fixtures/kas/powers/items-changed-2.21.2.json`. The plan named the flat `kas/powers-items-changed-2.21.2.json`, which broke `schema_deserializes_captured_kas_session_updates`: that fence deserializes *every* top-level `kas/*.json` as an `acp::SessionNotification`, so a non-session-update fixture there is a hard failure. `kas/workflow/` is the existing precedent for fixtures that are not session updates.
2. **`UiState::apply_notification` arm moved from slice 3 to slice 1.** Adding a `Notification` variant cannot compile without its arm in the exhaustive match at `state.rs:451`; the compiler named it (my pre-implementation grep had counted a catch-all that belongs to a different match). The arm's content is exactly what slice 3 would have written — an explicit `=> false` with the "the App refreshes only if open" comment — so slice 3's delta for `state.rs` is now 0 lines rather than 12.
3. **`crates/cyril/examples/test_bridge.rs` gained a `PowersChanged` print arm** (not in the plan's file list). Same cause: that match is exhaustive.
4. **`experiments/conductor-spike/` artifacts measured 468 lines** against the ledger's ~300 projection (the verdict JSON alone is 248). Placement is unchanged and the partition still holds; the plan's arithmetic was updated rather than the artifacts trimmed, because the verdict is the machine-readable form the directory's other probes already commit.
