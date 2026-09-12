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

## Final design-conformance review (gilfoyle `module-shape.md`)

**Isolation.** Two blank-context subagents, neither of which implemented the
change:

1. **Reconstruction — `ReconPowers`** (isolated worktree, instructed to read no
   `.cyril-v19o/` artifact, `docs/adr/`, or design/spec/plan document; production
   code only). Its full report is frozen at
   `.cyril-v19o/reconstruction-recon-powers.md`. It derived the four-cluster map
   (wire adaptation / domain vocabulary / catalog+user routing / presentation),
   the inbound and outbound seam chains, and the protected-parent growth split
   without seeing the design.
2. **Comparison — `ComparePowers`** (a second fresh reviewer that received the
   frozen reconstruction plus `design.md`, then verified every load-bearing
   claim in the code itself).

**Result: PASS.** All eleven approved ledger rows exist at the paths the ledger
names, each with the responsibility, interface, and adapter claims assigned to
it; both protected parents grew only by routing plus one required overlay
predicate; no MISSING rows.

**Mismatches and dispositions** (all record-level; no code change):

| Item | Verdict | Disposition |
|---|---|---|
| `app.rs:1585` — `&& !self.ui_state.has_powers_panel()` in the mouse-scroll overlay predicate | MISMATCH against the protected-parent allowed-change *enumeration* | the-design-record-is-wrong: the code is the required form (a new modal owner must join the predicate every other overlay occupies); the enumeration was incomplete. `design.md` corrected |
| `commands/mod.rs` (`ShowPowers`, `show_powers`, registration) | UNCOVERED (in code, only prose in the design) | record-is-incomplete: ledger rows added |
| `convert/kas.rs:18` (`pub(crate) mod powers;`), `types/mod.rs`, `widgets/mod.rs` | UNCOVERED | record-is-incomplete: ledger rows added |
| `theme.rs:1827` — the new widget joins the pre-existing theme-source census | UNCOVERED (never cited) | record-is-incomplete: ledger row added; removing it would redden an existing fence |
| `power.rs:36` — public `PowerInfo::new` absent from the interface cell | record gap | record-is-incomplete: row 2's own "fields private + invariants at construction" rule requires it; cell updated |
| `examples/test_bridge.rs:647-661` — new arm in the example printer | UNCOVERED (non-production) | acceptable: exhaustive-match obligation of the new variant |

Three trade-offs the reviewer adjudicated explicitly, all ruled **not**
mismatches with the governing ledger row named: the App's page-step literal `5`
(row 11 owns the key map; the same literal pattern exists for hooks and usage,
and the widget's `MAX_VISIBLE_POWERS` is private and caps a popup height rather
than the painted window), the twice-per-push catalog clone (row 5 + row 8;
explicitly accepted by Alternative 1 and already the hooks shape), and the
widget tests constructing `PowersPanelState` literals (row 9 sanctions
`render`-via-`TestBackend` with "a `PowersPanelState` the state layer would
produce"; ordering stays fenced at the state layer).

**Residual, non-blocking risks the reviewer named** (recorded, not fixed):

* The widget fixture restates the display order in a comment instead of deriving
  it, so a change to `show_powers_panel`'s sort key would not fail the widget
  test — the order is fenced at the state layer and by the App key-map test.
* `to_ascii_lowercase` in the sort key does not fold non-ASCII case (observable
  only for titles whose lowercase form crosses U+007A; the capture has none, and
  the design's `sort -f` oracle does not exercise it).
* If the App's page step and the widget's window height ever disagree, the
  effect is a longer page jump on a short terminal; the scroll clamp keeps the
  offset valid.

## PR122 review fixes (2026-09-10, tiers 1–4)

Scope and per-finding decisions: `review-decisions.md`. The slices above are
unchanged in behavior; this pass hardened them against the adjudicated review.

**Landed, by tier**

1. *Push-side, no spec change* — #9 (both defaulted fields are `Option`), #11
   (unrecognized `powers/*` warns, other families stay silent), #12
   (`discovery::nonempty` for the identifier; blank optionals normalized in
   `PowerInfo::new`), #4 (census control anchored to the converter, fed
   comment-stripped and test-stripped text), #2 (`names.push("powers")` above the
   `/help` snapshot + the registry-wide fence).
2. *Widget/state arithmetic* — #5 (`BORDER_ROWS = 2`), #7 (steering token
   budgeted out of the line), #8 (window-aware clamp in state; render-side clamp
   for a squeezed popup), #6 (title states the window), #19 (changed-check in
   both refresh methods, no clone, dead store deleted).
3. *Fence repairs* — #14a (ids asserted, fixture case fixed), #14c (CRLF fed to
   the function under test — **landed in round 3, not in this pass**: the row
   claimed it while the assertion was still `f(lf) == f(lf)`; see
   `review-decisions.md` → "Round 3"), #18 (order-agnostic transport
   collection), #13 +
   #15 + #16 (shared predicate; both doc files; ASCII-lowercase prose).
4. *Root cause* — #20 + #3: `Overlay` + `Overlay::ALL` in `traits.rs`,
   `UiState::topmost_overlay`/`has_modal_overlay`, `render` paints the constant
   (approval last), `handle_key` matches on it, and the three guards that must
   agree now share the predicate.

