# Design: Role-aware ANSI-16 speaker projection

## Purpose

Cyril's generic nearest-RGB projection remains the geometric foundation for
ANSI-16, but speaker identity becomes a role-aware finalization step. The
resolver will protect three terminal slots for user, agent, and system while
rejecting bundled themes whose muted-family projection occupies a protected
slot.

This design extends the behavior proved in `.cyril-q9dx/prototype.md`. It does
not contradict the 116/116 resolver-row agreement or the 3/3 runtime buffer-cell
agreement recorded there.

## Evidence base

- `.cyril-q9dx/spec.md` pins the three assignments, cross-theme rule, muted-role
  exclusion, unchanged outputs, proxy limits, and requester sign-off.
- `.cyril-q9dx/material-boundaries.md` marks representation, visibility, and
  Ratatui buffer semantics as material.
- `.cyril-q9dx/prototype.md` proves that a post-projection assignment changes
  exactly three rows and reaches the visible conversation markers.
- `.cyril-q9dx/related-issues.md` records the bounded prior-art search and issue
  relationships.

## Input shapes

<!-- markdownlint-disable MD013 -->

| Input | Production-reachable shapes | Design coverage |
| --- | --- | --- |
| `ThemeId` | Current `CyrilDark`; any additional bundled variant | C0 generates an authoritative enumeration from the enum declaration; C1 and C2 iterate it. |
| Bundled-theme validity | Four muted-family roles avoid protected slots; any one of four roles occupies any one of three protected slots | C2 covers 1 valid class and all 12 invalid role-slot combinations. |
| `ColorMode` | `TrueColor`, `Ansi256`, `Ansi16`, `None` | C1 changes only `Ansi16`; C3 and C7 preserve the other modes and syntax shape. |
| `SourceColor` | One `Reset` canvas; 28 `Rgb` role values | C3 preserves reset and non-speaker output; C5 preserves geometric projection for every applicable RGB role. |
| Semantic-role partition | 3 speakers; 4 muted-family roles; 22 remaining roles | C1, C2, C3, and C5 cover each partition separately. |
| Syntax component | `Some(selected bundled syntax)` in true-color, ANSI-256, and ANSI-16; `None` in no-color | C7 covers both `Option` shapes and preserves each theme's selected component. |
| Committed identity messages | `UserText`, `AgentText`, `System` | C4 observes all three through the actual chat renderer. |
| Streaming identity messages | Main-agent streaming label; subagent streaming label | C4 requires both agent-label paths to use the resolved agent role. |
| Identity message text | Empty user/agent text; non-empty ASCII; Unicode with spaces; empty system text | C4 covers labels for empty user/agent and styled cells for non-empty Unicode system text; empty system text has no rendered cell, so C1 remains the observable contract for that shape. |
| Viewport | Pinned 80×24 identity scene | C4 covers this signed viewport; scrolling, clipping policy, and alternate viewport layout are unchanged behaviors. |
| Repeated call | First resolution; repeated resolution with the same inputs | C6 covers deterministic idempotency. |

<!-- markdownlint-enable MD013 -->

No arbitrary runtime palette collection enters this feature. ADR 0005 limits
the resolver to the closed bundled-theme model.

## Removed-invariant sweep

The core move is subtractive: three roles lose the old invariant that every RGB
role's final ANSI-16 value is its nearest canonical palette entry.

<!-- markdownlint-disable MD013 -->

