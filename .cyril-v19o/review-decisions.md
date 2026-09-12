# PR122 review decisions (gilfoyle `assessing-review-feedback`)

Input: `review/PR122-code-review.md` (the review, 469 lines) adjudicated by
`review/PR122-code-review-verification.md` (the verification, 135 lines,
executed against head `b82576ce`). Both are committed here verbatim as the
adjudication of record — `sha256 6414ed52…` and `e02264d5…`, identical to the
copies the reviewer produced. Every finding gets one evidence state and one decision here; the
verification's own table is the evidence source, and the "Verified fix order"
tiers 1–4 are the work scope. Tier 5 (pull path, catalog lifecycle, gating) is
untouched — each of those is a spec decision, not a fix.

Evidence states are the verification's, not re-derived: `Verified` means the
parent reproduced the claim (executed probe or read source), `Refuted as stated`
means the reviewer's mechanism is wrong, `PLAUSIBLE` means reproduced but
trigger realism unproven.

## Decisions

| # | Finding | Reviewer | Evidence state | Decision | Fix landed | Note |
|---|---|---|---|---|---|---|
| 1 | Fence forbids `_kiro/powers/list`, which works | CONFIRMED | Verified | **Modify** — keep the prohibition, correct the rationale | `crates/cyril/tests/powers_source_fence.rs` header (`:10-16`), violator message, `list` unit case; `convert/kas/powers.rs` module doc | The prohibition implements ratified behavior (`spec.md:64` B8, `:77` SC5, `:86`, `:111`). `list` dispatches but is never advertised → not feature-detectable; `refresh` answers `-32603`. Inverting it is a spec change, out of scope. |
| 2 | `/help` never lists `/powers` | CONFIRMED | Verified | Accept | `commands/mod.rs` — `names.push("powers")` moved above `HelpCommand::new(&names)`; new fence `help_lists_every_registered_command` | The snapshot is eager; the old post-snapshot push was a dead store. |
| 3 | Key priority inverted vs render z-order | CONFIRMED | Verified (worse than the example: 15/15 pairs, approval the worst case) | Accept — direction-aware displacement policy deferred, z-order + precedence consistency fixed | `traits.rs::Overlay` + `Overlay::ALL`, `state.rs::topmost_overlay`/`has_modal_overlay`, `render.rs` paints `Overlay::ALL`, `app.rs::handle_key` matches on `topmost_overlay()` | Paint order now equals key order reversed, so the overlay that reacts is the one on top. See #20 for what was deferred. |
| 4 | Non-vacuity guard satisfied by a comment | CONFIRMED | Verified (understated: test-code hits also satisfied it) | Accept | `powers_source_fence.rs` — control anchored to `CONVERTER` and fed `production_prefix(strip_line_comments(src))` | The old guard was satisfied by a doc comment in `types/power.rs` and by test doubles inside `crates/*/src`. |
| 5 | `CHROME_ROWS = 4` drops a power | CONFIRMED | Verified (numbers exact) | Accept | `widgets/powers_panel.rs` — `BORDER_ROWS = 2`; fence `clamped_popup_still_shows_every_power_that_fits` | Both uses of the constant replaced; the clamp-reachable geometry (100×24, `input_top = 18`) is fenced. |
| 6 | Overflow silently truncated | CONFIRMED | Verified ("unlike every sibling" wrong — hooks has no affordance either) | Accept (reword) | `widgets/powers_panel.rs` title states `showing 1–N` when the catalog exceeds the window; fence `title_states_the_window_when_the_catalog_overflows` | Zero rows spent; the panel has no `+N more` row and no key footer. |
| 7 | `· steering` truncated away | CONFIRMED | Verified (101 cells, not ~103) | Accept | `widgets/powers_panel.rs` budgets the token before truncating the meta line; fence `steering_marker_survives_a_truncated_meta_line` | Reordering the tokens would break spec B1 and a named-mutation anchor, so the width is reserved instead. |
| 8 | Scroll clamps to last index | CONFIRMED | Verified (derivation exact) | Accept | `state.rs::max_powers_scroll` (window clamp) used by `powers_panel_scroll_down` and `refresh_powers_panel`; widget clamps to the placed window; `MAX_VISIBLE_POWERS` moved to `traits.rs` as the one number both readers share | Two layers: state clamps to the window the popup ACTUALLY gets (`render::powers_window`, derived from the frame the state reports), the widget clamps again for a hand-built state. **Advisory follow-up (2026-09-10):** the first pass bounded the state at `len - MAX_VISIBLE_POWERS`, which assumed the widest window — a squeezed popup then made the catalog's tail unreachable (9 powers in an 18-row frame show 3, so the keyboard stopped at index 4 while the viewport clamp starts at 6). The bound now reads the placed window; fences `squeezed_viewport_reaches_the_last_power` (end-to-end: real `UiState` scroll → real frame) and the widget's `viewport_window_clamps_scroll`. **Second advisory:** the offset could also outlive its window — `set_terminal_size` only records the size, so a terminal that grew left the offset past the new bound and Up spent keypresses walking it back into range with the viewport still (1 Up from offset 6 at 100×18 → 100×24 subtracted to 5, still rendered from 4). Up now normalizes into the current window first; fence `scroll_up_moves_after_the_window_grows`, mutation `scroll-up-ignores-the-current-window`. |
| 9 | Explicit JSON `null` discards the catalog | PLAUSIBLE | Verified (executed mirror probe) | Accept | `convert/kas/powers.rs` — `mcp_server_names: Option<Vec<String>>`, `has_steering_files: Option<bool>`, both `#[serde(default)]`; fence `explicit_null_on_a_defaulted_item_field_keeps_the_catalog` | Push-side fix only; no pull path (tier 5). |
| 10 | `powers` never cleared | PLAUSIBLE | **Refuted as stated** | **Reject** | None beyond the doc reword already in `session.rs:47-51` | `spec.md:118` pins process-global; clearing on `SessionCreated` is the one unsafe variant (the transport harness scripts push-after-created). |
| 11 | Re-spelled push vanishes silently | CONFIRMED | Verified | Accept | `convert/kas/powers.rs::to_notification` warns for unrecognized `powers` methods; fence `unrecognized_powers_methods_warn_but_other_families_stay_silent` | Mirrors the workflow sibling's `starts_with` warn; other families stay silent. |
| 12 | Blank name → blank row, sorted first | CONFIRMED | Verified | Accept | `convert/kas/powers.rs` — `identified_name` via `discovery::nonempty` (blank ⇒ frame dropped); `types/power.rs` normalizes blank `display_name`/`description`/server entries; fence `whitespace_only_optionals_are_absent_and_blank_servers_are_dropped` + malformed-list rows | A blank identifier is the same fact as a missing one: the item cannot be rendered, matched, or sorted. |
| 13 | Paste guard omits the powers panel | CONFIRMED | Verified (wider: every overlay except usage) | Accept (reword) | `app.rs` — mouse guard, paste guard and voice-transcript insert all call `has_modal_overlay()`; fence `paste_mouse_and_voice_respect_every_overlay` | One predicate, three guards; the old guards were hand-written lists that went stale per overlay. |
| 14a | Ordering fence cannot fail (tie-break) | CONFIRMED | Verified | Accept | `state.rs` — `powers_panel_orders_and_replaces` now asserts `(title, name)` pairs with the duplicate ids arriving in the wrong order | The id tie-break is load-bearing, not incidental. |
| 14b | App fence cannot fail (neuter the arm) | CONFIRMED | Verified (deletion is E0004; a neutered arm is caught elsewhere) | Accept | `app.rs` — the fence drives `/powers` through `submit_input`, so `handle_command_result`'s `ShowPowers` arm is load-bearing | The verification's correction is recorded: E0004, not a green test. |
| 14c | CRLF fence is a tautology | CONFIRMED | Verified | Accept | **NOT landed in this pass**; landed in round 3 (`powers_source_fence.rs` — `powers_census_is_line_ending_agnostic` now feeds the RAW CRLF string to the function under test, and pins `read_normalized`) | This row asserted a fix that was not in the tree: the assertion stayed `f(lf) == f(lf)` byte for byte (see "Round 3"). |
| 15 | `CLAUDE.md` overlay chain stale | CONFIRMED | Verified (also stale for `usage`; duplicated in `AGENTS.md`) | Accept (extend) | `AGENTS.md` + `CLAUDE.md` — chain replaced by the `Overlay` stack, paint/key order stated, the "mouse-scroll guard" rule replaced by the shared-predicate rule | Both files are plain files, not symlinks; both updated. |
| 16 | Sort-key docs overstate "case-insensitive" | CONFIRMED | Verified (prose only; code matches `spec.md:34`) | Accept | `state.rs` (`show_powers_panel`, `sorted_powers`), `traits.rs` (`PowersPanelState.powers`), two test comments → "ASCII-lowercase" | The approved `spec.md` wording is left alone. |
| 17 | `/powers` inert in a default build | CONFIRMED | Verified | **Modify** — reword the message, do not gate | `commands/builtin.rs` message names the real precondition (`--features kas` build, `--agent-engine kas` session); `plan.md:97` pin updated | Gating contradicts `spec.md:110` and still leaves the wrong sentence in the surviving states. The "No powers reported yet" prefix stays (two fences assert it). |
| 18 | Transport fence order-coupled | CONFIRMED | Verified | Accept | `bridge/tests/current_runtime_contract/powers.rs` — the loop collects the push and `SessionCreated` in either order | The 18 ms gap is wall-clock, not a wire guarantee. |
| 19 | `refresh_powers_panel` returns `true` for a no-op | CONFIRMED | Verified (same defect in `refresh_hooks_panel`; dead store at `app.rs:2041`) | Accept | `state.rs` both refresh methods take `&[T]`, compare sorted rows, and report `false` when nothing changed; the App passes by reference and the dead `redraw_needed` store is gone | Fences: `powers_panel_orders_and_replaces` (identical push), `refresh_replaces_contents_and_clamps_scroll` (hooks mirror). |
| 20 | Fifth hand-copied overlay | — | Verified root cause (magnitude imprecise) | Accept (direction) | `Overlay` enum + `Overlay::ALL` + `topmost_overlay()`; render and key dispatch both derive from the constant | Deferred: the displacement policy (product decision) and any `Overlay`-driven opening/refusal semantics. |
| R1 | Dropped `sessionId` is harmless | retracted | Retraction confirmed (sharper reason) | Accept | `convert/kas/powers.rs::WirePowersChanged` doc: catalog is process-global, extension frames route globally (`domain_mediator/inbound.rs`) | The reviewer's stated reason was wrong; the corrected reason is documented. |
| R2 | Turn-liveness stamp costs the cancel affordance | retracted | Retraction confirmed | Accept | `state.rs` stall field doc rewritten (six-variant allowlist, up to 30 s lag) | The doc was what made the retraction sound plausible on a read. |

