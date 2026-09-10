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

## Slice 2 — the panel paints the approved layout

Claims: C4, C6 (+ C5's ordering/refresh half)

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-ui --all-features` → 595 passed, 0 failed (lib) plus every integration suite ok; `cargo test -p cyril-core --all-features` ok; `cargo clippy --workspace --all-targets --all-features -- -D warnings` → 0 diagnostics; `cargo fmt --check` clean |
| 2 | Falsifiers (C4, C6 were `PENDING`) | PASS | Discharged: `title_falls_back_to_id_and_empty_strings_mean_absent` (C4), and C6's five widget fences — `layout_matches_the_approved_row_shape`, `empty_catalog_shows_placeholder`, `wide_title_clamps_to_the_panel`, `viewport_window_clamps_scroll`, `tiny_area_paints_nothing` |
| 3 | Stress fixture | PASS | CJK title at the panel width (truncation marker, border intact, content stops before it); 400-char description (marker, row stays inside the border); two MCP servers on one row; 12-power catalog with the offset at the end (window shows only the last power) and at the front (exactly five); 1000-power catalog renders the window only; zero-room area paints nothing; `displayName: ""` falls back to the id; duplicate display names both render, ordered by id |
| 4 | Implementation vs independent oracle | PASS | Two oracles. Ordering: `printf '%s\n' … \| sort -f` over the same titles produced exactly the sequence the state fence asserts. Layout: the fence asserts hand-written cell rows (0=title, 1=meta, 2=description) located from the popup's own `┌` border rather than from widget math |
| 5 | Module shape | PASS (slice scope) | `python3 .cyril-v19o/oracles/module_shape.py --slice 2` → `PASS C8/C7/C1`. `state.rs` delta = 84 added production lines, all inside `*powers*` methods / the panel accessor / the catalog field / the `PowersChanged` arm (the rewritten check classifies by enclosing function and skips the test region). Non-vacuity proven: an injected `sneaky_helper` in production code is caught and named; removed afterwards → clean |
| 6 | Production-scale budget | PASS | Always-on render phase at the pathological size of 1000 powers: **347 µs/frame** average over 100 `TestBackend` frames against the plan's 1 ms ceiling — the widget's work is bounded by the 5-power window, not the catalog. Harness removed after the run |
| 7 | Regression fence green | PASS | All six powers fences plus the two anti-rot fences (`widgets/mod.rs` coverage in `widget_theme_sources.rs`, the `theme.rs` include_str list) run green in the crate suite |
| 8 | Named mutation red | PASS | Re-proved after the fences changed mid-slice: C4 title-keeps-empty-display-name, C5 no-sort-on-open, C5 refresh-strands-the-viewport, C6 title-clamp-dropped, C6 steering-marker-dropped, C6 empty-catalog-placeholder-dropped — all RED, all GREEN after restore |
| 9 | Fence restored green | PASS | `mutations.sh` restores from byte-exact backups and re-runs each fence (`GREEN after restore` ×6); the slice-1 proofs were re-run in the same pass and still hold |

### Deviations from plan.md

1. **Two anti-rot fences required registration of the new widget module**, only one of which the plan anticipated: `crates/cyril-ui/tests/widget_theme_sources.rs` (`MODULES` 15 → 16) and `crates/cyril-ui/src/theme.rs::widgets_only_use_the_explicit_theme` (an `include_str!` list asserted against the directory count).
2. **`state.rs` gained the `PowersChanged` arm in slice 1, not slice 3** (recorded there), so this slice's `state.rs` delta is the panel cluster only.
3. **Fence text changed after the first mutation pass** (stress assertions added, blank-description assertion corrected), which invalidated the earlier mutation evidence under Evidence validity; all six slice-2 mutations were re-run against the final fences.

## Slice 3 — `/powers` answers, and the push refreshes without opening

Claims: C5 (command and App halves)

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-core -p cyril-ui -p cyril --all-features` → no failures in any suite; `cargo clippy --workspace --all-targets --all-features -- -D warnings` → 0 diagnostics; `cargo fmt --check` clean |
| 2 | Falsifiers (C5 were `PENDING`) | PASS | Discharged: `powers_without_catalog_reports_and_with_catalog_opens`, `powers_command_registered_and_parses`, `powers_push_updates_without_opening_and_command_opens`, `powers_panel_key_map` — the remaining C5 half, with the ordering half already discharged in slice 2 |
| 3 | Stress fixture | PASS | No catalog → one system line and no panel; `/powers enable datadog` → a usage line, not a panel request; known-empty catalog → the panel still opens (the placeholder's whole reason); catalog → rows handed over in wire order; two pushes with the panel open → contents replaced in place; five-vs-one-key page step at 12 powers; unrelated key → no-op, Esc → close |
| 4 | Implementation vs independent oracle | PASS | Full round trip replayed from the committed capture: `jq` over the fixture vs converter → session → command → App payload, diffed as TSV → **identical**, three rows × four fields. The command performs no transformation, so the oracle covers the whole path it sits in. Harness removed after the run |
| 5 | Module shape | PASS (slice scope) | `python3 .cyril-v19o/oracles/module_shape.py --slice 3` → `PASS C8/C7/C1` with `app.rs`'s delta confined to `dispatch_powers_panel_key` and the two marker-matched arms. Non-vacuity re-proven: an added `App` field is caught and named. The redundant line-based field check was deleted after it false-positived on a function parameter the region check had already covered |
| 6 | Production-scale budget | PASS | `/powers` at 1000 powers: **182 µs** average over 100 executions against the plan's 1 ms ceiling (clone + `sort_by_cached_key`); the push path was measured in slice 1 |
| 7 | Regression fence green | PASS | Command, registration, App push/command and key-map fences all green; the `Notification` variant's exhaustive-match arms in `state.rs` and `test_bridge.rs` compile and pass |
| 8 | Named mutation red | PASS | C5 push-opens-the-panel (App arm calling `show_powers_panel` unconditionally) RED; C5 no-catalog-answer-dropped (`Dispatched` instead of the system line) RED — both GREEN after restore, in a pass that also re-proved slices 1 and 2 (11 mutations total) |
| 9 | Fence restored green | PASS | `mutations.sh` prints `GREEN after restore` for all 11; `trap` restores on interrupt |

## Slice 4 — nothing can call a method that does not exist; the frame survives the wire

Claims: C7 (census), C8 (module ledger over the assembled increment)

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-core -p cyril-ui -p cyril --all-features` clean; **default-feature lane** also clean (the harness and the census compile without `kas`); `cargo clippy --workspace --all-targets --all-features -- -D warnings` → 0 diagnostics *and* the same at default features; `cargo fmt --check` clean |
| 2 | Falsifiers (C7, C8 were `PENDING`) | PASS | Discharged: `no_production_source_names_an_unusable_powers_method`, `powers_census_detects_the_methods_it_exists_to_catch`, `powers_census_is_line_ending_agnostic`, `powers_push_survives_the_transport_and_draws_no_request` |
| 3 | Stress fixture | PASS | Census walks **170** `crates/*/src/**/*.rs` files; the transport fence replays the committed capture through the real SDK2 path and checks the three power names, `title()`, `has_steering_files()` and `mcp_server_names()` at that boundary |
| 4 | Implementation vs independent oracle | PASS | The capture's own method census (`jq`) lists exactly three powers methods: one push, one unadvertised, one unimplemented. Production code names **only** the push — the other two appear six times, all six on `//`/`//!` lines (verified by grep), which is precisely the comment/code split the scanner is built on. C8's oracle also runs at **full strictness** (no `--slice`) over the assembled diff: PASS |
| 5 | Module shape | PASS | `python3 .cyril-v19o/oracles/module_shape.py` (full) → PASS C8/C7/C1. Every rule proved non-vacuous by injection: wire field in a protected parent's struct → named; helper outside the allowed `app.rs` regions → flagged; `serde`/`from_str` in the widget → flagged; `ratatui` in the adapter → flagged; each clean after restore. The redundant line-based `App`-field check was **deleted** after it false-positived on a function parameter the region check already covers |
| 6 | Production-scale budget | PASS | No hot-path code added: the census is a test (0.02 s over 170 files) and the transport fence is one session's worth of frames. Slice 3's `/powers` measurement (182 µs at 1000 powers) remains the production figure |
| 7 | Regression fence green | PASS | All four slice-4 fences green in both feature lanes |
| 8 | Named mutation red | PASS | C7 an-unusable-powers-method-cannot-be-called (a `refresh` call injected into `state.rs`) RED; C5 push-dropped-at-the-engine (the engine arm removed, i.e. the frame silently unrecognized) RED — both GREEN after restore. Re-proved after `cargo fmt` retouched the fence text. Full suite: **13 named mutations** |
| 9 | Fence restored green | PASS | `mutations.sh` → `all 13 named mutations proved red/green`; `trap` restores on interrupt |

### The fence found a real defect in itself

The census's first run failed on a false positive: the fixture lives at
`tests/fixtures/kas/powers/items-changed-2.21.2.json`, so `include_str!` lines
contain the text `powers/items` followed by `-`. A path is not a method. The
scan now anchors on the `kiro/powers/` wire namespace, and the case is a
permanent assertion in `powers_census_detects_the_methods_it_exists_to_catch` —
the direction that matters, since a scanner that quietly stops matching is
worse than no scanner.

Also recorded for honesty: the transport fence asserts the push's **content and
provenance**, not its position relative to the bridge's local
`UsageSessionStarted` — the push races that local notification, and nothing in
the evidence pins the order (the capture's 18 ms gap is wall-clock). The claim
at stake is "arrives unprompted and survives normalization", and that is what is
asserted.

## Live acceptance — SC1, SC2, Esc (after slice 4)

Run against kiro-cli 2.21.2 from this worktree (`--features kas`,
`--agent-engine kas`, PTY-driven). Full frame and interpretation in
`evidence.md` §"Live acceptance".

* **SC1 — PASS**: the panel renders all three powers' `displayName` values in
  wire order.
* **SC2 — PASS**: `datadog` shows id + `mcp datadog` + `steering`;
  `aws-infrastructure-as-code` shows `awslabs.aws-iac-mcp-server` and no
  `steering` — matching the one capture entry with `hasSteeringFiles: false`.
* **Esc — PASS (live)**: the next typed text reached the input box and
  submitted, proving focus returned to the textarea.

The credential had expired between the slice-1 probes and this run; cyril failed
closed with an actionable message, and `kiro-cli whoami` refreshed the PKCE token
without a browser. Recorded because a reviewer will otherwise wonder how a
"live" check ran on a machine whose token needed `kiro-cli login`.

SC4 — PASS: `powers_submit_distinguishes_unloaded_from_empty` in `app.rs` drives
the real submit path: no catalog → zero panels and exactly one `System` message
("No powers reported yet…") with no bridge traffic; known-empty → the panel
opens on its placeholder and adds no message.
