# Related issues — cyril-qaq0 (Activate bundled themes and terminal color modes)

Harvested 2026-07-11 by interrogated-spec (audit mode).

## Declared blockers (from the ticket)
- cyril-cc5e — Keep picker selection visible (open, P1): /theme reuses the picker overlay.
- cyril-ghuu — Migrate conversation surfaces to semantic colors (CLOSED ✓): supplies the 29-role contract, 4 color-mode projections, and the mode-matrix fence.
- cyril-nrnq — Migrate modal surfaces to semantic colors (open): picker overlay itself must be themed before /theme previews through it.
- cyril-dij8 — Migrate application chrome to semantic colors (open).
- cyril-fkke — Add the remaining bundled theme palettes (open): without it the /theme picker has one entry.
- cyril-6r3a — Remove legacy palette access after semantic migration (open, listed in Dependencies).

## Undeclared but intersecting (created AFTER this ticket, from the 2026-07-11 code review)
- cyril-q9dx (P1) — Preserve speaker identity in ANSI-16 projection: the ANSI-16 mode this ticket ACTIVATES currently renders user+system as identical Gray and agent as the muted DarkGray. Triple-fenced as "correct" by the projection oracle.
- cyril-leiq (P1) — Restore readable Cyril Dark semantic-role contrast: the default theme's links/statuses are near-invisible dim VGA on dark terminals.
- cyril-nd4h (P1) — Remove or honor ineffective UI configuration fields: this ticket adds startup configuration to the same [ui] surface that currently carries dead, schema-pinned knobs.
- cyril-x5xi (P2) — Make theme-dependent cache identities structurally complete: /theme preview is the first production path that swaps resolved themes live against the theme-keyed render caches.

## Decision sources
- docs/adr/0005-semantic-themes-and-color-modes.md (accepted 2026-07-10) — two-axis model; project once; read-only from UI state; no arbitrary user palettes.
- .cyril-ghuu/spec.md — 29-role contract, decision #4 (truecolor-only until configuration activates), edge rows for theme-keyed caches and no-color completeness.
- docs/reviews/2026-07-11-conversation-theme-code-review.md — findings #1/#6 (what the mode-matrix fence cannot see).