## Deferred (tier 5, requires re-approval)

1. **Pull-based recovery via `_kiro/powers/list`** — works, unadvertised, not
   feature-detectable. Would require `spec.md:64/77/86/111` re-approval plus the
   transport fence's offside filter, `builtin.rs`, and the converter's method
   set.
2. **Catalog lifecycle on `/new` / disconnect** — `spec.md:118` pins
   process-global; any change is a spec decision.
3. **`/powers` registration on non-KAS builds** — `spec.md:110` pins
   unconditional registration.
4. **Overlay displacement policy** — whether a push-driven open displaces an
   open overlay or refuses with a visible line. The codebase's stated doctrine
   (push-driven opens never pop a modal over the user) conflicts with
   last-opened-wins; the policy has to be chosen, then encoded once. This change
   fixes only the z-order/key-order consistency and the shared predicate.

Item 4's tracker: to be filed as a rivets issue when this branch lands (it is
the only deferred item that is a defect class rather than an absent feature).

## Artifact corrections made while executing this log

- `.cyril-v19o/oracles/module_shape.py` — the protected-parent allowlists were
  widened for tier-4 work: `UiState` may own the overlay query methods
  (`topmost_overlay`, `has_modal_overlay`, `is_overlay_open`) and the hooks
  panel's lifecycle methods (finding 19's mirror); `App` may consult the
  predicate in `handle_key`, `handle_terminal_event` (mouse + paste) and
  `handle_voice_event`, plus the `Overlay` import and the `overlay` marker.
