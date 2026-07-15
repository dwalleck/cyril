# cyril-6r3a — prior art (tracker sweep 2026-07-15)

- **cyril-ghuu / cyril-nrnq / cyril-dij8** (all closed) — the three
  consumer-migration batches this contraction was gated on. dij8's audit
  (.cyril-dij8/build-audit.md) already predicted: "After dij8, `palette`'s
  four color constants have zero production consumers."
- **cyril-qaq0** (open, blocked by 6r3a) — theme activation; consumes this
  issue's guarantee that the resolved `Theme` is the single color source.
- **cyril-fkke** (open) — bundled theme palettes: "palette" in the THEME
  sense (new `SourceTheme` value sets), unrelated to the legacy
  `palette.rs` module removed here.
- **cyril-xv3e** (open, P3) — conversation-theme fixture plumbing
  consolidation; the existing `conversation_theme_sources.rs` scanner
  (which this issue's AC3 fence extends or generalizes) is in its orbit.
- **cyril-leiq** (open, P1) — role VALUES readability; unaffected by
  contraction (no values change).
- **cyril-xi4a** (closed) — the source scanner is CRLF-hardened; keep that
  property when extending it.

No open issue covers the chat.rs spinner-constant duplication discovered
by this probe (searched: spinner, duplicate, chat constants) — scoped into
this issue's design (option) or filed at close-out if deferred.
