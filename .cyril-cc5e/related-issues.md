# cyril-cc5e — related issues (prior-art pass, 2026-07-14)

Tracker searched for `picker|modal|overlay|popup|centered|scroll` across all
issues. **No prior ticket describes the picker selection-visibility defect
itself** — cc5e is first contact. Related context:

## Dependents (consume what cc5e establishes)

- `cyril-a14l` (P1) — 60×16 usability floor; cc5e's popup geometry is its
  first proof point.
- `cyril-nrnq` (P2) — modal semantic-color migration; will restyle the same
  widget, so keep colors OUT of cc5e's scope (hardcoded palette stays as-is).
- `cyril-qaq0`, `cyril-91iu`, `cyril-uw20` — downstream usability DAG.

## Adjacent, not blocking

- `cyril-mdbp` (closed) — status-line overflow clipping on narrow terminals;
  different surface (toolbar), same 60×16 pressure. Fix pattern was
  truncation, not scrolling — not reusable here.
- `cyril-lxuo` (open, P3) — wants richer model-picker rows (capability badge
  + credit hint) → row height will grow further; the cc5e viewport must not
  assume uniform 1-line rows.
- `cyril-8r3u` (open, P2) — usability capstone; will lock cc5e's rendering
  behind visual-regression snapshots later. cc5e ships focused render tests
  only.

## Non-overlap guard

`origin/cyril-a71q` is in flight (bridge turn-seq dedup) — zero file overlap
with picker/render work. `.rivets/issues.jsonl` stays off this branch.
