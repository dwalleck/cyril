# PR122 code review — `/powers` panel

- **Effort:** max (10 finder angles, 3 verifier batches, 1 gap sweep)
- **Date:** 2026-09-10
- **Verdicts:** `CONFIRMED` = verified against source/evidence in-session. `PLAUSIBLE` = the code does not handle the case, but the trigger was not observed live.

Findings 1–15 were reported through the review UI; items 16–20 fell below the 15-item
reporting cap and are recorded here in rank order. Two first-pass claims were retracted
during verification and are listed at the end so they are not re-derived later.

---

## Summary

| # | Location | Verdict | Issue |
|---|---|---|---|
| 1 | `crates/cyril/tests/powers_source_fence.rs:146` | CONFIRMED | Fence forbids `_kiro/powers/list`, which the PR's own oracle proves works |
| 2 | `crates/cyril-core/src/commands/mod.rs:396` | CONFIRMED | `names.push("powers")` runs after the snapshot; `/help` never lists `/powers` |
| 3 | `crates/cyril/src/app.rs:1663` | CONFIRMED | Key priority inverted vs render z-order; Enter can fire an invisible picker |
| 4 | `crates/cyril/tests/powers_source_fence.rs:156` | CONFIRMED | Non-vacuity guard scans raw source; a doc comment satisfies it |
| 5 | `crates/cyril-ui/src/widgets/powers_panel.rs:38` | CONFIRMED | `CHROME_ROWS = 4` copied from a panel with a header this one lacks |
| 6 | `crates/cyril-ui/src/widgets/powers_panel.rs:103` | CONFIRMED | Overflow silently truncated — no `+N more`, no key hint |
| 7 | `crates/cyril-ui/src/widgets/powers_panel.rs:117` | CONFIRMED | `· steering` truncated away after a long MCP server list |
| 8 | `crates/cyril-ui/src/state.rs:2476` | CONFIRMED | Scroll clamps to last index, not last valid window start |
| 9 | `crates/cyril-core/src/protocol/convert/kas/powers.rs:68` | PLAUSIBLE | Explicit JSON `null` on one power discards the entire catalog |
| 10 | `crates/cyril-core/src/session.rs:52` | PLAUSIBLE | `powers` never cleared on new session or disconnect |
| 11 | `crates/cyril-core/src/protocol/convert/kas/powers.rs:108` | CONFIRMED | Re-spelled push vanishes below default log level; no near-miss warning |
| 12 | `crates/cyril-core/src/types/power.rs:45` | CONFIRMED | Whitespace-only or empty name yields a blank row sorted to the top |
| 13 | `crates/cyril/src/app.rs:1609` | CONFIRMED | `Event::Paste` guard omits the powers panel (pre-existing shape) |
| 14 | `crates/cyril-ui/src/state.rs:5633` | CONFIRMED | Two new regression fences cannot fail on the property they name |
| 15 | `CLAUDE.md:295` | CONFIRMED | Documented overlay chain not updated for the new panel |
| 16 | `crates/cyril-ui/src/state.rs:2418`, `traits.rs:528` | CONFIRMED | Sort-key doc comments overstate "case-insensitive"; code matches spec |
| 17 | `crates/cyril-core/src/commands/mod.rs` (registration) | CONFIRMED (latent) | `/powers` registered unconditionally; inert in a default `cargo build` |
| 18 | transport fence | CONFIRMED | Two-outcome ordering coupling |
| 19 | `crates/cyril/src/app.rs:2041` | CONFIRMED | `refresh_powers_panel` returns `true` for a no-op push; wasted clone, dead `redraw_needed` |
| 20 | architecture | — | Fifth hand-copied overlay across ~17 uncompiler-checked edit sites |

---

## 1. The source fence forbids a method that works

**File:** `crates/cyril/tests/powers_source_fence.rs:146` · **CONFIRMED** · test-correctness

