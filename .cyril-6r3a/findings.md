# cyril-6r3a — prove-it-prototype findings (2026-07-15)

## Smallest question

> Which code (production vs test, per file) consumes each of the 8 items
> in `cyril-ui/src/palette.rs` today?

## Probe

`probe_6r3a.py` — text scan of every `.rs` file under `crates/` for the 8
item names + `use ...palette` imports, classified production/test by
position relative to the file's first `#[cfg(test)]`.

## Oracle

The COMPILER (different mechanism: name resolution vs text matching):
gut `palette.rs` to a doc comment, run `cargo check --all-targets`,
collect unresolved-name errors; restore. The error set enumerates every
real consumer, immune to grep blind spots (comments, string data,
aliased imports).

## Agreement (after one investigated disagreement)

Both mechanisms agree on the consumer set:

| Item | Real consumers |
|---|---|
| `SPINNER_CHARS` | toolbar.rs prod (render ×3 + `spinner_index`), chrome_theme_tests.rs (×1, cfg(test) via lib.rs) |
| `SPINNER_FRAME_MS` | toolbar.rs prod (`spinner_index`) |
| `USER_BLUE`, `AGENT_GREEN`, `SYSTEM_MAUVE`, `MUTED_GRAY`, `CODE_BLOCK_BG`, `MAX_BORDER_WIDTH` | **ZERO consumers — dead code** |

The apparent disagreement (oracle errors pointing at chat.rs:8-9, which
the probe never matched) was rustc SUGGESTION NOTES, not consumer errors:
"constant `crate::widgets::chat::SPINNER_CHARS` exists but is
inaccessible" — which surfaced the real discovery below. Probe artifacts
noted for the record: the theme.rs "hits" are comments/string-data inside
the dij8 fences (`chrome_legacy_colors_are_representable` comments,
`chrome_widgets_have_no_legacy_color_sources` scan list), not consumers;
the naive per-line item attribution undercounts `SPINNER_FRAME_MS` when
it shares a line with `SPINNER_CHARS`.

## What I learned (that I didn't know before)

**`chat.rs` privately duplicates the spinner constants** (`const
SPINNER_CHARS`/`SPINNER_FRAME_MS` at chat.rs:8-9, byte-identical values
to palette's) — the ghuu migration inlined rather than imported them, so
the contraction is not just "delete dead items + relocate two constants":
there are TWO spinner-constant copies and a design decision about where
the single copy lives.

Also material:
1. All six color/layout items are already dead — deleting them changes
   zero behavior by construction (the compiler proves it).
2. The AC3 fence shape already exists: `conversation_theme_sources.rs`
   scans 5 conversation modules for `palette::`/`crate::palette`
   (PaletteAccess) and `Color::` (HardCodedColor); extending coverage to
   all widget modules is the natural fence implementation.
3. `pub mod palette;` in lib.rs is the only module-level surface; nothing
   outside `cyril-ui` references it (cyril binary: zero hits).

## Hard gate

- [x] Probe written, runs against the real codebase
- [x] Oracle defined, produces output (rustc unresolved-name enumeration)
- [x] Probe and oracle agree (consumer set identical; disagreement investigated → suggestion notes, and it yielded the key discovery)
- [x] One-sentence learning recorded (above)
