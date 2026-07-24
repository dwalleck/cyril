# cyril-leiq — falsifiable design: readable Cyril Dark contrast

## Purpose

The conversation-theme migration (cyril-ghuu) set Cyril Dark's dim roles to the
VGA 16-color dark half as truecolor RGB. Measured (probe `probe_contrast.py`,
anchor white/black=21.00): markdown links (`accent_tertiary #000080`) are
**1.31:1 on black, 1.02:1 on chrome** — invisible. Fix the color VALUES to meet
explicit per-tier contrast targets, preserving hue identity, without rebinding
roles (cyril-nx1q) or touching the ANSI projections (cyril-q9dx).

## The core move is a VALUE change (additive to behavior, invalidating to baseline tests)

Not subtractive in the concurrency sense — no lock/ordering/uniqueness removed.
But it changes the **Cyril Dark baseline**, so any test that pins the old RGB
literals or renders a snapshot of a changed role WILL break by design (that is
the fix, per AC3). Impact analysis (plan slice 0): grep tests/snapshots for the
old hex literals and the changed role names; update them in the same change.
cyril-nx1q's AC "Cyril Dark baseline changes are consciously reviewed" is
satisfied by this PR being that conscious review.

## Decisions (flagged for the design pause)

- **FD-1 — per-tier targets** (recommended): PRIMARY (readable emphasis, links)
  **≥ 4.5:1** (WCAG AA text); MUTED (intentionally de-emphasized: `subdued*`,
  `muted`, `diff_context`, `text_secondary`, `border`) **≥ 3.0:1** (AA large /
  UI floor); SEMANTIC-SATURATED (standard signal hues: `danger`/`diff_delete`
  red, `success`/`diff_add` green, `warning` yellow) keep the standard hue and
  clear **≥ 3.0:1 vs chrome / ≥ 4.5:1 vs black**. Alternative: flat 4.5 for all
  — rejected, it would desaturate semantic red and over-brighten deliberately
  muted roles.
- **FD-2 — the exact new values** (taste; the CONTRACT is what's approved, hexes
  are adjustable so long as they pass their tier): `accent_tertiary #6cb6ff`
  (link), `accent_quaternary #cd9ee6`, `accent_quinary #56c7d0`,
  `subdued_negative #d98a8a`, `emphasis #d7ba7d`. The other 21 foreground roles
  are UNCHANGED (they already clear their tier).
- **FD-3 — reference background**: bind the contract to **#1e1e2e (chrome)** —
  the tighter of the representative dark backgrounds (a role passing chrome
  passes any darker bg incl. #000000) — and also report #000000. Alternative:
  #000000 only (laxer; a role could pass black yet be dim on the chrome UI).
- **FD-4 — fixed RGB, NOT terminal-palette remapping** (AC2 decision): the
  migration deliberately left terminal-named colors (they vary per terminal and
  caused the KAS ANSI issues); remapping reintroduces that terminal-dependence.
  The ANSI16/256 projections are a separate path (cyril-q9dx). Documented in
  the theme module + the issue's AC2.

## Input shapes

- **Role tiers** (the 26 conversation foreground roles):
  - PRIMARY currently failing: `accent_tertiary` (1.02 chrome), `accent_quaternary`
    (1.74), `accent_quinary` (3.44), `emphasis` (3.91).
  - PRIMARY currently passing (must stay ≥4.5, unchanged): text, user, agent,
    accent, info, soft_accent, positive_accent, accent_alt, accent_violet.
  - MUTED currently failing: `subdued_negative` (1.50).
  - MUTED currently passing (≥3.0, unchanged): subdued (4.15), subdued_positive
    (3.19), muted, diff_context, text_secondary, border.
  - SEMANTIC-SATURATED (keep hue): danger/diff_delete (#ff0000, 4.10 chrome),
    success/diff_add (#00ff00), warning (#ffff00).
- **Color mode**: TrueColor (IN SCOPE — values render as-is). Ansi256/Ansi16/None
  = OUT OF SCOPE (projection paths; ansi16 identity is cyril-q9dx).
- **Background**: #000000 (black canvas/terminal), #1e1e2e (chrome, binding).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | Every PRIMARY conversation role ≥ 4.5:1 vs #1e1e2e (and #000000) | compute WCAG contrast of each primary role's value | `falsifier_proposed.py` (independent WCAG, anchor-checked) + Rust fence | 5m | **passed** | `theme::tests::cyril_dark_contrast_contract` |
| 2 | Every MUTED conversation role ≥ 3.0:1 vs #1e1e2e | contrast of each muted role | same | 5m | **passed** | same fence |
| 3 | SEMANTIC-SATURATED roles keep standard hue AND ≥3.0 vs chrome / ≥4.5 vs black | contrast + dominant-channel of danger/success/warning | same | 5m | **passed** | same fence |
| 4 | Link role `accent_tertiary` ≥ 4.5:1 (headline; was 1.02) | contrast of the link value | same | 2m | **passed** (7.63 chrome) | same fence (explicit link assert) |
| 5 | Each CHANGED role preserves its hue family (no hue swap) | dominant-channel check per changed role | `falsifier_proposed.py` hue check | 2m | **passed** | `theme::tests::cyril_dark_hue_identity` |
| 6 | Roles that already passed are UNCHANGED (no gratuitous churn) | diff changed-role set vs the failing-role set from the probe | git diff of `cyril_dark_source` literals | 3m | pending | `theme::tests::cyril_dark_unchanged_roles_stable` (asserts specific passing roles' exact RGB) |
| 7 | The fixed-RGB (not remap) decision + tier targets are documented in-repo | grep the theme module / docs | `grep` | 2m | pending | `cyril_dark_contrast_contract` doc-comment references the decision |
| 8 | Only Cyril Dark truecolor literals change; ANSI/None projection code untouched | inspect the diff's file/function scope | git diff scope | 2m | pending | n/a (scope discipline; verified in review) |

Cheapest falsifier (claims 1-5) run via `falsifier_proposed.py`: **all PASS**,
hue identity preserved. Claim 4 (link) is `passed` — gate satisfied.

## Regression fence

`theme::tests::cyril_dark_contrast_contract` (new, in `theme.rs` or
`tests/`): builds `resolve_truecolor(CyrilDark)`, computes WCAG contrast per
role in Rust (independent of the Python probe), and asserts each meets its tier
target vs BOTH #1e1e2e and #000000. It embeds the bug class: on today's
`#000080` link it FAILS (1.02 < 4.5); on the fixed value it PASSES. Anchored by
an in-test white/black == 21.0 assertion so a broken formula can't make it
vacuously pass. Rides `cargo test` (already in CI) — no CI-job change (contrast
is deterministic, unlike the pixel snapshots cyril-8r3u will add).

## Negative space (what this deliberately does NOT do)

1. Does NOT rebind which role each conversation surface uses — role→surface
   mapping is **cyril-nx1q**.
2. Does NOT change the ANSI16/256/None projections or `nearest_ansi16/256` —
   ansi-projection identity is **cyril-q9dx** (closed) / the projection paths.
3. Does NOT add the broad full-screen snapshot matrix or user docs — the
   usability-milestone lock is **cyril-8r3u**.
4. Does NOT add new themes, palettes, or color-mode activation — **cyril-qaq0**
   / **cyril-fkke**.
5. Does NOT touch syntax-highlight (syntect) colors — **cyril-d43s**.

## Tracker note

All five negative-space boundaries cite existing issues (nx1q, q9dx, 8r3u,
qaq0, fkke, d43s — all verified present in the tracker this session). No new
deferral filed; nothing in this design is deferred work without a home.
