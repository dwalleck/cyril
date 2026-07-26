# Cyril Dark uses fixed brightened RGB with per-tier contrast targets

Status: accepted (2026-07-24, cyril-leiq)

## Context

The conversation-theme migration (cyril-ghuu) set several Cyril Dark roles to
the VGA 16-color dark half as truecolor RGB. Measured with WCAG 2.x contrast
(anchor: white-on-black = 21.0), against representative dark backgrounds
`#000000` and the theme's chrome `#1e1e2e`, those values were unreadable:
markdown links (`accent_tertiary #000080`) sat at **1.31:1 on black, 1.02:1 on
chrome** — barely above the 1.0 invisibility floor. `subdued_negative #800000`
(1.50), `accent_quaternary #800080` (1.74), `accent_quinary #008080` (3.44),
and `emphasis #808000` (3.91) also missed the readable range. The failure is
tiered, and pure `#ff0000` danger is a hue-luminance floor (~5:1 max on black),
not a dim-VGA mistake. Evidence: `.cyril-leiq/` (probe + oracle).

## Decision

- **Fixed RGB, not terminal-palette remapping.** Cyril Dark stays a fixed
  brightened RGB palette. Reverting the dim roles to terminal-named ANSI colors
  would reintroduce the terminal-dependence the semantic-theme model (ADR-0005)
  deliberately removed, and the ANSI16/256 projections are a separate, derived
  path (cyril-q9dx owns that identity). The brightened truecolor values project
  into the ansi modes exactly as any other RGB does.
- **Per-tier contrast targets**, measured against BOTH `#000000` and `#1e1e2e`
  (chrome is the tighter of the two, so meeting it implies any darker bg):
  - **PRIMARY** (readable emphasis, links, semantic accents): **≥ 4.5:1**
    (WCAG AA normal text).
  - **MUTED** (intentionally de-emphasized: `subdued*`, `muted`, `border`,
    `diff_context`, `text_secondary`): **≥ 3.0:1** (WCAG AA large / UI floor).
  - **SATURATED** standard signal hues (`danger`/`diff_delete` red,
    `success`/`diff_add` green, `warning`): keep the standard hue and clear
    **≥ 3.0:1 on chrome / ≥ 4.5:1 on black** — brightening them would desaturate
    the signal.
- **Values changed (fixed RGB, hue preserved):** `accent_tertiary #6cb6ff`
  (link), `accent_quaternary #cd9ee6`, `accent_quinary #56c7d0`,
  `subdued_negative #d98a8a`, `emphasis #d7ba7d`. The other 21 conversation
  foreground roles already met their tier and are unchanged.
- **Enforced, not aspirational:** `theme::tests::cyril_dark_contrast_contract`
  computes WCAG contrast per role in CI and fails if any role misses its tier;
  `cyril_dark_hue_identity` fails on a hue swap. Both run under `cargo test`.

## Consequences

- Chrome and modal surfaces share these roles, so they inherit the improved
  contrast; their frozen baselines (`chrome-theme-baseline.tsv`,
  `modal-baseline.tsv`) and the conversation baseline were regenerated as a
  conscious review (cyril-nx1q AC), each diff verified foreground-color-only.
- The three `*_legacy_colors_are_representable` tests no longer require the five
  superseded dim VGA colors; the remaining legacy colors are still preserved.
- Adding a role or a bundled theme must satisfy the same contract test
  (extend the tier table). Rejected alternative: a flat 4.5:1 for every role —
  it would over-brighten deliberately-muted roles and desaturate semantic red.