| Former invariant or derived assumption | After this change | Coverage |
| --- | --- | --- |
| Every RGB role's final ANSI-16 value comes directly from `SourceColor::ansi16`. | False for user, agent, and system; true for the other 25 RGB roles. | C1 and C5. |
| Lower-index nearest-distance tie-breaking determines every final ANSI-16 role. | It still determines 25 non-speaker RGB roles; the three speakers use fixed semantic slots. | C5. |
| Equal source RGB values imply equal final ANSI-16 values. | Speaker/generic pairs may diverge: user/soft-accent, agent/positive-accent, and system/alternate-accent. | Safe because renderers read separate role fields and both color-bearing cache keys hash the fields separately; source inspection found no equality-dependent reader. |
| A role-agnostic brute-force oracle can certify all ANSI-16 rows. | False; the oracle must use semantic expectations for three role names and geometric expectations for the other roles. | C1 and C5; acceptance oracle component below. |
| `resolve_ansi16` cannot reject a developer-authored bundled palette. | False when a muted-family role occupies a protected speaker slot. | C2 names the role and rejects before assignment. |
| True-color, ANSI-256, no-color, and syntax selection share the same old behavior. | Still true. | C3 and C7. |

<!-- markdownlint-enable MD013 -->

No serialization, ordering, lock, uniqueness, or shared-state invariant is
removed. Resolution remains synchronous and receives immutable source data.

## Architecture

### 0. Generate an authoritative bundled-theme registry

A local declarative macro will generate both the `ThemeId` variants and
`ThemeId::ALL` from one variant list. Adding a variant through that declaration
automatically places it in the registry; the existing source-theme `match`
remains exhaustive and therefore requires its source values.

Every theme-contract test, compiled source probe, and acceptance oracle will
iterate `ThemeId::ALL`. The probe output gains a theme-id column so a failure
identifies the exact bundled theme and role. This closes the current
single-theme sampling gap without a manually duplicated registry.

### 1. Keep geometric projection role-agnostic

`SourceColor::ansi16`, `nearest_ansi16`, `ANSI16_RGB`, `ANSI16_COLORS`, and
lower-index tie-breaking remain unchanged. `resolve_with(id,
SourceColor::ansi16)` continues to produce a complete geometric `Theme`.

This preserves one source of truth for canonical palette math and avoids
teaching `SourceColor` about semantic role names.

### 2. Add an ANSI-16 semantic finalizer

A private finalizer receives the geometric `Theme` by value. It performs these
operations in order:

1. Inspect `muted`, `border`, `subdued`, and `diff_context` for membership in
   `{LightBlue, LightGreen, LightMagenta}`.
2. If a collision exists, fail the bundled-theme invariant with a diagnostic
   naming the first colliding role and color.
3. Assign `user = LightBlue`, `agent = LightGreen`, and `system = LightMagenta`.
4. Return the otherwise unchanged `Theme`, including its syntax component.

`resolve_ansi16` becomes geometric projection followed by this finalizer. The
finalizer is after the exhaustive `ThemeId` source match, so every bundled
variant traverses the same semantic rule without a per-theme branch.

The collision is a programmer-authored bundled-theme contract violation, not an
external runtime error. The public resolver remains infallible for valid bundled
themes; an internal assertion prevents an invalid built-in theme from silently
rendering.

### 3. Preserve role observability

The finalizer updates only the three existing public `Theme` fields. Committed
and streaming renderers already consume those fields, and Ratatui preserves the
named variants in `TestBackend` cells according to the prototype.

No widget receives a new projection rule, no role is recomputed in a widget,
and no color mode is detected below the resolver seam.

### 4. Split geometric and semantic verification

Production unit tests will distinguish these properties:

- canonical RGB-distance and tie-breaking math;
- fixed semantic speaker assignments;
- muted-family slot exclusion;
- unchanged non-speaker and non-ANSI-16 outputs;
- syntax-component preservation;
- runtime conversation bindings.

A production acceptance oracle will consume freshly compiled
`emit_source_probe` rows for every `ThemeId::ALL` entry. It will expect semantic
indexes 12, 10, and 13 for user, agent, and system, while independently
computing nearest entries for the other 25 RGB ANSI-16 rows and all 28 ANSI-256
rows. Historical `.cyril-q9dx/oracle.py`, `.cyril-ixua`, and `.cyril-ghuu`
artifacts stay as prototype and superseded-contract evidence; the production
acceptance oracle is a separate role-aware implementation artifact.

## Claims

0. **C0:** `ThemeId::ALL` contains every declared bundled-theme variant exactly
   once because the enum and registry come from one macro invocation.
