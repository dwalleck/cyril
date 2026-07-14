# Feature: Preserve ANSI-16 speaker identity

## What this is

Cyril will replace nearest-RGB projection for the three speaker-identity roles
in ANSI-16 mode with fixed semantic slots. Every bundled theme will emit
LightBlue for user, LightGreen for agent, and LightMagenta for system while
retaining existing projection behavior for the other 26 roles and three color
modes.

## Users

- **Cyril terminal operator**: reads conversation turns and needs user, agent,
  and system identity to occupy different ANSI-16 slots from each other and
  from muted presentation roles.
- **Cyril UI contributor**: adds bundled themes and needs an executable contract
  that rejects speaker or muted-role projection collisions before the theme
  passes the workspace quality gate.

## Behavior

### Project speaker roles into fixed ANSI-16 slots

- **Given**: any bundled theme represented by a `ThemeId` variant
- **When**: Cyril resolves that theme with `ColorMode::Ansi16`
- **Then**: `user` is `Color::LightBlue`, `agent` is `Color::LightGreen`, and
  `system` is `Color::LightMagenta`

### Reject a bundled theme whose muted roles occupy protected slots

- **Given**: a bundled theme whose unchanged ANSI-16 projection maps `muted`,
  `border`, `subdued`, or `diff_context` to LightBlue, LightGreen, or
  LightMagenta
- **When**: the exhaustive theme-contract test and independent projection oracle
  evaluate the theme
- **Then**: validation fails and the theme cannot pass the workspace quality
  gate

### Preserve every projection outside the three speaker assignments

- **Given**: the projection outputs before this change and the outputs after it
- **When**: every bundled theme is resolved in all four color modes
- **Then**: all 26 non-speaker ANSI-16 role assignments and all 87 role
  assignments across true-color, ANSI-256, and no-color match their previous
  outputs

## Success criteria

- **Canonical speaker distinction**: 3/3 speaker roles for 100% of `ThemeId`
  variants emit their pinned ANSI-16 slots, measured by an exhaustive resolver
  test and compiled projection probe.
  This method cannot see: terminal palette remapping or whether the three colors
  are perceptually distinguishable for every form of color-vision deficiency.
- **Muted-role separation**: 4/4 muted-family roles for 100% of `ThemeId`
  variants emit none of the 3 protected speaker slots, measured by the
  exhaustive theme-contract test.
  This method cannot see: terminal palettes that render different ANSI slots as
  equal or perceptually similar colors.
- **Conversation binding**: 3/3 speaker labels in 1/1 pinned ANSI-16
  conversation scene use LightBlue, LightGreen, and LightMagenta, measured by
  Ratatui `TestBackend` cell inspection.
  This method cannot see: terminal-emulator rendering after `TestBackend`
  produces the correct ANSI slot identities.
- **Collision rejection**: 1/1 synthetic bundled-theme fixture with a protected
  muted-role collision fails contract validation, measured by a negative unit
  test.
- **Projection stability**: 26/26 non-speaker ANSI-16 assignments and 87/87
  assignments across the other three modes remain unchanged per bundled theme,
  measured by the signed projection table and compiled probe.
- **Independent oracle agreement**: 29/29 ANSI-16 role rows for 100% of
  `ThemeId` variants agree with an independently maintained oracle that encodes
  the three fixed assignments, four forbidden muted-role collisions, and the
  existing nearest-RGB rule for every other role, measured by compiled probe
  comparison against oracle output.
  This method cannot see: widget code that fails to bind a rendered speaker to
  its speaker role; the pinned conversation-scene criterion covers that path.
- **Regression compatibility**: 100% of workspace tests pass, measured by
  `cargo test --workspace`.
