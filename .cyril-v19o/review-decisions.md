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
| 8 | Scroll clamps to last index | CONFIRMED | Verified (derivation exact) | Accept | `state.rs::max_powers_scroll` (window clamp) used by `powers_panel_scroll_down` and `refresh_powers_panel`; widget clamps to the placed window; `MAX_VISIBLE_POWERS` moved to `traits.rs` as the one number both readers share | Two layers: state clamps to the maximum window, the widget clamps again for a squeezed popup. |
| 9 | Explicit JSON `null` discards the catalog | PLAUSIBLE | Verified (executed mirror probe) | Accept | `convert/kas/powers.rs` — `mcp_server_names: Option<Vec<String>>`, `has_steering_files: Option<bool>`, both `#[serde(default)]`; fence `explicit_null_on_a_defaulted_item_field_keeps_the_catalog` | Push-side fix only; no pull path (tier 5). |
| 10 | `powers` never cleared | PLAUSIBLE | **Refuted as stated** | **Reject** | None beyond the doc reword already in `session.rs:47-51` | `spec.md:118` pins process-global; clearing on `SessionCreated` is the one unsafe variant (the transport harness scripts push-after-created). |
| 11 | Re-spelled push vanishes silently | CONFIRMED | Verified | Accept | `convert/kas/powers.rs::to_notification` warns for unrecognized `powers` methods; fence `unrecognized_powers_methods_warn_but_other_families_stay_silent` | Mirrors the workflow sibling's `starts_with` warn; other families stay silent. |
| 12 | Blank name → blank row, sorted first | CONFIRMED | Verified | Accept | `convert/kas/powers.rs` — `identified_name` via `discovery::nonempty` (blank ⇒ frame dropped); `types/power.rs` normalizes blank `display_name`/`description`/server entries; fence `whitespace_only_optionals_are_absent_and_blank_servers_are_dropped` + malformed-list rows | A blank identifier is the same fact as a missing one: the item cannot be rendered, matched, or sorted. |
| 13 | Paste guard omits the powers panel | CONFIRMED | Verified (wider: every overlay except usage) | Accept (reword) | `app.rs` — mouse guard, paste guard and voice-transcript insert all call `has_modal_overlay()`; fence `paste_mouse_and_voice_respect_every_overlay` | One predicate, three guards; the old guards were hand-written lists that went stale per overlay. |
| 14a | Ordering fence cannot fail (tie-break) | CONFIRMED | Verified | Accept | `state.rs` — `powers_panel_orders_and_replaces` now asserts `(title, name)` pairs with the duplicate ids arriving in the wrong order | The id tie-break is load-bearing, not incidental. |
| 14b | App fence cannot fail (neuter the arm) | CONFIRMED | Verified (deletion is E0004; a neutered arm is caught elsewhere) | Accept | `app.rs` — the fence drives `/powers` through `submit_input`, so `handle_command_result`'s `ShowPowers` arm is load-bearing | The verification's correction is recorded: E0004, not a green test. |
| 14c | CRLF fence is a tautology | CONFIRMED | Verified | Accept | `powers_source_fence.rs` — `powers_census_is_line_ending_agnostic` feeds the RAW CRLF string to the function under test | No more `f(lf) == f(lf)`. |
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