- `.cyril-v19o/oracles/mutations.sh` — anchors updated for the rewritten
  methods, and one named mutation added per review-driven fence.
- `plan.md:97` — the pinned message text follows finding 17.

## Verification limits carried forward

- No live KAS run was performed for this fix set; the push-side changes are
  covered by the committed 2.21.2 fixture and the transport harness.
- The 100×24 `input_top = 18` draw is exercised in `TestBackend`, not in a real
  terminal.
- The scroll bound is derived from the frame the state reports for its own
  terminal size. A layout change between a keypress and the next frame (the
  input growing while the panel is open) can leave the bound one row stale for
  that one press; the widget's own clamp absorbs it, so the frame stays whole
  and no power becomes unreachable — the frame after the press re-derives the
  bound.

## Round 3 — review of head `92cf3cde` (2026-09-12)

Input: an independent review of the repair round (`agent://PR122Review`, one
isolated read-only reviewer over `ab65e7a3..92cf3cde`, plus the parent's own
re-derivation of every decisive claim, CI inspection and a re-run of the
oracles). Eight commits landed between the head the verification above judged
(`b82576ce`) and the head reviewed here, so none of the tiers 1–4 fixes were
covered by it; the round-3 findings are the new code's, not the earlier ones'.

| # | Finding (round 3) | Evidence | Decision | What landed |
|---|---|---|---|---|
| 1 | Windows CI red: the census compared a `display()` path (backslashes) to a `/`-spelled literal, so the control failed on a walk that had found the converter | CONFIRMED — CI log (`Test (windows-latest)`, `panicked at powers_source_fence.rs:195`); `CI Success` failed, and nextest's cancel-on-first-failure left 1828 of 1881 Windows tests unrun | Accept | `powers_source_fence.rs` — the control is a `Path` (`converter_path()`), compared as components; the reported paths stay `PathBuf`s |
| 2 | `/powers`' scroll bound came from `UiState::terminal_size`, which production never initializes (only the `Event::Resize` arm writes it; crossterm emits `Resize` only on SIGWINCH) — a startup terminal of ≤ 21 rows kept the 80×24 default and the catalog's tail became unreachable, the class of bug the window clamp had closed for the case where the size is right | CONFIRMED — read + arithmetic, and independently derived by the parent before the reviewer reported it | Accept | `app.rs` — `App::draw_frame` reads `terminal.size()` and syncs the state on every draw (both draw sites in `run` go through it); fence `drawing_syncs_the_frame_geometry_the_scroll_bound_reads` drives a `TestBackend` 100×18 through the real draw path and requires the offset the squeezed window allows |
| 3 | The CRLF fence was still `f(lf) == f(lf)` and was byte-identical to the pre-repair revision, while row 14c above recorded it as fixed; the census's own read path was unguarded | CONFIRMED — `git diff b82576ce 92cf3cde` has no hunk there; executed probe on the shipped function bodies | Accept | `powers_source_fence.rs` — the fence feeds the RAW CRLF string, adds non-vacuity and mixed-ending cases, and pins `read_normalized` on a real CRLF file; row 14c corrected (above). **Retracted by the parent after re-execution:** the claim that dropping `read_normalized`'s replace would change the verdict — `strip_line_comments` normalizes through `str::lines().join("\n")`, so it would not; the fence now asserts the behaviour rather than the mechanism |
| 4 | The module-shape gate's protected-parent net was inert for `app.rs`: `production_text` cut at the first `#[cfg(test)]` token, which in `app.rs` is a struct field at line 133, so 406 of 407 added lines were skipped — and `design.md`'s C8 falsifier ("add an `App` field → fails") was therefore false while the gate printed PASS | CONFIRMED — ran the oracle's own functions (`added=407, considered=1, skipped=406`) | Accept | `module_shape.py` — `production_text` cuts at the `#[cfg(test)] mod` declaration and `function_ranges` sees free functions (they were attributed to the preceding impl method, which made `dispatch_powers_panel_key` unallowable). With the net live, the App allowances were named explicitly (draw_frame, the two draw sites' call, the powers/hooks wiring calls, the redraw store, the two imports) and recorded in `design.md` → C8; two named mutations now require the gate to go red |
| 5 | The overlay guards discard input on a premise that is false — every overlay is placed strictly above `input_top`, so the input box stays visible; the paste arm dropped text with no log at all, and a dropped dictation is unrecoverable | CONFIRMED — `modal::place`'s contract; `app.rs` guard sites | Accept | `app.rs` — the paste drop logs at `warn!` (voice's level), the voice drop additionally says so in chat, and both comments state the real reason (a policy that keeps input from parking under an overlay) instead of the layout claim. **The user-visible half is fenced** (it landed unfenced at first — the same record-claims-what-the-tree-does-not-do pattern finding 3 caught): `paste_mouse_and_voice_respect_every_overlay` counts the discard notice on the drop path *and* on the path where the transcript lands |
| 6 | `STEERING_TOKEN` budgeted by `len()` (bytes) while `truncate` measures cells: U+00B7 is 2 bytes, 1 cell, so the token was dropped a column early and its "ASCII" doc was false | CONFIRMED — measured (11 cells / 12 bytes) against `text.rs`'s display-width `truncate` | Accept | `widgets/powers_panel.rs` — `steering_width()` via `UnicodeWidthStr`, used by both the guard and the budget; doc corrected |
| 7 | `plan.md`'s slice-3 pin said page keys scroll ten; the key map scrolls five | CONFIRMED | Accept | `plan.md:97` corrected; `dispatch_powers_panel_key`'s doc states the step against `MAX_VISIBLE_POWERS` and the state's clamp |

**Named mutations added** (all proved red then green): `draw-path-does-not-sync-the-frame-geometry`,
`crlf-read-path-stops-normalizing`, `module-shape-net-blind-to-a-new-App-field`,
`module-shape-net-blind-outside-the-allowed-regions`,
`voice-discard-not-announced`, `voice-notice-emitted-on-both-paths`. The oracle
is now invoked from `mutations.sh` at all — previously the gate's `PASS C8` was
never falsified by the mutation set.

**Harness defect found by running the full set:** `restore_all` restored only
the ten production files it knew about, so the first `crlf-read-path-stops-
normalizing` run (whose target is the fence file itself) left the mutation in
the working tree — the proof reported "fence still RED after restore" and the
byte-exact revert had to be made by hand. The fence file is now in the restore
set. A mutation whose target is outside that set is a mutation that outlives
its run; the guard is the harness's, not the mutation's.

**Still open / deferred:** the overlay displacement policy (tier 5, item 4
above) — a user-initiated open can land under an existing opaque popup and be
revealed only after Esc; unchanged by this round.