The fence permanently forbids `_kiro/powers/list`, which the PR's own Oracle B proves
WORKS — freezing "unadvertised" into "cannot work" and blocking the only recovery path
for two other defects.

**Failure scenario.** The assertion message reads "`list` is unadvertised, `refresh`
answers -32603 ... an affordance on either is a control that cannot work."
`.cyril-v19o/evidence.md` contradicts that four times:

- line 37 — "`list` returns the same 3"
- line 44 — "its success is the control that makes the `refresh` error specific"
- line 53 — "`list` returns the same 3 items with identical key sets and `errors: []`"
- line 63 — records `probe-kas-powers-2.21.2.py` actually calling it 1 ms after the
  `session/new` reply

`list` is live-proven functional on 2.21.2 and merely absent from `extensionMethods`.
A working pull is exactly what fixes the stale catalog across `/new` (finding 10) and the
unrecoverable "No powers reported yet" after one bad frame (finding 9) — and both are now
unfixable without first deleting a 268-line test that asserts a falsehood.

---

## 2. `/powers` is missing from `/help`

**File:** `crates/cyril-core/src/commands/mod.rs:396` · **CONFIRMED** · correctness

`names.push("powers")` runs 14 lines after `HelpCommand::new(&names)` snapshots the list,
so the push is dead and `/help` never lists `/powers`.

**Failure scenario.** `builtin.rs:12-16` copies eagerly into an owned `Vec<String>`;
`mod.rs:382` registers it; the push at `:396` mutates a local never read again. Both
siblings it imitates (`hooks` at `:375`, `workflow` at `:379`) push *before* `:382`.
Type `/help` on any engine: 14 commands listed, `powers` absent. Autocomplete still works
(`app.rs:410-422` seeds from `all_commands()`), so only the explicit discovery surface is
blind. Unfenced — the sole help test is `HelpCommand::new(&[])`.

---

## 3. Key priority is inverted against render z-order

**File:** `crates/cyril/src/app.rs:1663` · **CONFIRMED** · correctness

Key priority (first match) is inverted against render z-order (last painted), and no
overlay excludes another — so Enter can execute a model change from a picker the user
cannot see.

**Failure scenario.** Submit `/model` (async `QueryCommandOptions`), then `/powers`
(fully local, opens instantly at `app.rs:2037`); `CommandOptionsReceived` then calls
`show_picker` (`app.rs:1435`) with no open-overlay guard. Powers is 96 cols vs the
picker's 80 and both `Clear`, so powers completely overpaints it. `handle_key` tests
`has_picker()` first: Up/Down move the invisible selection and Enter sends
`BridgeCommand::ExecuteCommand{command:"model", args:{"value": <unseen>}}`.

Same collision with `/hooks` (async `_kiro/hooks/list` → `show_hooks_panel`, no guard)
where arrows scroll an invisible panel and the visible one needs two Escs.

Pre-existing inversion; this PR adds five more inverted pairs.

---

## 4. The census's non-vacuity guard is satisfied by a comment

**File:** `crates/cyril/tests/powers_source_fence.rs:156` · **CONFIRMED** · test-correctness

The census's own non-vacuity guard scans the RAW source while the violator scan strips
comments, so prose alone satisfies it — delete the entire converter and the fence still
reports green *and* non-vacuous.

**Failure scenario.** `push_seen_in` is filled by
`src.contains(&format!("kiro/powers/{PUSH_METHOD}"))` on un-stripped text, whereas
`unimplemented_powers_methods` runs `strip_line_comments` first.
`crates/cyril-core/src/types/power.rs:2` names `_kiro/powers/items_changed` only in a doc
comment. Remove `convert/kas/powers.rs` (const `METHOD` and all) and
`assert!(!push_seen_in.is_empty(), "...its absence means this census is scanning the wrong tree")`
still passes — the exact failure it exists to detect.

---

## 5. `CHROME_ROWS = 4` drops a power that fits

**File:** `crates/cyril-ui/src/widgets/powers_panel.rs:38` · **CONFIRMED** · correctness