1. **C1:** Resolving any bundled theme in ANSI-16 assigns user to LightBlue,
   agent to LightGreen, and system to LightMagenta.
2. **C2:** ANSI-16 resolution rejects a geometric theme when any of the four
   muted-family roles occupies any protected speaker slot and identifies the
   colliding role.
3. **C3:** Cyril Dark changes exactly 3/29 ANSI-16 role values, while every
   bundled theme preserves 26/26 non-speaker ANSI-16 values and 87/87 role
   values across true-color, ANSI-256, and no-color.
4. **C4:** Every visible committed or streaming identity marker receives its
   corresponding resolved speaker role in the pinned 80×24 Ratatui buffer.
5. **C5:** Nearest-distance and lower-index tie-breaking remain authoritative
   for 25/25 non-speaker RGB ANSI-16 roles and 28/28 ANSI-256 roles.
6. **C6:** Repeated resolution with an unchanged `ThemeId` and `ColorMode`
   returns an equal `Theme` for 4/4 modes.
7. **C7:** Semantic finalization preserves each bundled theme's selected syntax
   component in ANSI-16, while the existing three-`Some`/one-`None` mode shape
   remains unchanged.

## Material-boundary coverage

<!-- markdownlint-disable MD013 -->

| Prototype boundary | Design treatment |
| --- | --- |
| Representation and normalization | C1, C2, C3, C5, and C7 separate geometric projection, semantic assignment, collision validation, and syntax shape. |
| Selection and visibility | C4 exercises committed and streaming identity paths through the actual renderer at 80×24. |
| External Ratatui semantics | C4 inspects named `Color` variants on runtime buffer cells rather than inferring them from source alone. |
| Mutable shared state | Cannot affect accepted behavior: speaker markers bypass Markdown/syntax caches, while existing cache keys already hash the separate speaker fields; `cyril-x5xi` owns structural cache-identity work. |
| Ordering and concurrency | Cannot affect accepted behavior: one resolution and one frame render are synchronous immutable reads. |
| Transport and serialization | Cannot affect accepted behavior: the feature adds no file, process, or protocol transport; TSV is acceptance evidence rather than production data flow. |

<!-- markdownlint-enable MD013 -->

## Oracle blindness ledger

<!-- markdownlint-disable MD013 -->

| Rule or stand-in | Difference it erases | Treatment |
| --- | --- | --- |
| Canonical ANSI-16 slot identity | Per-terminal RGB palette remapping | Named accepted risk from spec Q4 and consequence sign-off: a terminal may render the three slots similarly or identically. |
| Equality of three named `Color` variants | Perceptual distinguishability across color-vision conditions | Named accepted risk from spec Q4; this feature guarantees semantic slot identity, not universal perception. |
| Ratatui `TestBackend` | Terminal-emulator SGR handling and physical display output | Named accepted risk from the signed spec; C4 proves only the in-process buffer seam. |
| Pinned 80×24 scene | Other viewport sizes and scroll positions | Accepted boundary: this change does not alter layout or clipping, and C4 includes all identity paths in the signed viewport. |
| Python source-binding oracle | Runtime selection, clipping, and external-library behavior | Covered by C4's separate runtime `TestBackend` falsifier. |
| Projection oracle's role partition | Widget code that binds a marker to the wrong role | Covered by C4 with per-marker output. |
| Prototype's single `CyrilDark` theme | Palette-specific muted collisions in additional bundled themes | Covered by C0's generated registry and C2's exhaustive contract/oracle iteration. |

<!-- markdownlint-enable MD013 -->

## Falsification

<!-- markdownlint-disable MD013 -->

