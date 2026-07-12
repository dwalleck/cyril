# Prove-it prototype — cyril-qaq0

Status: **BLOCKED — hard gate not passed**  
Run: 2026-07-11

## Upstream gate

`.cyril-qaq0/spec.md` exists and contains the requester's verbatim sign-off:

> Confirmed - themes persist via dialog, tmux shift and edge cases accepted

## Tracker check

Prior art is recorded in `related-issues.md`. A fresh Rivets check found one
closed substrate ticket (`cyril-ghuu`) and six open blockers:
`cyril-cc5e`, `cyril-nrnq`, `cyril-dij8`, `cyril-fkke`, `cyril-6r3a`, and
`cyril-q9dx`. The last is the known ANSI-16 speaker-identity failure.

## Smallest questions

1. Can today's startup/config and `UiState` seams represent a selected bundled
   theme and automatic color mode?
2. Does the generic picker expose enough lifecycle information for immediate
   preview, Enter/persist, and Esc/restore?
3. Does the landed resolved-theme substrate project and render its one installed
   theme consistently in all four explicit modes?
4. Can a one-key persistence operation preserve production-shape TOML comments
   and formatting?

## Material-boundary inventory

<!-- markdownlint-disable MD013 -->

| Boundary | Material observation / falsifier | Evidence | Result |
| --- | --- | --- | --- |
| Representation and normalization | Unknown `theme`/`color_mode` must remain observable per-key values rather than disappear. `automatic` must be representable separately from resolved modes. | Runtime config probe + lexical oracle | **Blocked:** existing fields survive, but both new keys are silently discarded; `ColorMode` has no `Automatic`. |
| Selection and visibility | Moving selection must mutate the next frame's theme; Esc must retain the picker-open theme. Long lists must keep selection visible. | Runtime picker probe + lexical oracle + Rivets | **Blocked:** movement does not change theme, cancel discards picker state, confirm returns only `(title, value)`; `cyril-cc5e` remains open. |
| Mutable shared state | A live swap must have an owned mutation seam, and color caches must not leak earlier theme/mode values. | `UiState` runtime probe; focused cache tests | **Partial:** `UiState` owns a resolved `Theme`, and current mode-isolation tests pass, but there is no theme mutation seam and only one installed theme to alternate. |
| Ordering and concurrency | Selection event → state mutation → next redraw must be observable while streaming; Esc after the persistence dialog must restore the picker-open theme. | App source seam + runtime picker probe | **Blocked:** the event loop redraws after picker input, but no theme mutation or local nested lifecycle exists to observe. Generic Enter dispatches `BridgeCommand::ExecuteCommand` to the agent. |
| Transport and serialization | Startup must surface invalid values; persistence must alter one key, create a missing file, and surface write failure without undoing session state. | Config runtime probe; persistence fixture + Git diff oracle | **Partial:** a direct line update changes exactly one line, but production has no theme persistence seam; missing-file and write-failure behavior cannot be observed. |
| External-library semantics | Compiled Ratatui colors must agree with an independent palette projection; TOML decoding must not erase required diagnostics. | Compiled probe + Python projection oracle; config probe | **Partial:** 29-role projection agrees for the one installed theme, but Serde currently erases the two unknown UI keys and additional palettes are unavailable. |

<!-- markdownlint-enable MD013 -->

No boundary was excluded as immaterial.

## Probes

### Runtime seams

`python .cyril-qaq0/probe-runtime.py` creates a temporary public integration
test, runs it against the real crates, and removes the test file. Key output:

```text
QAQ0 config_preserved=max_messages:321 mouse_capture:false
QAQ0 picker_move=selected:1 theme_changed:false
QAQ0 picker_cancel=open:false theme_changed:false
QAQ0 picker_confirm=Some(("theme", "cyril-light")) theme_changed:false
QAQ0 mode=TrueColor user=Rgb(138, 180, 248) syntax=Some(Base16EightiesDark)
QAQ0 mode=Ansi256 user=Indexed(111) syntax=Some(Base16EightiesDark)
QAQ0 mode=Ansi16 user=Gray syntax=Some(Base16EightiesDark)
QAQ0 mode=None user=Reset syntax=None
```

### Compiled projection and render/cache substrate

The existing compiled test emitters produced `theme-probe.tsv` and
`no-color-probe.tsv`. Focused current-code tests passed:

```text
all_sixteen_scene_mode_combinations_pass: PASS
cache_never_leaks_truecolor_into_no_color_in_either_order: PASS
syntax_and_markdown_caches_isolate_truecolor_and_no_color_in_both_orders: PASS
```

This certifies only the single installed `CyrilDark` theme. It does not certify
live mutation or the pending bundled palettes.

### Persistence shape

`python .cyril-qaq0/probe-persist.py` updates the real fixture copy without
re-serializing it:

```text
QAQ0 persist_matches=1 changed_lines=1
QAQ0 before=theme = "cyril-dark" # keep this inline comment
QAQ0 after=theme = "gruvbox-dark" # keep this inline comment
```

## Oracle

The independent mechanisms agree on every observation that could be made:

<!-- markdownlint-disable MD013 -->

| Observation | Probe mechanism | Independent oracle | Agreement |
| --- | --- | --- | --- |
| Config shape | Compiled `Config::load_from_path` integration test | Regex inventory of typed `UiConfig` fields | Existing fields preserved; no `theme` or `color_mode` field. |
| Registry/modes | Compiled `resolve` calls | Lexical enum inventory | One theme; four explicit modes; no `Automatic`. |
| Picker lifecycle | Compiled public `UiState` calls | Static method/handler shape inspection | Selection never mutates theme; cancel discards; confirm is value-only and routes to agent execution. |
| Color projection | Compiled Rust role rows | `.cyril-ghuu/projection-oracle.py`, independently computing nearest palettes | `AGREE compiled-theme roles=29 rgb=28 ansi256=28 ansi16=28 no-color=29`. |
| Persistence fidelity | Regex one-key updater | `git diff --no-index --numstat` | `1` deletion + `1` addition, both on the theme assignment only. |

<!-- markdownlint-enable MD013 -->

Run the static oracle with `python .cyril-qaq0/oracle-static.py`.

## What I learned

The generic picker is not merely missing a preview callback: Enter closes it and
routes `(title, value)` to the agent as `ExecuteCommand`, while Esc discards all
picker state, so `/theme` cannot obtain preview/restore/persistence semantics by
registering an ordinary selection command unchanged.

## Hard gate

- [x] Probe written and run against the real codebase.
- [x] Material-boundary inventory completed.
- [ ] Every material boundary probed through an observable seam or excluded.
- [x] Independent oracles defined and run for the observations above.
- [x] Probe and oracle agree on all observations made, including non-trivial
  compiled projection.
- [x] New learning recorded.

Per the skill, no design or implementation plan may proceed. Re-run the probe
after the open blockers land and the branch contains the multi-theme/modal/chrome
substrate; then add real-seam observations for live mutation, next-frame preview,
nested Esc restore, missing-file creation, and write failure.