`CHROME_ROWS = 4` was copied from the hooks panel, whose fourth row is a header this
widget lacks; the block costs 2, so a power that fits is dropped whenever `place` clamps.

**Failure scenario.** Verified in a harness replicating `render.rs` on the 100x24 terminal
the constant's own comment cites: with an empty draft `input_area.y = 18`, `place` clamps
the popup to height 17, inner height 15 — which holds 5 powers — but
`visible_powers = (17-4)/3 = 4`, so 4 of 5 are painted with rows 14-16 blank.

Unclamped (terminals >= 28 rows) 15 of 17 inner rows are used: 2 permanently blank.

The widget tests cannot catch it because `draw()` (`:161-176`) passes
`frame.area().height` as `input_top`, never triggering the clamp.

**Fix:** use `BORDER_ROWS = 2` in the window calculation.

---

## 6. Overflow is silently truncated

**File:** `crates/cyril-ui/src/widgets/powers_panel.rs:103` · **CONFIRMED** · correctness

A catalog larger than the window is silently truncated with no `+N more` row and no key
hint, unlike every sibling overlay.

**Failure scenario.** With 12 installed powers the title reads " /powers · 12 powers " but
`.take(visible_powers.max(1))` paints 5 rows and stops. `crew_panel` renders a `+N more`
overflow indicator (CLAUDE.md names it the single source of truth for that panel's
sizing); `usage_panel.rs:42` and `code_panel.rs:121` both render a key footer. Here there
is neither, so the user sees 12 claimed, 5 shown, and no indication that
Up/Down/PageDown scroll or that Esc closes — the panel reads as having lost 7 powers.

---

## 7. `· steering` is truncated away

**File:** `crates/cyril-ui/src/widgets/powers_panel.rs:117` · **CONFIRMED** · correctness

`· steering` is appended after every `· mcp <server>` token and the whole line is then
truncated, so a power with long or multiple MCP servers silently loses the steering fact.

**Failure scenario.** `meta` is built as name, then one `· mcp {server}` per server
(`:113-115`), then `· steering` (`:117`), then `truncate_and_pad(&meta, inner_width)`
(`:120`). On 80 columns `place` clamps width to 76 → `inner_width = 74`.

```json
{"name":"aws-infrastructure-as-code",
 "mcpServerNames":["awslabs.aws-iac-mcp-server","awslabs.cdk-mcp-server"],
 "hasSteeringFiles":true}
```

builds a ~103-cell line; truncation at 74 drops ` · steering` entirely and the panel
states by omission that the power ships none. The widget's own test counts `"· steering"`
occurrences only at width 100.

**Fix:** emit the steering token before the MCP list, or budget it out of the width.

---

## 8. Scroll clamps to the last index, not the last window start

**File:** `crates/cyril-ui/src/state.rs:2476` · **CONFIRMED** · correctness

`powers_panel_scroll_down` clamps to the last item's INDEX rather than the last valid
window start, so the real 3-power catalog — which fits entirely — can be scrolled into a
mostly-blank panel; a new test freezes the behaviour.

**Failure scenario.**

```rust
let max = panel.powers.len().saturating_sub(1);
panel.scroll_offset = (panel.scroll_offset + powers).min(max);
```

with a PageDown step of 5 (`app.rs:2788`). Derivation on 100x24 for 3 powers:
`desired_height = 3*3+4 = 13`, popup height 13, inner 11, `visible_powers = 3`. One
PageDown sets offset to `min(5, 2) = 2`; `.skip(2).take(3)` paints ONE power — 3 lines
under 8 blank inner rows — with the title still reading " /powers · 3 powers ".

Strictly worse than the hooks panel it copied (3 blank of 5) because rows are 3 lines
tall. `state.rs:5653-5655` asserts `scroll_offset == 2` on a 3-item list, freezing it as
intended.

**Fix:** the correct bound is `len().saturating_sub(visible_window)`.

---

## 9. One explicit `null` discards the whole catalog