| # | Claim | Falsifier | Independent oracle | Bug that makes it fail | Cost | Status | Regression fence |
| --- | --- | --- | --- | --- | ---: | --- | --- |
| 1 | C1 exact speaker slots | Run the Rust probe against the actual resolver, apply the proposed finalizer to every currently registered theme, and emit `C1 SPEAKER_SLOT_MISMATCH` for any wrong role/color pair. | Human-signed fixed mapping encoded independently in `.cyril-q9dx/design-falsifier.py`, which launches the Rust probe itself. | Swap agent/system assignments or leave one role on nearest-RGB output. | <1s warm | **Passed:** `C1 PASS user=LightBlue agent=LightGreen system=LightMagenta` | Unit test `theme::tests::ansi16_speaker_roles_use_semantic_slots`. |
| 2 | C0 exhaustive registry | Expand the registry macro with three test variants and inspect the generated `ALL`; emit `C0 REGISTRY_MISMATCH` unless all 3 appear once in declaration order. | Independent AST extraction of the single macro invocation's variant tokens. | Declare `ALL` manually and omit the middle variant. | <1s | Pending implementation | Unit test `theme::tests::bundled_theme_registry_is_complete_and_unique`; compiled probes iterate `ThemeId::ALL`. |
| 3 | C2 collision rejection | Inject each of 4 muted roles into each of 3 protected slots; emit `C2 COLLISION_ACCEPTED role=<r> color=<c>` for any accepted case or wrong diagnostic. | Independent set-intersection table over 12 role-slot pairs. | Run validation after overwriting speaker fields, or omit `diff_context` from the four checked roles. | <1s | Pending implementation | Unit test `theme::tests::ansi16_rejects_all_muted_speaker_slot_collisions`. |
| 4 | C6 deterministic resolution | Run two fresh probe processes for each of 4 modes and byte-diff their role rows; emit `C6 NONDETERMINISTIC mode=<m>` on any difference. | OS byte comparison between separate process outputs. | Rotate speaker slots through mutable static state. | <1s | Pending implementation | Unit test `theme::tests::resolution_is_deterministic_in_every_mode`. |
| 5 | C7 syntax preservation | Resolve all 4 modes and emit source and resolved syntax selections; emit `C7 SYNTAX_CHANGED mode=<m>` if ANSI-16 loses the theme's component or the existing Some/None mode shape changes. | Independent source-theme selection plus Syntect default-theme lookup. | Reconstruct `Theme` without carrying `syntax`, causing ANSI-16 to become `None`. | <1s | Pending implementation | Unit test `theme::tests::ansi16_semantics_preserve_syntax_component`. |
| 6 | C3 exact change surface | Compare freshly compiled role-mode rows for every `ThemeId::ALL` entry; emit `C3 UNEXPECTED_DIFF theme=<t> mode=<m> role=<r>` for any non-speaker or non-ANSI-16 difference, and require the 3 signed Cyril Dark differences. | Production acceptance oracle parses source literals independently and computes role-aware expectations; `.cyril-q9dx/oracle.py` remains prototype evidence only. | Apply fixed speaker colors in `resolve_with`, changing true-color or ANSI-256 too. | 2s | Prototype evidence passed; production fence pending | Unit tests `theme::tests::ansi16_semantics_change_only_speaker_roles` and `theme::tests::other_modes_remain_unchanged`. |
| 7 | C5 geometric projection remains authoritative | Emit compiled indexes for 25 non-speaker ANSI-16 and 28 ANSI-256 RGB roles for each registered theme; emit separate `C5 ANSI16_NEAREST` or `C5 ANSI256_NEAREST` mismatch rows. | Python brute-force canonical tables with lower-index ordering over each emitted source row. | Special-case a generic role in `SourceColor::ansi16` or change tie comparison from `<` to `<=`. | 2s | Pending implementation | Unit tests `theme::tests::non_speaker_ansi16_remains_nearest` and existing tie-break tests. |
| 8 | C4 runtime identity bindings | Render empty user/agent messages, Unicode system text with spaces, main streaming agent, and subagent streaming agent at 80×24; emit `C4 BINDING_MISMATCH path=<p>` per incorrect or missing cell. | Lexical arm-to-role extraction plus fixed semantic mapping, compared with runtime buffer cells. | Pass `state.theme()` instead of the frame theme on one streaming path, or hard-code DarkGray for system text. | 3s | Committed-message prototype passed; expanded shape fence pending | Widget test `chat::tests::ansi16_identity_consumers_use_speaker_roles`. |

