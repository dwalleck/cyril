# cyril-leiq — related issues (prove-it-prototype step 0)

Tracker swept 2026-07-24 (keywords: contrast, palette, cyril dark, vga, wcag,
readable, theme, semantic, ansi).

## Origin

- **cyril-ghuu** (closed) — "Migrate conversation surfaces to semantic colors."
  The migration that replaced terminal-resolved named ANSI colors with the dim
  VGA RGB literals now in `cyril_dark_source`. cyril-leiq is `discovered-from`
  its review (finding #1).

## Scope boundaries (siblings — cyril-leiq must NOT absorb these)

- **cyril-nx1q** (open) — role *binding* (finding #15: conversation renderers
  use generic `subdued` roles, semantic fields visually dead). That is *which
  role maps to which surface*. cyril-leiq fixes the **color VALUES** for
  contrast, NOT the binding. Leave role→surface remapping to nx1q.
- **cyril-8r3u** (open, P2) — the broad usability milestone: full-screen
  snapshots + docs + every bundled theme/color-mode projection. Blocked by
  qaq0/a14l/lme2/9ode/uw20/91iu/4vvw. cyril-leiq ships a **narrow contrast
  regression fence** (its AC3), not the broad snapshot matrix — that's 8r3u.
- **cyril-q9dx** (closed) — ANSI-16 projection identity. cyril-leiq targets the
  **truecolor** values (the active render mode); the ansi16/256 quantization is
  a separate projection path.
- **cyril-qaq0 / cyril-fkke** (open) — activating more themes / adding palettes.
  Out of scope; cyril-leiq only touches the one shipped `CyrilDark` palette.

## Key facts from the code (pre-probe)

- Links render with `theme.accent_tertiary` = `Rgb(0x00,0x00,0x80)` (navy) +
  underline (`widgets/markdown.rs` `Tag::Link`). Navy on a dark terminal is the
  headline "unreadable links."
- The dim offenders are literally the **VGA 16-color dark half** used as
  truecolor RGB: `accent_tertiary 0x000080`, `accent_quaternary 0x800080`,
  `accent_quinary 0x008080`, `subdued 0x808080`, `subdued_positive 0x008000`,
  `subdued_negative 0x800000`, `emphasis 0x808000` (all match `ANSI16_RGB`).
- Active render mode is `ColorMode::TrueColor` (`render.rs`), so these RGBs
  render as-is; `canvas` is `Reset` (the terminal's own dark background).

No existing ticket measures or fixes the contrast values themselves — cyril-leiq
is not a duplicate.