**File:** `crates/cyril-core/src/protocol/convert/kas/powers.rs:68` · **PLAUSIBLE** · error-handling

`#[serde(default)]` does not cover an explicit JSON `null`, so one null field on one power
discards the ENTIRE catalog frame — with no recovery, because the push is once-per-session
and the fence (finding 1) forbids the working pull.

**Failure scenario.** serde's `default` applies only to an ABSENT key; an explicit null
reaches `Vec::deserialize` / `bool::deserialize` and errors.

```json
{"powers":[{"name":"good"},{"name":"bad","hasSteeringFiles":null}]}
```

→ `parse` fails → `to_notification` warns and returns `Ok(None)` (`:114-121`) →
`SessionController.powers` stays `None` → `/powers` answers "No powers reported yet" for
the whole session.

The field's own doc at `:70-73` states the opposite intent: "rejecting the frame would
discard a power the user has over a field cyril does not act on." The malformed-frame
fence tests `"powers": null` but never a null on a defaulted ITEM field.

Realism is the weak leg (`JSON.stringify` omits `undefined`; the capture populates both
fields), so the trigger is unobserved — but the code does not handle it.

**Fix:** one line — `Option<T>` + `unwrap_or_default()`.

---

## 10. `powers` is never cleared

**File:** `crates/cyril-core/src/session.rs:52` · **PLAUSIBLE** · state-management

`SessionCreated` resets every other per-session field and `BridgeDisconnected` clears
`context_usage` for staleness, but both leave the catalog — and the fence forbids any pull
to recover.

**Failure scenario.** The only arm touching `self.powers` is `PowersChanged` (`:223`).
`SessionCreated` (`:310-334`) resets `session_cost`, `context_usage` ("so a prior
session's value doesn't linger"), `last_turn`, `pending_*`, `steering_unsupported`;
`BridgeDisconnected` clears `context_usage` with "A dead connection has no live context —
clear it so a consumer doesn't read a stale value as current."

Sequence: session A pushes 3 powers; `/new`; if B's single push is dropped (one drift
field, or `/load` rather than `session/new`), `/powers` reports A's catalog as B's
forever, and there is no path back to the documented `None`.

**Why PLAUSIBLE, not CONFIRMED:** `kas_hooks`, `modes`, `models` and `cached_model` are
also uncleared, there is no in-process reconnect path, and the push lands +18 ms after
`session/new` so `/new` normally self-heals.

**Confirming probe:** a second `session/new` on one live KAS connection, to see whether
the push is per-session or per-connection.

---

## 11. A re-spelled push vanishes silently

**File:** `crates/cyril-core/src/protocol/convert/kas/powers.rs:108` · **CONFIRMED** · observability

`to_notification` rejects any non-exact method with a bare `Ok(None)` and no near-miss
warning, so a re-spelled push vanishes below the default log level — the sibling adapter
warns for exactly this reason.

**Failure scenario.** `convert/kas/workflow.rs:449-455` warns on
`starts_with("kiro/workflow/")` with the comment "a tenth lifecycle kind would otherwise
vanish into the generic unknown-extension `debug!`".

If KAS ships `_kiro/powers/items_changed_v2` or re-spells the push, this arm returns
`Ok(None)`, `KasEngine` falls through to `convert::kiro::to_ext_notification`, whose
`other =>` arm logs at `debug!`. `/powers` then answers "No powers reported yet" on a live
KAS session indefinitely and nothing in `cyril.log` ever names the powers family.

The `engine.rs:334-340` comment defends this as leaving "the near-miss performance fence
below unaffected" — that is precisely what makes drift invisible.

---

## 12. A blank name produces a blank row, sorted first

**File:** `crates/cyril-core/src/types/power.rs:45` · **CONFIRMED** · correctness

`PowerInfo::new` normalizes only exactly-empty strings and `WirePower.name` may be `""`,
so `title()`'s documented "Always non-empty ... cannot produce a blank row" is false in
two ways, and the blank row sorts to the TOP.

