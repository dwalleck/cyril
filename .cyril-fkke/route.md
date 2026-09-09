# Route: cyril-fkke

Change: Add five bundled palettes before cyril-qaq0 activation.
Date: 2026-09-09

## Route tests

| # | Test | Evidence | Verdict |
|---|---|---|---|
| 1 | Empirical premise | Existing source defines pure RGB projections, fixed ANSI-16 speaker slots, Syntect name lookup and state-owned rendering. No real-terminal equivalence or unverified terminal capability guarantee is assumed. Palette readability must be measured on explicitly specified backgrounds; scope questions are policy decisions, not unknown external behavior. | no |
| 2 | Structural module shape | Public ThemeId and SyntaxTheme enums in crates/cyril-ui/src/theme.rs expand. Existing theme module owns source values and projection; highlight.rs owns syntax loading; render.rs owns frame painting. SourceTheme already requires all 31 roles. Protected parents: App, UiState, main, configuration and command routing receive no new responsibility. Palette data remains behind resolve(ThemeId, ColorMode); no vendor or per-palette widget branches. | yes |
| 3 | Production-scale risk | Five constant palettes added to one existing palette; fixed 31-role resolution and existing bounded caches. No new unbounded work or concurrency. | no |
| 4 | Explicit behavior | Five names, deterministic projection, mandatory role/syntax population and no selection/configuration are explicit in the ticket. ADR 0007 requires per-tier contrast but specifies dark backgrounds only; light and high-contrast targets, intended canvas presentation and compatible branded syntax choices are not pinned. | no |

Unknown tests: none.

## Selected route

Structural — public enum expansion and unresolved observable palette/readability policy.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — pin readability and presentation policy |
| evidence.md, probe.* | prove-it-prototype | N/A — no external premise assumed |
| design.md | falsifiable-design | required after specification approval |
| plan.md | budgeted-plan | required after design approval |

Oracle checkpoint in checkpointed-build: required — Structural route.

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate. Not satisfied; specification decisions remain outstanding.

## Evidence

- Worktree /home/dwalleck/repos/cyril-wt-feat-cyril-fkke, branch feat/cyril-fkke, base 7fe7f5ed.
- rivets show cyril-fkke --json, 2026-09-09; claimed in primary and feature trackers.
- Source investigation agent://PaletteEvidence, 2026-09-09. No prior fkke artifact directory or approved RGB tables found.
- docs/adr/0005-semantic-themes-and-color-modes.md: accepted theme/mode separation.
- docs/adr/0007-cyril-dark-contrast-contract.md: accepted 4.5 primary / 3.0 muted tiers and dark signal exception; additional themes must extend the contract.
- theme.rs:31–35 and 52–63: current theme/syntax enums; 66–100: mandatory 31-role source; 411–455: protected speaker semantics and no-color.
- Existing Cyril Dark rendering must remain unchanged. cyril-qaq0 owns later activation; cyril-nx1q covers wider purpose-built semantic role binding; cyril-x5xi covers structural cache identity work. None is silently folded into fkke.