<!-- markdownlint-enable MD013 -->

The cheapest falsifier was run after the claims were fixed. The Python command
launches `cargo run --bin q9dx-probe` and parses fresh stdout; it does not read
the captured TSV.

```text
python .cyril-q9dx/design-falsifier.py
C1 PASS user=LightBlue agent=LightGreen system=LightMagenta
```

Captured result: `.cyril-q9dx/design-falsifier-output.txt`.

## Negative space

1. Startup selection, automatic detection, and the `/theme` picker are owned by
   verified ticket `cyril-qaq0`; this design changes resolver output only.
2. Five additional source palettes are owned by verified ticket `cyril-fkke`;
   the finalizer applies structurally when those variants exist but defines no
   palette values.
3. Fixed-palette contrast targets are owned by verified ticket `cyril-leiq`;
   this design guarantees named slots rather than display contrast.
4. Structural cache-key completeness is owned by verified ticket `cyril-x5xi`;
   this design changes values already present in both cache keys.
5. Projection-fixture plumbing consolidation is owned by verified ticket
   `cyril-xv3e`; this design adds semantic expectations without consolidating
   existing helpers.
6. Widget text, labels, layout, scrolling, clipping, and syntax-token RGB
   projection remain unchanged because they do not determine speaker-slot
   assignment.
7. Historical `.cyril-ixua` and `.cyril-ghuu` artifacts remain unchanged as an
   audit trail of the contract that q9dx supersedes for three roles.

## Tracker verification

Verified against `.rivets/issues.jsonl` during this design session:

<!-- markdownlint-disable MD013 -->

| ID | Status | Scope match |
| --- | --- | --- |
| `cyril-qaq0` | open | Activates configured terminal modes and theme selection; depends on `cyril-q9dx`. |
| `cyril-fkke` | open | Adds five bundled palettes and requires deterministic ANSI projections. |
| `cyril-leiq` | open | Defines fixed-palette contrast targets and tests current dim values. |
| `cyril-x5xi` | open | Makes theme-dependent cache identities structurally complete. |
| `cyril-xv3e` | open | Consolidates projection and conversation-fixture test plumbing. |
| `cyril-q9dx` | open | Owns ANSI-16 speaker identity and semantic oracle constraints. |

<!-- markdownlint-enable MD013 -->

## Self-review

- **Claim count:** 8; the feature remains one resolver-policy change plus its
  structurally exhaustive bundled-theme registry.
- **Input coverage:** every reachable enum, option, role partition, collision
  class, message identity path, and repeated-call shape maps to a claim or a
  written no-cell/layout reason.
- **Removed invariants:** universal nearest projection, tie applicability,
  equal-source/equal-output, role-agnostic oracle behavior, and infallible
  invalid-palette handling are accounted for.
- **Oracle independence:** compiled Rust, Python palette math, OS byte diff,
  lexical binding extraction, and Ratatui runtime buffers are separated by
  claim.
- **Non-vacuity:** every falsifier row names a concrete buggy implementation.
- **Distinctness:** failures carry claim-specific `C0` through `C7` output and
  identify the mode, role, color, or renderer path where applicable.
- **Regression fences:** every empirical or manual falsifier has a named
  deterministic unit or widget test.
- **Material boundaries:** every material prototype boundary has a runtime or
  representation claim; non-material boundaries retain written reasons.
- **Blindness:** every `This method cannot see` consequence from the signed spec
  appears in the oracle blindness ledger as covered behavior or accepted risk.
- **Negative space:** seven deliberate exclusions are named, with verified
  tracker ownership where implementation is promised elsewhere.

The design is ready for requester review because the cheapest falsifier passed.

## Approval

The requester approved this design for `budgeted-plan`, verbatim: "yes"

Date: 2026-07-11