**Failure scenario.** `display_name.filter(|value| !value.is_empty())` keeps `"   "`,
which becomes the title and paints an all-blank BOLD row via `truncate_and_pad`.
`{"name":""}` decodes (the converter rejects only a missing or non-string name), making
all three rows blank. Because `show_powers_panel` keys on `title().to_ascii_lowercase()`
and `""` / `"   "` sort below every digit and letter, the blank entry lands first while
the header still reads "N powers".

The repo already ships the trimming helper — `protocol/kas/discovery.rs:155`:

```rust
fn nonempty(v: Option<String>) -> Option<String> { v.filter(|s| !s.trim().is_empty()) }
```

whose test asserts `nonempty(Some("   ")) == None`.

The same unguarded pattern leaves `mcp_server_names` entries unfiltered, so
`["","datadog"]` renders `· mcp  · mcp datadog` — a token naming nothing.

---

## 13. The paste guard omits the powers panel

**File:** `crates/cyril/src/app.rs:1609` · **CONFIRMED** · correctness

The `Event::Paste` guard names only `has_usage_panel()`; the PR added the powers panel to
the key chain and the mouse-scroll guard but not to this third guard, so a paste is not
consumed by the modal.

**Failure scenario.**

```rust
Event::Paste(text) => { if !self.ui_state.has_usage_panel() { self.ui_state.insert_text(&text); ... } }
```

A bracketed paste is not a key event, so it bypasses `handle_key` and the whole modal
chain; `insert_text` (`state.rs:1664-1669`) really mutates the textarea.

**Two corrections to the first-pass claim:** this is PRE-EXISTING (`main` has the
byte-identical guard, so `/hooks` and `/code` share it) and the input is NOT hidden —
`modal::place` keeps the popup strictly above `input_top` (popup ends row 17, input starts
row 18), so the text is visible below the panel.

It is a modal-consumption violation, not a silent one; the repo's own
`usage_modal_command_and_key_priority` test (`app.rs:3393`) asserts "modal consumes paste"
for the sibling.

---

## 14. Two new regression fences cannot fail

**File:** `crates/cyril-ui/src/state.rs:5633` · **CONFIRMED** · test-coverage

The ordering fence never checks the tie-break it names, and the App fence's "positive
control" bypasses the arm it claims to control.

**Failure scenario.** `state.rs:5633` compares `power("datadog","Datadog Observability")`
and `power("datadog-copy","Datadog Observability")` through `map(PowerInfo::title)`, which
yields the identical string for both — so dropping `power.name().to_owned()` from the sort
key entirely leaves the test green despite its message "the duplicate title is ordered by
id, not dropped".

`app.rs:5516` comments "the command result is what opens it" but then calls
`app.ui_state.show_powers_panel(catalog.clone())` directly; `handle_command_result`'s
`CommandResultKind::ShowPowers` arm is never entered, so deleting that arm keeps both
halves of the test passing, and the parenthetical "(the scroll offset is preserved)" is
never asserted.

Compare `powers_census_is_line_ending_agnostic` (`:259`), which reduces to
`f(lf) == f(lf)` because `crlf.replace("\r\n","\n") == lf`.

---

## 15. The documented overlay chain is stale

**File:** `CLAUDE.md:295` · **CONFIRMED** · docs

The documented key-handling overlay chain was not updated for the new panel, breaking the
rule stated four lines below it.

**Failure scenario.** `CLAUDE.md:300` states "Any new modal overlay must be added to both
this chain and the mouse-scroll guard in `handle_terminal_event`." The mouse-scroll guard
was updated (`app.rs:1585`) and the code chain was updated (`app.rs:1663`), but the
numbered reference chain in CLAUDE.md still reads 4=Hooks panel, 5=Code panel,
6=Autocomplete with no powers layer — so the checked-in reference now disagrees with
`handle_key`'s actual order (approval → picker → hooks → powers → code → usage).

