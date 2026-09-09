Closes cyril-fkke.

## What this adds

Five bundled palettes beside Cyril Dark — **Cyril Light**, **High Contrast Dark**, **High Contrast Light**, **Catppuccin Mocha**, **Gruvbox Dark** — plus the canvas painting they need to actually render. Selection and configuration stay out of scope (cyril-qaq0).

## Design records

| Artifact | Path |
|---|---|
| Route | `.cyril-fkke/route.md` |
| Spec | `.cyril-fkke/spec.md` |
| Design + claims + falsification table | `.cyril-fkke/design.md` |
| Plan + nine-gate checkpoint records | `.cyril-fkke/plan.md` |
| Independent oracle | `.cyril-fkke/palette-oracle.py`, `.cyril-fkke/oracles/compare_probe.py` |
| Module-shape fence | `.cyril-fkke/oracles/module_shape.py` |
| Mutation proofs | `.cyril-fkke/oracles/mutations.sh` |

## Claims and evidence

| Claim | Evidence |
|---|---|
| C1 every palette populates all 31 roles | `compare_probe.py` → `PASS 186 rows agree with the independent oracle` |
| C2 WCAG tiers met per palette | `palette-oracle.py` → `PASS 6 palettes satisfy contrast and ANSI-16 constraints`; Rust fence recomputes independently |
| C3 ANSI-16 muted roles stay out of protected speaker slots | mutation M2 red, restored green |
| C4 projection deterministic and correct in all four modes | `compare_probe.py` over 6 palettes × 4 modes |
| C5 every syntax component exists in Syntect's bundled set | `all_bundled_syntax_themes_exist`; mutation M3 red |
| C6 explicit canvas painted; `Reset` unchanged | `light_palette_paints_canvas_background`, `reset_canvas_leaves_terminal_background_untouched`; mutation M5 red |
| C7 cache keys discriminate palettes | highlight + markdown fences; mutations M4/M7 red |
| C8 no protected-parent or widget responsibility | `module_shape.py` → `PASS`; mutation M6 red |

Full mutation run: `bash .cyril-fkke/oracles/mutations.sh` → `PASS all named mutations red, all fences green after restore`.

## Readability policy

ADR 0007 (WCAG AA) extended to AAA for the high-contrast variants, approved by the requester on 2026-09-09 (recorded in `.cyril-fkke/design.md` §Approval): ordinary palettes PRIMARY ≥ 4.5:1, MUTED ≥ 3.0:1, SATURATED ≥ 4.5:1; high-contrast 7.0 / 4.5 / 7.0 / 4.5. Branded palettes use upstream colors except where a tier required an adjustment. Syntect ships no Catppuccin or Gruvbox component, so those syntax pairings are documented approximations.

## Review notes

- **Snapshots changed intentionally.** The three `roomy_*_80x24` floor snapshots use the marker theme, which declares an explicit canvas. Verified as 88 changed lines with **zero** differences outside the `bg` field; the frozen Cyril Dark baseline is untouched.
- **Design claim C7 was amended.** Its first fence falsified it: no-color resolves identically for every palette, so one shared cache key is correct rather than stale.
- **C6's mutation was corrected.** Painting unconditionally is unobservable (`Reset` projects to `Reset`), so the named mutation is now "remove the paint entirely".
- **Independent design-conformance review** (read-only reviewer, no access to the design or plan) reconstructed the module shape and agreed on every verdict: registry types confined to `theme.rs`, no `render.rs` branching on theme identity, no widget palette access, no `app.rs`/`main.rs`/`state.rs` theme responsibility, no pass-through module.
