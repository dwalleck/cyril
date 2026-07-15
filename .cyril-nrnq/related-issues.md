# cyril-nrnq — related issues (prior-art pass, 2026-07-14)

- `cyril-ixua` (closed) — defined the semantic seam: 29 roles, 4 projections.
  Contract spec at `.cyril-ixua/spec.md` (its "Cyril Dark compatibility
  mapping" section is one of the two canonical-value sources ghuu's falsifier
  reads).
- `cyril-ghuu` (closed) — conversation-surface migration; THE method to
  replicate: frozen legacy inventory → canonical ANSI RGB → representability
  claim → zero-normalized-diff equivalence fences (`.cyril-ghuu/design.md`,
  `cheapest-falsifier.py`, `legacy-color-baseline.tsv`). Its design
  explicitly excludes modal surfaces, naming cyril-nrnq as the owner.
- `cyril-leiq` (open, P1) — dim VGA role values unreadable on dark
  terminals; discovered-from ghuu. Role-VALUE bug, orthogonal to nrnq's
  role-assignment: canonical mapping here neither fixes nor worsens it, and
  a later leiq re-valuation flows through to modals automatically once they
  consume roles.
- `cyril-dij8` (open, sibling) — chrome batch (toolbar, status bar, crew,
  voice). Non-overlapping files; both feed cyril-6r3a.
- `cyril-6r3a` (open) — remove legacy palette access after all batches;
  blocked by nrnq + dij8.
- `cyril-a14l` / `cyril-uw20` (open) — own modal GEOMETRY (60×16 floor,
  hooks responsiveness, modal::centered adoption). nrnq deliberately does
  not touch layout.
- `cyril-x5xi` (open) — theme-identity cache completeness; modals are
  uncached render paths, so no cache-key changes belong in nrnq.

No prior ticket covers modal color migration itself — nrnq is first contact.