The next contributor adding an overlay reads a chain missing a layer and places theirs
against stale priorities, which is how the paste-guard asymmetry (finding 13) survives.

---

# Below the reporting cap

These were ranked next and cut only by the 15-item limit.

## 16. Sort-key doc comments overstate "case-insensitive"

**Files:** `crates/cyril-ui/src/state.rs:2418`, `crates/cyril-ui/src/traits.rs:528` · CONFIRMED

The sort key is ASCII. The *code* matches `.cyril-v19o/spec.md:34`, so this is not a
behaviour bug — it is the two doc comments that overstate the guarantee as
"case-insensitive". Fix the prose, not the sort.

## 17. `/powers` is registered unconditionally and is inert in a default build

**File:** `crates/cyril-core/src/commands/mod.rs` (registration) · CONFIRMED, latent

`docs/kiro-2.20.1-wire-audit.md:128` records that Kiro's own TUI added `/powers` in
2.20.1. `bridge.rs:283-284` makes the command permanently inert in a default
`cargo build` — while still advising the user to "start a KAS session first", which is
advice they cannot act on. Either gate the registration on the engine or change the
message to name the real precondition.

## 18. The transport fence couples two outcomes through ordering

CONFIRMED. The transport fence's two outcomes are order-coupled, so the test's result
depends on which arrives first rather than on the property under test.

## 19. `refresh_powers_panel` reports a redraw for a no-op

**File:** `crates/cyril/src/app.rs:2041` · CONFIRMED

`refresh_powers_panel` returns `true` for a push that changed nothing, forcing a redraw
that paints identical pixels. Also carries a wasted clone, and `redraw_needed` at
`app.rs:2041` is dead.

## 20. Altitude: the fifth hand-copied overlay

Architectural root cause behind findings 3, 5, 6, 8 and 13.

This is the fifth hand-copied overlay across roughly 17 edit sites the compiler never
checks. Each copy inherits the sibling's arithmetic without the sibling's layout —
`CHROME_ROWS = 4` came from a panel with a header this one lacks (finding 5); the scroll
clamp came from a list whose rows are 1 line, not 3 (finding 8); the paste guard was never
generalized past the first panel (finding 13).

A `ScrollList<T>` plus a single `Overlay` enum would collapse all five into one place and
make the key-priority/z-order relationship (finding 3) a compile-time property rather than
a convention.

---

# Retracted during verification

Recorded so they are not re-derived by a future review.

**The dropped `sessionId` is harmless.** The KAS `PowersManager` fans one process-global
catalog out to every session, and the only session-varying frame carries no `powers` key.
The residual defect is documentation only: the converter's doc comment at
`powers.rs:46-50` is wrong and contradicts `docs/kiro-2.20.1-wire-audit.md:147`.

**The turn-liveness stamp does not cost the cancel affordance.** `state.rs:448-456` clears
the stall chip from a six-variant allowlist that excludes `PowersChanged`. The residual
harm is a first chip delayed up to 30 s, which every session-less frame already causes.

---

# Suggested fix order

1. **Finding 1** — delete or invert the `list` fence. It gates 9 and 10, so nothing
   downstream can be fixed while it stands.
2. **Finding 2** — one-line move of `names.push("powers")` above `HelpCommand::new`.
3. **Findings 5, 7, 8** — the arithmetic bugs in the widget and scroll clamp. Small,
   independent, each needs a test that actually triggers the clamp.
4. **Finding 3** — the overlay guard. Larger; consider doing it as part of finding 20
   rather than patching one more pair.
5. **Findings 9, 11, 12** — converter hardening: `Option<T>` + `unwrap_or_default()`, a
   near-miss `warn!`, and reuse of `discovery.rs`'s `nonempty` helper.
6. **Findings 4, 14** — repair the fences so they can fail.
7. **Findings 15, 16** — documentation corrections.
8. **Finding 10** — after 1 lands, decide clear-on-`SessionCreated` vs pull-on-demand.
   Run the confirming probe first.