**Not landed (tier 5, needs re-approval)** — the pull path, the catalog
lifecycle on `/new`, `/powers` registration gating, and the overlay displacement
policy. Rationale per item in `review-decisions.md`.

**Oracles**

- `python3 .cyril-v19o/oracles/module_shape.py` → PASS (C8 ledger + the two
  censuses). The protected-parent allowlist was widened once for this pass, with
  the correction recorded in `design.md` → "PR122 review fixes".
- `bash .cyril-v19o/oracles/mutations.sh` → every named mutation red, every
  fence green after restore (transcript in the commit that added them).

**Gates** (committed tree) — `cargo fmt --all -- --check` clean;
`cargo clippy --workspace --all-targets --features kas` and
`cargo clippy --all-targets -- -D warnings` both silent; `cargo test --workspace
--features kas` 1993 passed / 0 failed; `cargo test --workspace` 1991 passed /
0 failed. Oracle run: 27 named mutations, every fence red under its mutation and
green after restore.

**Not verified** — no live KAS run this pass; the 100×24 / `input_top = 18`
geometry is exercised in `TestBackend` only.

## Advisory follow-up — squeezed-window reachability (2026-09-10)

The advisory on finding 8 was correct and is fixed. The first pass bounded the
state's scroll at `len - MAX_VISIBLE_POWERS` — the WIDEST window — while the
widget paints at most the window the PLACED popup fits. When `modal::place`
squeezes the popup (a short terminal, or a tall input), the state's bound stops
the keyboard short of the last full window; the widget then clamps the viewport
into that window, so the tail of the catalog is unreachable rather than merely
unscrolled. Nine powers in an 18-row frame show three at a time: the keyboard
stopped at index 4, the viewport clamp starts at 6, and indices 7–8 could never
be painted. Not exotic — the standard 24-row frame squeezes the popup to four
powers once the input is tall enough for 10+ powers to overflow.

Fix — the bound is the window the popup actually gets:

- `render::frame_rows` — the vertical chrome budget extracted from `draw_inner`
  (one formula, two readers: the constraint list and the geometry queries).
- `render::input_top` / `render::powers_window` — the input row and the window
  that follows from it, for the frame the state reports a size for.
- `widgets::powers_panel::placement` — the popup rect and its window in one
  place; `render` and the state's bound both read it.
- `UiState::max_powers_scroll` — `len - powers_window(len)`, read before the
  panel is borrowed mutably; `refresh_powers_panel` re-clamps through it.

Fence: `squeezed_viewport_reaches_the_last_power` — drives the real `UiState`
scroll to its extreme at 100×18, then draws the WHOLE frame (the input row comes
from the real layout, not a hand-passed constant) and asserts the last power is
painted inside a whole three-power window, with the title saying `showing 7–9`.

Mutations: `scroll-clamp-assumes-the-max-window` (replaces
`scroll-clamp-uses-the-last-index`, which no longer names a defect) and
`powers-window-ignores-the-placed-popup`; both red the new fence. `render.rs`
joined the restore set, so a killed run cannot leave it mutated.

`.cyril-v19o/oracles/anchor_check.py` — new pre-flight: parses every `prove`
invocation and requires each anchor to occur exactly once. A stale anchor
aborted the first post-fix oracle run after ~9 minutes; this answers in 0.2s.

**Gates** (this change) — `cargo fmt --all -- --check` clean;
`cargo clippy --workspace --all-targets --features kas` and
`cargo clippy --all-targets -- -D warnings` silent;
`cargo test --workspace --features kas` 1994 passed / 0 failed;
`cargo test --workspace` 1992 passed / 0 failed;
`cargo test --doc --workspace --all-features` green; `cargo doc -p cyril-ui
--no-deps` warning-free; `module_shape.py` PASS; `mutations.sh` 28/28 red then
green.

## Second advisory — the offset outlives its window (2026-09-10)

Correct again. `set_terminal_size` only records the size, and
`powers_panel_scroll_up` subtracted from whatever offset was stored. A terminal
that GROWS (or an input/crew/voice row that goes away) enlarges the popup and
puts the stored offset past the new last full window, where the widget renders
the view from the window's end: from offset 6 at 100×18, growing to 100×24 (five
powers, bound 4) and pressing Up left the viewport at 4 — two keypresses bought
nothing before the view moved.

Fix: `powers_panel_scroll_up` normalizes the offset into the CURRENT window
(`min(max_powers_scroll(len))`) before subtracting, read before the panel is
borrowed mutably. Down needed nothing — it saturates to the bound and the view
is already at the bottom, so no press can be swallowed there.

Fence: `scroll_up_moves_after_the_window_grows` — 100×18, scroll to the extreme,
grow to 100×24, one Up, draw the whole frame and require `showing 4–8` with
`Power 03` on the first content row. Mutation
`scroll-up-ignores-the-current-window` reds it.

**Gates** (final tree) — fmt clean; both clippy configurations silent;
`cargo test --workspace --features kas` 1995 passed / 0 failed;
`cargo test --workspace` 1993 passed / 0 failed;
`cargo test --doc --workspace --all-features` green; `module_shape.py` PASS;
`mutations.sh` 29/29 red then green; `anchor_check.py` 29/29 anchors unique.
