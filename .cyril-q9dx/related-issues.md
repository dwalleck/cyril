# Related issues — cyril-q9dx (Preserve speaker identity in ANSI-16 projection)

Harvested 2026-07-11 by interrogated-spec (audit mode). The bounded
tracker search covered exact `speaker identity`, `ANSI-16 projection`, and
`semantic projection` terms in Rivets and GitHub Issues; GitHub returned no
matches.

## Direct lineage

- **cyril-ghuu** (closed, P2) — Migrate conversation surfaces to semantic
  colors. `cyril-q9dx` is recorded as `discovered-from` this ticket. Its signed
  nearest-RGB ANSI-16 contract and oracle currently certify the identity
  collapse.
- **cyril-ixua** (closed, P2) — Expand the semantic theme seam. Established
  unconstrained nearest-RGB projection for all ANSI-16 roles; `cyril-q9dx`
  intentionally supersedes that rule for speaker-identity roles.
- **cyril-qaq0** (open, P2) — Activate bundled themes and terminal color modes.
  Rivets records `cyril-q9dx` as a blocker because `qaq0` makes ANSI-16
  production-selectable.

## Intersecting work

- **cyril-fkke** (open, P2) — Add the remaining bundled theme palettes. Its
  palettes will also need any role-level ANSI-16 identity invariant if the
  constraint is theme-wide rather than Cyril-Dark-only.
- **cyril-leiq** (open, P1) — Restore readable Cyril Dark semantic-role
  contrast. Adjacent real-terminal perceptual risk, but separate from
  structural speaker identity.
- **cyril-x5xi** (open, P2) — Make theme-dependent cache identities
  structurally complete. Projection changes participate in rendered-theme
  cache identities, but cache structure is separate work.
- **cyril-xv3e** (open, P3) — Consolidate conversation-theme fixture test
  plumbing. It names projection-table authority as cleanup scope; `q9dx` owns
  the semantic rule, not that mechanical consolidation.

## Decision sources

- `docs/reviews/2026-07-11-conversation-theme-code-review.md`, finding 6 —
  verified current user/system and agent/muted collisions and the triple-fenced
  false contract.
- `docs/reviews/2026-07-11-conversation-theme-review-decisions.md`, finding 6 —
  chooses semantic projection constraints rather than true-color retuning.
- `docs/adr/0005-semantic-themes-and-color-modes.md` — separate theme/mode axes,
  project once, expose the resolved theme read-only, and preserve status meaning
  when color is limited.
- `.cyril-ixua/spec.md` — original canonical ANSI-16 table, lower-index tie
  break, deterministic projection, and role separation.
- `.cyril-ghuu/spec.md` — expanded 29-role conversation contract, current
  nearest-palette criterion, fixed 4-mode scene fence, cache behavior, and scope
  boundaries.
- `.cyril-qaq0/spec.md` — explicitly records `cyril-q9dx` as an ANSI-16
  activation blocker and names the mode-matrix proxy blind spot.
- `.rivets/issues.jsonl`, record `cyril-q9dx` — ticket behavior and acceptance
  criteria.
