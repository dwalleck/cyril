# Plan: cyril-v19o — `/powers` panel from the KAS powers push

Consumes `.cyril-v19o/design.md` (approved 2026-09-10, verbatim `"Approve design.md"`) and `.cyril-v19o/spec.md`. Route: Empirical (`route.md`).

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-core/src/protocol/convert/kas/powers.rs` | 0 | 130–190 | add: powers frame recognition + validation | new `to_notification(method, params) -> Result<Option<Notification>>` | N/A — new module |
| `crates/cyril-core/src/types/power.rs` | 0 | 60–95 | add: domain payload | new `PowerInfo` accessors | N/A — new module |
| `crates/cyril-core/src/types/mod.rs` | 89 | 90 | add: module registration | none | N/A |
| `crates/cyril-core/src/types/event.rs` | 1128 | 1140–1155 | add: `PowersChanged` variant | enum variant | N/A |
| `crates/cyril-core/src/protocol/engine.rs` | 839 | 850–862 | add: one dispatch arm in `KasEngine` | none (trait unchanged) | N/A |
| `crates/cyril-core/src/session.rs` | 1247 | 1280–1305 | add: catalog storage + getter | new `powers()` reader, new `apply_notification` arm | N/A |
| `crates/cyril-core/src/commands/mod.rs` | 1438 | 1445–1455 | add: result kind | new `ShowPowers { powers }` variant | N/A |
| `crates/cyril-core/src/commands/builtin.rs` | 455 | 520–550 | add: `/powers` command | new `Command` impl | N/A |
| `crates/cyril-ui/src/traits.rs` | 1324 | 1360–1385 | add: panel view type + accessor | new `PowersPanelState`, new `TuiState::powers_panel()` | N/A |
| `crates/cyril-ui/src/state.rs` | 7926 | 8040–8085 | add: panel lifecycle (same cluster as the four existing panels) | new panel methods, one `apply_notification` arm | protected: panel-lifecycle methods + one arm only; no wire parsing, no chat lines |
| `crates/cyril-ui/src/widgets/powers_panel.rs` | 0 | 190–250 | add: painting | new `render(...)` | N/A — new module |
| `crates/cyril-ui/src/widgets/mod.rs` | 13 | 14 | add: module registration | none | N/A |
| `crates/cyril-ui/src/render.rs` | 1492 | 1495–1500 | add: one overlay arm | none | N/A |
| `crates/cyril/src/app.rs` | 6857 | 6910–6945 | add: three wiring arms (notification, key, command result) | none | protected: those arms only; no new `App` field, no parsing, no formatting, no ordering |
| `crates/cyril/tests/powers_source_fence.rs` | 0 | 70–100 | add: absence census | none | N/A — new test file |
| `crates/cyril-core/tests/fixtures/kas/powers/items-changed-2.21.2.json` | 0 | ~45 | add: committed wire fixture | none | N/A |
| `experiments/conductor-spike/kas-powers-2.21.2.{jsonl,verdict.json,py}` | 0 | ~300 | add: audit provenance for C0 | none | N/A |

No module gains a second responsibility cluster: `state.rs`'s delta extends the existing panel-lifecycle cluster, and `app.rs`'s delta stays inside the routing arms the design's protected-parent rule allows.

## Partition arithmetic

| Slice | Diff estimate (changed lines) |
|---|---:|
| 1 — core conversion | 920 |
| 2 — panel view | 660 |
| 3 — command + wiring | 250 |
| 4 — absence fence + shape proof | 440 |
| **Sum** | **2270** |
| Churn margin | 590 (26% — this change rewrites no existing behavior, but empirical wire work has historically grown fixtures and tests at step 6; the margin covers a second malformed-shape fixture set and one widget-layout iteration) |
| **Total** | **2860** |

2860 ≤ 4000 → **single PR increment**.

### PR increment: `feat/cyril-v19o` (one PR, all four slices)

Slices 1–4 in order. Mergeable definition: the branch builds, the four crates' test suites pass, `cargo clippy -- -D warnings` and `cargo fmt --check` are clean, the module-shape oracle is green over the whole diff, and the live SC1/SC2 check ran against kiro-cli 2.21.2. Verification seams, without later slices: slice 1 verifies alone from the committed fixture (no client needed); slice 2 verifies from widget/state tests (no command needed); slice 3 verifies from command/app tests (no live agent needed); slice 4 verifies from the census + oracle (no runtime at all). Every slice therefore verifies without the increments after it; the increment is a single PR because the total is under the partition rule.

---

## Slice 1: A KAS powers push becomes a typed domain catalog, offline-verifiable from the live fixture

**Claim IDs:** [C1, C2, C3] — and the C0 provenance artifacts are committed here, since this slice creates the fixture C0 attests.
**Expected behavior:** `convert::kas::powers::to_notification("kiro/powers/items_changed", params)` returns `Ok(Some(Notification::PowersChanged { powers }))` carrying exactly the wire values for the recorded 3-item frame; returns a `Some` with an empty list for `powers: []`; returns `Ok(None)` (after a `warn!`) for a frame whose `powers` key is missing, is not an array, or holds an item without `name` or that is not an object; `SessionController::apply_notification` records the catalog, replaces it on a later valid push, leaves it untouched on a malformed frame, and exposes it through `powers()`. `KasEngine` dispatches the frame to this converter and falls through to the shared kiro converter for anything else.
**Oracle:** the committed fixture is a canonicalized slice of the live 2.21.2 capture (C0, PASS); the test's expected values are the `jq`-extracted capture values, and malformed frames are classified independently by a `jq` predicate.
**Stress fixture:** one case per malformed shape (missing key, `powers: null`, `powers: {}`, item is a string, item missing `name`, item with an unknown extra key, `displayName: null`) plus a 200-item list — each expected outcome written now: the six malformed cases → `Ok(None)` and an unchanged catalog; the unknown-extra-key case → converts with the known fields only; the 200-item case → converts with 200 items in wire order, no truncation, no dedup.
**Regression fence:** `convert/kas/powers.rs` tests `powers_frame_maps_every_field_from_the_capture`, `empty_catalog_is_loaded_not_dropped`, `malformed_powers_frames_drop_and_do_not_clear`; `session.rs` test `powers_catalog_replaces_and_survives_malformed_pushes`; `engine.rs` test `kas_engine_dispatches_powers_frames` — created in THIS slice.
**Named mutation:** in `convert/kas/powers.rs`, swap `display_name` and `name` when constructing `PowerInfo` (C1); return `Ok(None)` when the parsed list is empty (C2); replace the `Option<Vec<_>>` extraction with `unwrap_or_default()` (C3). Each is applied to this slice's fences by checkpointed-build: red, restore, green.
**Complexity/production scale:** per push, one `serde_json` deserialize of the frame plus one `O(N)` pass over `powers` (N = 3 live; N is bounded by the user's installed catalog). Production-scale input: N ≈ 3–50. Resulting bound: `O(N)`, one allocation per item, no per-frame work proportional to session length. Slice maximum accepted cost: **1 ms per push at N ≤ 1000** — rationale: the frame arrives once per session (measured: once, +18 ms after `session/new`) and shares the ordered domain queue with tool calls, so a single linear pass must stay far below the 50 ms fast-tick frame time even for a pathological catalog.
**Wall budget/phase:** one-off phase — the converter runs once per received frame; no wall budget required.
**Module shape:** adds the converter module `convert/kas/powers.rs` (owner of wire recognition + validation), the payload module `types/power.rs` (owner of the domain payload), one enum variant, one engine dispatch arm, and the `SessionController` catalog (owner of last-write-wins storage). Protected parents: `crates/cyril-ui/src/state.rs` and `crates/cyril/src/app.rs` — expected production delta **0 lines in this slice**. Shape fence: `python3 .cyril-v19o/oracles/module_shape.py` (authored in this slice so every later slice can be gated) → `PASS C1 C2 C3 C8`, reporting the new paths and a 0-line delta for both protected parents.
**Files:** `crates/cyril-core/src/protocol/convert/kas/powers.rs` (create), `crates/cyril-core/src/protocol/convert/kas.rs` (register the submodule), `crates/cyril-core/src/types/power.rs` (create), `crates/cyril-core/src/types/mod.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/engine.rs`, `crates/cyril-core/src/session.rs`, `crates/cyril-core/tests/fixtures/kas/powers/items-changed-2.21.2.json` (create), `experiments/conductor-spike/kas-powers-2.21.2.jsonl` + `.verdict.json` + `probe-kas-powers-2.21.2.py` (create, copied from `.cyril-v19o/`), `.cyril-v19o/oracles/module_shape.py` (create).
**Estimate:** 2–3 h.
**Diff estimate:** 920 changed lines (implementation ~280, tests ~280, fixture ~45, audit artifacts ~300, oracle ~180 — oracle counted here, its mutation script in slice 4).
**PR increment:** `feat/cyril-v19o`.
**Commands and expected results:**
- `cargo test -p cyril-core powers` → the three converter fences pass; item-by-item agreement with the fixture's `jq`-extracted values (`name`/`displayName`/`mcpServerNames`/`hasSteeringFiles` for all three powers: aws-infrastructure-as-code `false`, datadog `true`, markdownlint `true`), the empty-list fence passes, all malformed shapes return `Ok(None)`, and the session fence shows the catalog unchanged after a malformed push.
- `python3 .cyril-v19o/oracles/module_shape.py` → `PASS` naming `convert/kas/powers.rs` and `types/power.rs`, and reporting 0-line production deltas for `app.rs` and `state.rs`.
- `cargo clippy -- -D warnings && cargo fmt --check` → clean.

---

## Slice 2: The panel paints the approved layout from a view state the state layer owns

**Claim IDs:** [C4, C6]
**Expected behavior:** `PowerInfo::title()` returns `displayName` when present and non-empty and the id otherwise; `description()`/`mcp_server_names()` are empty when absent; `UiState::show_powers_panel` opens the panel with rows ordered by display name (ASCII-lowercase) with id as tie-break; `refresh_powers_panel` replaces an open panel's contents and clamps the scroll offset, returning `false` when closed; `widgets::powers_panel::render` paints the three-line row layout (title / id + `mcp <server>` tokens + `steering` / description), the `No powers installed` placeholder for a known-empty catalog, and never paints outside `modal::place`'s area.
**Oracle:** the expected layout is a hand-written table of cell columns for a fixed 80×24 `TestBackend` frame; the expected order comes from `sort -f` over the fixture's display names; the bounds invariants reuse the existing `modals_never_cover_input` invariant.
**Stress fixture:** CJK wide-character title sized to the exact inner width (must not cross the border), a title and description longer than the panel (truncated, no panic), a 200-item catalog with a 15-row viewport (only the window paints; scroll clamps), a zero-height area (`place` → `None` → nothing painted, no panic), duplicate display names (both rows render, tie-break by id), `displayName: ""` (falls back to the id, no blank row), an empty `mcp_server_names` (no `mcp` token), and two mcp servers (both tokens, no overlap). Expected outcome for each is written now.
**Regression fence:** `widgets/powers_panel.rs` tests (`layout_matches_the_approved_row_shape`, `empty_catalog_shows_placeholder`, `wide_title_clamps_to_the_panel`, `viewport_window_clamps_scroll`, `tiny_area_paints_nothing`), `types/power.rs` accessor test `title_falls_back_to_id_for_empty_display_name`, `state.rs` test `powers_panel_orders_and_replaces` — created in THIS slice.
**Named mutation:** in `PowerInfo::title()`, drop the `is_empty` guard (C4); in `powers_panel.rs`, drop the width clamp on the title row (C6); in `powers_panel.rs`, drop the `steering` token (C6); in `UiState::show_powers_panel`, remove the sort (C4's ordering / C6's panel-order check). Each red, restore, green.
**Complexity/production scale:** render is `O(V)` in visible rows (V ≤ 15 by the panel's own clamp) plus `O(T)` in tokens per row; the state setter sorts `O(N log N)` once per open/refresh. Production-scale input: N ≈ 3–50, V ≤ 15. Resulting bound: painting ≤ 15 rows per frame regardless of catalog size. Slice maximum accepted cost: **1 ms per frame at N ≤ 1000** — rationale: the panel renders on every frame while open, so cost must be independent of catalog size (it is, via the viewport window); a linear scan of a 1000-item catalog per frame would still be sub-millisecond but is explicitly not what the widget does.
**Wall budget/phase:** always-on while the panel is open — the panel sits on the render path for every frame until dismissed; budget ≤ 1 ms/frame at N ≤ 1000 rows, rationale: the fast tick while any stream is active is 50 ms, so 1 ms leaves 98% headroom for chat rendering, and the widget's work is bounded by the 15-row window rather than the catalog.
**Module shape:** adds `widgets/powers_panel.rs` (owner of powers painting), `PowersPanelState` + `TuiState::powers_panel()` in `traits.rs` (owner of the read-only projection), and the panel-lifecycle methods in `state.rs` (owner of panel state, ordering, clamping). Protected parents: `crates/cyril-ui/src/state.rs` — expected delta ≤ 160 production lines, all inside the panel-lifecycle cluster, zero wire parsing and zero chat-message emission; `crates/cyril/src/app.rs` — expected delta **0 lines in this slice**. Shape fence: `python3 .cyril-v19o/oracles/module_shape.py` → `PASS`, with `state.rs`'s delta confined to the named methods (the oracle asserts the absence of a `serde` import in `cyril-ui` and the presence of the panel methods).
**Files:** `crates/cyril-ui/src/widgets/powers_panel.rs` (create), `crates/cyril-ui/src/widgets/mod.rs`, `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/render.rs`, `crates/cyril-ui/tests/widget_theme_sources.rs` (add the module if that audit enumerates widget modules), `crates/cyril-ui/tests/modal_theme.rs` (regenerate only if the audit enumerates the new module).
**Estimate:** 3–4 h.
**Diff estimate:** 660 changed lines.
**PR increment:** `feat/cyril-v19o`.
**Commands and expected results:**
- `cargo test -p cyril-ui powers` → every layout assertion holds at the fixed frame size: the 3-item panel shows each display name on its own title row, the id, one `mcp` token per server, `steering` for the two powers whose wire flag is `true` and not for the `false` one, and the placeholder for the empty catalog; the CJK title neither crosses the border nor panics; the zero-height area paints nothing.
- `cargo test -p cyril-ui` → the full UI suite passes, including `modals_never_cover_input` with the powers panel open.
- `python3 .cyril-v19o/oracles/module_shape.py` → `PASS` with the `state.rs` delta reported inside the panel cluster and `app.rs` at 0.
- `cargo clippy -- -D warnings && cargo fmt --check` → clean.

---

## Slice 3: `/powers` answers, and an unprompted push refreshes without opening

**Claim IDs:** [C5]
**Expected behavior:** `/powers` with no catalog held adds exactly one system line (`No powers reported yet — start a KAS session first.`) and opens nothing; with a catalog held it returns `ShowPowers { powers }` and the App opens the panel with those rows; `/powers <unrecognized argument>` adds a usage line and opens nothing; a `PowersChanged` push with no panel open leaves the panel closed while still updating the session catalog; with the panel open it replaces the contents; Esc closes it, arrows scroll one line, page keys scroll ten.
**Oracle:** the expected order comes from `sort -f` over the fixture's display names (`Build AWS infrastructure…`, `Datadog Observability`, `Markdownlint`); the no-auto-open assertion is checked against a rendered frame with a positive control (the same render after `/powers` must contain the panel); the absence of bridge traffic is checked against the bridge harness's recorded outbound frames, which are captured below the command's code path.
**Stress fixture:** `/powers` with an empty catalog (panel must still open and show the placeholder), `/powers` with the catalog held, `/powers extra`, and two pushes in one session (second shorter) with the panel scrolled to the end — expected outcome for each written now: the second push's rows replace the first, duplicates are not collapsed, and `scroll_offset` is clamped so the viewport is never stranded past the end.
**Regression fence:** `commands/builtin.rs` tests `powers_without_catalog_reports_and_with_catalog_opens`, `powers_ignores_nothing_but_reports_usage_for_arguments`; `crates/cyril/src/app.rs` tests `powers_push_updates_without_opening`, `powers_command_result_opens_the_panel`, `powers_panel_key_map` — created in THIS slice.
**Named mutation:** in the App's notification arm, call `ui_state.show_powers_panel(...)` unconditionally instead of `refresh_powers_panel` (C5 — the no-auto-open fence must go red); in `PowersCommand::execute`, return `Dispatched` instead of the not-loaded system message (the command fence must go red). Each red, restore, green.
**Complexity/production scale:** per command/push: one clone of the catalog (`O(N)`, N ≤ 1000) and one sort (`O(N log N)`); no new loop in the App arms. Slice maximum accepted cost: **1 ms per command/push at N ≤ 1000** — rationale: both paths run on the event loop, and a 1000-item clone+sort must stay two orders of magnitude below the 50 ms tick.
**Wall budget/phase:** one-off phase — command execution and push handling each run once per user action or agent event; no wall budget required.
**Module shape:** adds one result-kind variant (`commands/mod.rs`), the `PowersCommand` impl (`commands/builtin.rs`), one `UiState::apply_notification` arm, and three `App` wiring arms. Protected parents: `crates/cyril/src/app.rs` — expected delta ≤ 45 production lines, all inside the notification arm / key-dispatch function / command-result arm, with no new `App` field and no parsing or formatting; `crates/cyril-ui/src/state.rs` — expected delta ≤ 12 lines for the `apply_notification` arm. Shape fence: `python3 .cyril-v19o/oracles/module_shape.py` → `PASS`, reporting both deltas inside their allowed regions and failing if any `App` field is added.
**Files:** `crates/cyril-core/src/commands/mod.rs`, `crates/cyril-core/src/commands/builtin.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril/src/app.rs`.
**Estimate:** 2–3 h.
**Diff estimate:** 250 changed lines.
**PR increment:** `feat/cyril-v19o`.
**Commands and expected results:**
- `cargo test -p cyril-core powers && cargo test -p cyril` → the command fences pass (one system line, no panel, when no catalog is held; `ShowPowers` carrying the three fixture rows otherwise), and the App fences pass: the push with the panel closed updates the catalog and leaves the frame free of the panel text while the positive control shows the panel after `/powers`; the key map hides and scrolls.
- `python3 .cyril-v19o/oracles/module_shape.py` → `PASS` with `app.rs`'s delta inside the three allowed arms.
- `cargo clippy -- -D warnings && cargo fmt --check` → clean.

---

## Slice 4: No outbound powers traffic exists, and the diff matches the module ledger

**Claim IDs:** [C7, C8]
**Expected behavior:** a census over `crates/` finds zero occurrences of `powers/list` or `powers/refresh` in production sources while finding `powers/items_changed` (non-vacuity control) and a minimum number of scanned files; driving `/powers` through the bridge harness emits no powers method on the outbound path while the same run records at least one other outbound frame; the module-shape oracle passes over the whole assembled diff, and each named mutation turns it red.
**Oracle:** an independent `grep -rn` census over the tree (different mechanism from the in-crate test's file walk) and the bridge harness's transport-level frame recording (below the command's code path); the shape oracle is a source/diff census written independently of the Rust code.
**Stress fixture:** the census run against a temporary directory with no matches must fail the non-vacuity self-check (proving the fence cannot pass by scanning nothing), and a source file with the forbidden string must fail it; the oracle must fail when `WirePower` is moved into `widgets/powers_panel.rs`. Expected outcomes written now.
**Regression fence:** `crates/cyril/tests/powers_source_fence.rs`, the bridge negative test, and `.cyril-v19o/oracles/mutations.sh` (with the full red/restore/green proof) — created in THIS slice.
**Named mutation:** add a `BridgeCommand` send carrying a `_kiro/powers/list` frame inside `PowersCommand::execute` → both the census and the bridge test go red; move a wire struct into `widgets/powers_panel.rs`, add a `serde_json` import to the widget, and add a field to `App` → the oracle reports the exact path for each and fails. Each red, restore, green.
**Complexity/production scale:** N/A — the fence and oracle are test-time (one-off) tree scans, not production paths.
**Wall budget/phase:** one-off phase — the census and oracle run once per gate invocation (test suite / checkpoint), not per request; no wall budget required.
**Module shape:** no production responsibility moves; this slice creates the mechanical proof (C8) over the ledger the earlier slices established. Protected parents: `app.rs` and `state.rs` — expected production delta **0 lines in this slice**, asserted by the oracle's per-slice delta report.
**Files:** `crates/cyril/tests/powers_source_fence.rs` (create), `crates/cyril/tests/` bridge negative test file, `.cyril-v19o/oracles/mutations.sh` (create).
**Estimate:** 2–3 h.
**Diff estimate:** 440 changed lines.
**PR increment:** `feat/cyril-v19o`.
**Commands and expected results:**
- `cargo test -p cyril --test powers_source_fence` → passes with the non-vacuity control satisfied (the census reports the scanned file count and finds the inbound method string while finding no outbound request string).
- `bash .cyril-v19o/oracles/mutations.sh` → every named mutation prints its fence going red and its restoration going green; the script exits non-zero if any mutation fails to turn its fence red.
- `python3 .cyril-v19o/oracles/module_shape.py` → `PASS` over the assembled diff.
- `cargo test --workspace && cargo clippy -- -D warnings && cargo fmt --check` → clean.

---

## Tracker taxonomy

- Permanent non-goals, recorded in `design.md` with rationale (no tracker issue): parsing `keywords`/`isAgentPlugin`/`_meta`; reading `~/.kiro/powers` from disk or deriving fields from the filesystem; rendering powers anywhere but this panel; any pull/refresh path; v2 powers support.
- Intended future work, cited by verified tracker ID: **cyril-q159** (Agent Plugin format research — owns the `keywords`/mention flow), **cyril-58uv** (in_progress — extension-notification drop visibility), **cyril-nk4o** (the `_kiro/mcp/*` sibling panel), **cyril-oiyt** (KAS hooks-registry panel extension).
- No new tracker issues are needed: every deferred item already has a live issue or is a permanent non-goal with its rationale recorded in the design.

## Self-review

1. **Row→slice coverage** — C0 (slice 1, provenance committed there), C1/C2/C3 (slice 1), C4/C6 (slice 2), C5 (slice 3), C7/C8 (slice 4). Each row assigned exactly once; every `PENDING` falsifier is discharged by the slice implementing its claim, and each slice's Commands field carries the falsifier experiment and expected outcome while its Oracle field carries the independent comparison.
2. **Fourteen fields** — every slice records all fourteen, with `N/A — reason` in the conditional cells (`stress fixture`/`complexity`/`wall budget` are filled or reasoned, never empty).
3. **Fences and mutations** — each claim's fence is created in the slice implementing it; C0 alone carries `N/A — approved risk` for both its fence and mutation, copied verbatim from the design and covered by the recorded approval.
4. **Loops and budgets** — three loops exist (per-push conversion pass, widget viewport/token loop, setter sort); each states its asymptotics, production-scale sizes, resulting bound, and an explicit maximum accepted cost with rationale. Two always-on phases are recorded (panel render while open, with a wall budget); the rest are one-off.
5. **Module shape** — the growth ledger covers every touched module and both protected parents; no slice crosses an approved seam with unrelated responsibilities (slice 2 keeps wire knowledge in `cyril-core`, slice 3 keeps chat/command duties out of `cyril-ui`).
6. **Partition** — sum 2270, churn margin 590 (26%) documented, total 2860 ≤ 4000 → one increment; every slice names `feat/cyril-v19o` and the increment carries its mergeable definition and per-slice verification seams.
7. **Tracker taxonomy** — applied above; every intended-future-work item cites a verified tracker ID, every permanent non-goal records its rationale in the design.
8. **No completion claims** — this plan declares no slice complete; checkpointed-build judges completion.

## Plan amendments

- **Slice 1 (checkpoint record: `.cyril-v19o/checkpoints.md`).** Fixture path is `crates/cyril-core/tests/fixtures/kas/powers/items-changed-2.21.2.json` — the flat path first chosen broke the existing `schema_deserializes_captured_kas_session_updates` fence, which deserializes every top-level `kas/*.json` as a session update. The `UiState::apply_notification` arm moved from slice 3 to slice 1: a `Notification` variant cannot compile without its arm in that exhaustive match, and `crates/cyril/examples/test_bridge.rs` needed the same one-line arm (a file the plan did not list). Slice 3's `state.rs` delta therefore drops to 0 lines. Ledger correction: `experiments/conductor-spike/kas-powers-2.21.2.{py,jsonl,verdict.json}` = 468 lines actual against the ~300 projection (placement unchanged, no new responsibility). Revised sum: 2270 − 12 + 168 = **2426**, plus the 590-line churn margin (24%) = **3016** ≤ 4000, so the single-PR partition is unchanged.
- **Approved delivery deviation (2026-09-10, user decision `"One PR, accept the overshoot"`).** The size tripwire fired after slice 1: actual 3226 changed lines (the plan's per-slice estimates omitted the `.cyril-v19o/` artifacts, which the `cyril-qaq0` precedent counts — its PR was 3156). Honest recomputation is ~4570, above the exact 4,000 rule. Presented as the contract requires with three options; the requester chose a single PR over a stacked chain, accepting the overshoot so the adapter, fixture, provenance, panel, and fences stay reviewable in one place. Recorded here as the owning artifact's revision; no other plan field changes.
