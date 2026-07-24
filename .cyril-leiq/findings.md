# cyril-leiq — prove-it-prototype findings

**Headline: the failure is measured and tiered, not uniform.** Markdown links
render in `accent_tertiary #000080` (navy) at **1.31:1** against a black
terminal — barely above the 1.0 invisibility floor. But the 9 failing
conversation foreground roles split into two tiers, and one apparent "failure"
(#ff0000 danger) is a hue-luminance floor, not a dim-VGA mistake. A blanket
"raise everything to 4.5:1" would be wrong.

## Smallest question

Which Cyril Dark conversation FOREGROUND roles fall below the WCAG AA text
target (4.5:1) on a dark terminal, and by how much?

## Probe (`probe_contrast.py`) — regex-parse the theme literals + WCAG contrast

Independent of the theme-resolution code (reads the `cyril_dark_source` RGB
literals directly). Anchor: white-on-black computes to **21.00** (WCAG max),
so the formula is correct. Contrast of each foreground role vs `#000000`
(black, worst case) and `#1e1e2e` (chrome):

**Tier 1 — hard-unreadable (< 3:1 on black):**
| role | rgb | vs black | note |
|---|---|---|---|
| `accent_tertiary` | `#000080` | **1.31** | the LINK color — the headline bug |
| `subdued_negative` | `#800000` | **1.92** | |
| `accent_quaternary` | `#800080` | **2.23** | |

**Tier 2 — below AA text but ≥ 3:1 (dim; legible only at large/secondary):**
`accent_quinary #008080` 4.40, `subdued #808080` 5.32/chrome 4.15,
`subdued_positive #008000` 4.09, `emphasis #808000` 5.01/chrome 3.91.

**Not a dim-VGA bug — a hue floor:** `danger`/`diff_delete #ff0000` = 5.25 on
black, 4.10 on chrome. Pure red's luminance coefficient (0.2126) caps it near
~5:1; it is the standard semantic red, not a migration mistake. The design
must NOT treat it as broken (raising it would desaturate the danger signal).

Passing roles (all ≥ 4.5 on black): text, user, agent, accent, info, success,
warning, soft_accent, positive_accent, accent_alt, accent_violet,
text_secondary, muted, border, diff_add — 15 of 26.

## Oracle — two independent axes, both agree

1. **Compiled == source** (`oracle-compiled-roles.txt`): a scratch integration
   test read the COMPILED `resolve_truecolor(CyrilDark)` Theme (a different
   mechanism than the probe's regex) and dumped each role's `Color::Rgb`.
   All 9 spot roles MATCH the probe's parsed literals — the renderer uses
   exactly these RGBs (truecolor projection is identity), so the probe is not
   measuring a fiction.
2. **Formula anchor + hand check**: white/black = 21.00 (exact WCAG max), and a
   by-hand computation of navy `#000080` on black — L_B = 0.0722·((0.502+0.055)/1.055)^2.4 = 0.0156, contrast = (0.0156+0.05)/0.05 = **1.31** — matches the probe to two decimals.

## What I learned that I didn't know before

The dim roles are literally the **VGA 16-color dark half** used as truecolor
RGB (each matches `ANSI16_RGB`), and the damage is **tiered**: links at 1.31:1
are effectively invisible while others merely miss AA-text at 3–4.5. Crucially,
pure `#ff0000` danger is a **hue-luminance floor** (~5:1 max on black), so the
contrast target cannot be a flat 4.5 across all roles — muted/secondary roles
and saturated semantic hues need per-tier targets. That tiering is the design
decision cyril-leiq's AC calls out.

## Feeds the design

- The AC decision "fixed RGB vs terminal-palette remapping" now has data: the
  active mode is `TrueColor`, so fixed RGB values are what render; the fix is
  brightening the tier-1/tier-2 RGBs to hit per-tier targets (NOT remapping to
  terminal-named colors, which reintroduces the terminal-dependence the
  migration removed — cyril-q9dx owns the ANSI projection).
- AC3 regression fence: a contrast test over the failing roles (the scratch
  oracle is its seed) — must fail on today's dim VGA values, pass after.
