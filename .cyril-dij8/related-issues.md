# cyril-dij8 — prior art (tracker sweep 2026-07-14)

Search: `rivets list -n 200 | grep -iE 'theme|semantic|color|palette|chrome|toolbar|status bar'`

## Directly load-bearing

- **cyril-ixua** (closed) — built the semantic theme seam. Contract is now
  **31 roles** (theme.rs `EXPECTED_ROLES`), grown from ixua's 19 by ghuu
  (+10) and nrnq (+2: `text_secondary`, `accent_violet`). Artifacts:
  `.cyril-ixua/`.
- **cyril-ghuu** (closed) — conversation-surface migration. Established the
  method: TestBackend style-tuple probe + pre-transcribed source-scan
  oracle + NAMED legacy→canonical mapping table
  (`.cyril-ghuu/cheapest-falsifier.py`). Established the widget seam:
  `render(..., theme: &Theme)` parameter, resolved once in `render.rs`
  via `state.theme()`.
- **cyril-nrnq** (closed, PR #53) — modal-surface migration. Same method;
  artifacts at `.cyril-nrnq/` are the direct template for this issue.
  Left fences in theme.rs this issue must extend
  (`modal_widgets_have_no_legacy_color_sources`,
  `widgets_only_use_the_explicit_theme`).
- **cyril-leiq** (OPEN, P1) — "Restore readable Cyril Dark semantic-role
  contrast": the canonical VGA values (#808000 etc.) chosen for
  named-ANSI replacements are dim/unreadable on dark terminals. Filed
  from ghuu review. **Role-VALUE problem, orthogonal to this issue's
  role-ASSIGNMENT problem** (nrnq's findings recorded the same posture).
  Mapping chrome onto the same canonical roles keeps equivalence intact
  and lets leiq re-value once, centrally, without re-touching chrome.

## Adjacent / downstream (do not solve here)

- **cyril-6r3a** (open, blocked by dij8) — removes legacy `palette`
  color constants after all batches migrate. After dij8, `palette`'s
  color constants have zero production consumers (only
  `SPINNER_CHARS`/`SPINNER_FRAME_MS`/`MAX_BORDER_WIDTH` remain used) —
  that contraction is 6r3a's job, not this issue's.
- **cyril-qaq0, cyril-fkke, cyril-a14l, cyril-9ode, cyril-lme2** (open,
  blocked by dij8) — theme activation, bundled palettes, and follow-on
  UI work; unblocked by this issue, not touched by it.
- **cyril-nx1q** (open) — purpose-built conversation roles (rename/re-bind
  pass); may eventually re-bind chrome roles too, but explicitly a later
  re-mapping pass.
- **cyril-xv3e** (open, P3) — conversation-theme fixture plumbing
  consolidation; if chrome tests add similar fixtures, note but don't
  merge scopes.
- **cyril-xi4a** (closed) — theme-source scanner CRLF fix; the
  `conversation_theme_sources.rs` scanner this issue may extend is
  CRLF-hardened already.

No open issue describes the chrome migration itself other than cyril-dij8.
