# cyril-dij8 — build audit (slice 9, C9 close-out)

Branch: `feat/cyril-dij8-chrome-semantic-colors`, base `d4f105f`.
Final integration run 2026-07-14: full workspace `cargo test` (15/15
result groups ok), `cargo clippy --all-targets -- -D warnings` clean,
`cargo fmt --check` clean, doctests clean, representability falsifier
missing=0 + phantom control fires.

## Diff scope (C9)

`git diff d4f105f --stat`: production changes confined to
`widgets/{toolbar,crew_panel,voice}.rs` (literal→role swaps + `theme:
&Theme` params), `render.rs` (+4 caller args, +1 frame fence test),
`theme.rs` (test-only fences), `lib.rs` (+2 lines cfg(test) module),
`chrome_theme_tests.rs` (new, cfg(test)), the frozen baseline TSV, and
the three sanctioned snapshot re-baselines (every changed cell y:0/y:23,
named→canonical RGB only — inspected at slice 4). No `state.rs`, no
`traits.rs`, no geometry arithmetic, no `cyril-core` changes. `.rivets/`
untouched on this branch (parallel-session discipline). Artifacts under
`.cyril-dij8/` complete the audit trail.

## Claim → landed fence map (all 10 green)

| Claim | Fence (permanent CI form) |
|---|---|
| C1 representability | `theme::tests::chrome_legacy_colors_are_representable` (+ one-shot #123456 mutation check at slice 1; script control fires) |
| C2 equivalence | `chrome_theme_tests::chrome_baseline_equivalence` vs frozen TSV @44bd61c (guarded slices 4–6 live) |
| C3 role wiring | `chrome_theme_tests::marker_wiring_{toolbar,status,crew,voice}` (all three value-twin pairs asserted) |
| C4 no legacy sources | `theme::tests::chrome_widgets_have_no_legacy_color_sources` (control: predicate fires on d4f105f toolbar.rs, 26 literals) |
| C5 single theme flow | `render::tests::chrome_frame_uses_state_theme` (+ existing `conversation_frame_uses_state_theme_once` pins the single `state.theme()` call) |
| C6 no-color reset | `chrome_theme_tests::no_color_scenarios_reset` |
| C7 label distinguishability (AC2) | `chrome_theme_tests::no_color_status_distinguishable` |
| C8 scenario completeness | `chrome_theme_tests::baseline_covers_probe_inventory` (20-tuple normalized SET; deviation from raw-23 count documented at slice 2 — 3 named-collapses across toolbar/status shared bg, verified item-by-item) |
| C9 behavior untouched | full pre-existing suites pass unmodified (only sanctioned snapshot re-baselines); this audit |
| C10 edge shapes | `chrome_theme_tests::edge_*` (idle toolbar, strict 70/90 boundaries, crew headers, idle voice) |

## Issue AC → evidence

- AC1 (semantic roles for normal/muted/success/warning/danger/accent/
  selection states): all chrome states render via theme roles (C4 scan =
  zero literals); status families map VGA-exact per approved posture —
  success-family=`subdued_positive`, warning-family=`emphasis`,
  danger-family=`subdued_negative`, accent=`accent_quinary`/`_quaternary`,
  muted=`subdued`/`muted`, normal=`text`. Chrome has no selection state —
  this audit's own probe-verified conclusion (no selectable rows in any of
  the four surfaces; selection UI lives in the pickers/approval overlays
  migrated by cyril-nrnq). The design does not carry an explicit N/A note.
- AC2 (meaning not by color alone): C7 fence.
- AC3 (Cyril Dark visually equivalent): C2 zero-normalized-diff fence over
  18 scenes; snapshot re-baselines named→RGB only.
- AC4 (full workspace quality gate): this audit's integration run.

## Deviations from plan (all documented in slice commits)

1. Slice 2: C8 pinned as 20-tuple normalized SET instead of raw count 23
   (named-collapse arithmetic verified item-by-item).
2. Slices 4–6 also touched `chrome_theme_tests.rs` call sites (signature
   ripple, compile-enforced; anticipated at plan critique).
3. Slice 8 parameterized scene builders by theme (needed for NoColor
   scenes; theme moved to first param for uniform call sites).
4. Design step 2b sanctioned re-baselining "the two full-frame insta
   snapshots"; three exist — `theme_seam_picker` also contains toolbar/
   status cells, so it re-baselined too. Sanctioned at plan slice 4;
   inspected delta is named→canonical RGB at y:0/y:23 only, same as the
   other two.

## Discovered issues to file at close-out

None — no `to-file.md` entries accumulated during the build.