- **Quality gate**: 3/3 commands exit with status 0: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace`.

No latency target is introduced because resolution remains a fixed 29-role
operation over a compile-time theme set.

## Edge cases and decisions

<!-- markdownlint-disable MD013 -->

| Edge | Decision | Source |
| --- | --- | --- |
| Empty theme set | Not applicable: `ThemeId` is a non-empty compile-time enum and arbitrary runtime palettes are unsupported. | ADR 0005; `theme.rs:4-7` |
| Maximum scale | Evaluate exactly 29 roles for every `ThemeId` variant; no row-count-dependent input exists. | `.cyril-ghuu/spec.md`, contract constraint; `gap question Q2, this session` |
| Null or missing speaker field | Not applicable: `SourceTheme` requires all 29 fields at compile time. | `theme.rs:35-69` |
| Concurrent writes | Not applicable: theme resolution reads immutable values and performs no writes. | `theme.rs:204-238` |
| Permission denied or unauthenticated | Not applicable: projection performs no I/O or authentication. | `theme.rs:204-238` |
| Partial failure | A bundled theme either passes all identity constraints or fails the contract gate; no partial resolved theme is accepted. | `gap question Q6, this session` |
| Retries and idempotency | Repeated resolution of the same theme and mode returns an equal `Theme`. | `.cyril-ixua/spec.md`, deterministic projection decisions |
| Soft-deleted records | Not applicable: projection has no records or persistence. | ADR 0005 |
| Multi-tenancy boundaries | Not applicable: bundled themes contain no tenant-scoped state. | ADR 0005 |
| Time zone or DST | Not applicable: projection reads no clock or calendar. | `theme.rs:204-238` |
| Replication lag | Not applicable: projection uses no distributed state. | `theme.rs:204-238` |
| Cache invalidation | Resolved role values continue to participate in color-bearing cache identities; structural cache work remains in `cyril-x5xi`. | `.cyril-ghuu/spec.md`, cache edge; `cyril-x5xi` |
| Customized terminal palette | Emit the three protected named ANSI slots; actual terminal RGB values and perceptual separation are outside the guarantee. | `gap question Q4, this session` |
| Future bundled theme collides through a muted role | Reject it at the contract quality gate rather than remapping any non-speaker role. | `gap question Q6, this session` |

<!-- markdownlint-enable MD013 -->

## Out of scope

This change does NOT include:

- activating or configuring ANSI-16 mode (`cyril-qaq0`);
- changing source-palette values or adding bundled themes (`cyril-leiq`,
  `cyril-fkke`);
- changing true-color, ANSI-256, no-color, or Syntect token projection;
- changing any of the 26 non-speaker ANSI-16 role assignments;
- cache-identity or projection-test-plumbing refactors (`cyril-x5xi`,
  `cyril-xv3e`);
- changing widget labels, symbols, or layout;
- guaranteeing perceptual distinction after terminal palette remapping or for
  every form of color-vision deficiency.

## Constraints

<!-- markdownlint-disable MD013 -->

| Dimension | Limit | How measured |
| --- | --- | --- |
| Semantic role contract | Exactly 29 UI roles | Exhaustive contract test |
| Changed ANSI-16 assignments | Exactly 3 speaker roles per bundled theme | Signed projection diff |
| Protected speaker slots | Exactly LightBlue, LightGreen, and LightMagenta | Resolver test and independent oracle |
| Muted-family exclusions | 4/4 roles avoid all 3 protected slots | Contract test |
| Stable ANSI-16 assignments | 26/26 non-speaker roles per bundled theme | Compiled probe comparison |
| Stable non-ANSI-16 assignments | 87/87 role assignments per bundled theme | Compiled probe comparison |
| Runtime palette inputs | 0 | `ThemeId` API and ADR 0005 |

<!-- markdownlint-enable MD013 -->

## Decisions

<!-- markdownlint-disable MD013 -->

| # | Decision | Source | Why |
| --- | --- | --- | --- |
| 1 | The current ANSI-16 projection is defective because user and system both become Gray while agent and the muted family become DarkGray. | `docs/reviews/2026-07-11-conversation-theme-code-review.md`, finding 6 | The collision removes the intended speaker-role separation. |
| 2 | Correct projection semantics rather than retuning true-color source values. | `docs/reviews/2026-07-11-conversation-theme-review-decisions.md`, finding 6 | True-color values are a separate palette contract. |
| 3 | Pin user to LightBlue, agent to LightGreen, and system to LightMagenta; protect those slots from the muted family. | `gap question Q1, this session` | Exact semantic slots make the behavior deterministic and independently testable. |
| 4 | Apply the speaker mapping to every bundled theme. | `gap question Q2, this session` | `cyril-qaq0` activates ANSI-16 for every installed theme. |
| 5 | Change only the three ANSI-16 speaker assignments. | `gap question Q3, this session` | The other roles and modes are outside this bug's boundary. |
| 6 | Guarantee named ANSI-slot identity, not perception under every terminal palette. | `gap question Q4, this session` | Terminals control the physical RGB values of ANSI slots. |
| 7 | Exclude activation, palette work, other modes, caches, test-plumbing refactors, widget content, and universal perceptual guarantees. | `gap question Q5, this session` | Each excluded behavior has separate ownership or no enforceable terminal-independent oracle. |
| 8 | Reject a future bundled theme whose muted projection occupies a protected speaker slot. | `gap question Q6, this session` | This preserves the cross-theme identity rule without changing non-speaker projections. |
| 9 | Keep visual-theme selection separate from terminal color mode and project once into read-only UI state. | `docs/adr/0005-semantic-themes-and-color-modes.md` | Widgets must not implement independent fallback rules. |
| 10 | Keep speaker identity roles separate from status-severity roles. | `.cyril-ixua/spec.md`, decision 4 | Identity and status are different meanings. |
| 11 | Block ANSI-16 activation until this collision is corrected. | `.cyril-qaq0/spec.md`, decision 1; Rivets dependency on `cyril-q9dx` | Production selection must not activate the known identity collapse. |
| 12 | Supersede nearest-RGB projection for the three ANSI-16 speaker roles while retaining it elsewhere. | `.rivets/issues.jsonl`, `cyril-q9dx`; decisions 3 and 5 above | The older `.cyril-ixua` and `.cyril-ghuu` nearest-RGB criteria certify the reported defect for those roles. |

<!-- markdownlint-enable MD013 -->

## Sign-off

<!-- markdownlint-disable MD013 -->

Consequences stated to the requester:

In ANSI-16 mode, every bundled theme will render **user as LightBlue, agent as LightGreen, and system as LightMagenta**. Those slots are protected from the four muted roles; a future bundled theme that collides will fail the theme-contract quality gate. The remaining 26 roles and all other color modes remain unchanged.

The tests and independent oracle observe ANSI slot identities and TestBackend cells. They cannot see terminal palette remapping or every form of color-vision deficiency. On a terminal whose bright blue, green, and magenta are configured similarly, speakers may still look indistinguishable.

Cyril terminal operators will not receive theme selection, ANSI-16 activation, new palettes, changed syntax colors, revised labels/layout, cache refactoring, or broader contrast fixes from this change.

The requester replied, verbatim: "confirmed"

Date: 2026-07-11

<!-- markdownlint-enable MD013 -->
