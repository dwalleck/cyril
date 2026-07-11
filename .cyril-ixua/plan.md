# Budgeted plan: Semantic theme seam for Cyril Dark

Status: approved; 11 slices with bounded Slice 1 dead-code debt (2026-07-10)

## Basis and checkpoint protocol

This plan implements the approved `.cyril-ixua/design.md`. Its unit is a
checkpointed slice, not a commit-sized code transcription. The implementer may
change the internal shape when evidence demands it, but may not change a claim,
stress expectation, or budget without returning to design review.

Production scale is fixed for this ticket:

- Semantic roles `R = 19`, of which 18 are RGB and one is reset.
- ANSI-256 candidates `P256 = 240` (indices 16–255).
- ANSI-16 candidates `P16 = 16`.
- Compatibility frame size `W × H = 80 × 24 = 1,920` cells.
- Widget-source architecture scan `B ≤ 300,000` bytes across at most 16 files.

Starting with Slice 1, a test-only emitter in the compiled `cyril-ui` test
binary writes source/projection rows between explicit markers. The checkpoint
extracts those rows to a temporary TSV and runs
`.cyril-ixua/oracle.py --input <tsv>`. A two-column `role/rgb` stream checks
source and true-color values; ANSI columns are added only when their projection
exists. Agreement lines are data on stdout. Oracle disagreements are
diagnostics on stderr and return nonzero.

Every slice runs formatting, workspace tests, and workspace Clippy. Slice 1 may
finish with `dead_code` warnings only for the private source-theme path that has
no production consumer until Slice 2. Those warnings are recorded, not
suppressed, and no other warning is accepted. Slice 2 must remove the entire
warning ledger. Slice 2 and every subsequent slice run the strict repository
gate:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The full prove-it probe is also rerun after every slice so a structural change
cannot silently alter the agreed 18-role projection target.

<!-- markdownlint-disable MD013 -->

## Claim coverage

| Design claim | Plan slices |
| ---: | --- |
| 1. Exact 19-role Cyril Dark contract | 1–2 |
| 2. RGB/reset-only source, canvas sole reset | 1 |
| 3. True-color identity | 2 |
| 4. Nearest ANSI-256 projection | 3 |
| 5. Nearest ANSI-16 projection and named output | 4 |
| 6. Lower-index tie-breaking | 5 |
| 7. Complete no-color projection | 6 |
| 8. Valid Syntect component | 7 |
| 9. Three unchanged render buffers | 9–11 |
| 10. Unchanged configuration and no widget consumer | 8 |

<!-- markdownlint-enable MD013 -->

## Slice 1: Establish the fixed Cyril Dark source contract

**Claim:** The source and resolved shapes contain all 19 semantic roles, Cyril
Dark contains the 19 pinned values and syntax identifier, every source color is
RGB or reset, and canvas is the sole reset.

**Oracle:** Parse the signed role list and compatibility table independently,
then compare them with role/value rows emitted by the compiled test binary;
compare role/reset/syntax counts with literal expectations 19/1/1.

**Stress fixture:** Populate semantically distinct roles that deliberately share
values, plus the reset canvas. Expected: all 19 role names are present, 18 RGB
rows pass the oracle, exactly one reset role is canvas, and the syntax identifier
is `base16-eighties.dark`. This fails if a field is omitted, roles are
collapsed by equal color, a value drifts, a named ANSI source color enters, or
syntax is absent.

**Smallest change:** Atomically replace the unverified draft with local
explicit-mode and single-theme identifiers, private RGB/reset source color,
fixed source/resolved role containers, the pinned Cyril Dark source, typed
syntax identifier, test-binary source-row emitter, and unused module export.
The private source path remains intentionally unreachable until the true-color
resolver in Slice 2 consumes it; its bounded `dead_code` warnings are carried in
the Slice 1 ledger without lint suppression or widened visibility.

**Loop budget:** Production construction is explicit field assignment with no
loop. Test emission is `O(R) = 19` visits, below 10^6 operations with zero
syscalls.

**Wall budget:** Not applicable; the source has no always-on caller.

**Files:**

- `crates/cyril-ui/src/theme.rs`
- `crates/cyril-ui/src/lib.rs`

**Code (advisory):** No code is pre-typed; the fixed fields, signed value table,
and source-color sum type are the contract.

**Verification:**

- [ ] Unit tests pass; Clippy reports only the recorded private source-theme
      `dead_code` ledger and no other warning.
- [ ] Stress fixture emits all 19 roles, 18 oracle-approved RGB rows, one reset
      canvas, and one syntax identifier.
- [ ] Prove-it oracle reports `AGREE role-names 18/18` and
      `AGREE role-values 18/18` against compiled source rows.
- [ ] Loop and wall budgets hold at `R = 19`.

## Slice 2: Resolve true-color without transforming the source

**Claim:** True-color resolution preserves every pinned RGB value exactly and
preserves the reset canvas.

**Oracle:** Compare compiled `role/rgb` rows with the signed table and compare
the canvas with the committed terminal-default behavior.

**Stress fixture:** Resolve the full source containing zero-channel colors,
255-channel colors, mid-range colors, duplicate values, and reset. Expected:
18 byte-identical RGB triples, canvas reset, and no syntax-identifier change.
This fails if reset becomes black, arithmetic alters a channel, or duplicate
roles collapse.

**Smallest change:** Add the pure true-color resolver and focused identity test;
make the binary emitter source its rows from resolved true-color output.

**Loop budget:** Mapping all roles is `O(R) = 19` color visits per explicit
resolution. Production scale is one on-demand theme resolution, so 19
operations are below budget with zero syscalls.

**Wall budget:** Not applicable; resolution is on demand, not an always-on
phase.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; use the smallest pure mapping that
preserves the signed source values.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture preserves 18 RGB triples, one reset, and the syntax ID.
- [ ] Prove-it oracle reports `AGREE role-values 18/18` against compiled
      true-color rows.
- [ ] Loop and wall budgets hold at `R = 19`.

## Slice 3: Project onto the fixed ANSI-256 palette

**Claim:** Every RGB role maps to the minimum-distance fixed xterm entry in
indices 16–255.

**Oracle:** The Python oracle independently generates the 216-color cube and
24-entry grayscale ramp, then brute-forces each compiled projection.

**Stress fixture:** Project all 18 roles, with required sentinels
`#1e1e2e → 235`, `#282c34 → 236`, `#323246 → 237`, `#8c8c8c → 245`, and user
message `#8ab4f8 → 111`. Expected: 18/18 agreement. This fixture defeats the
known cube-rounding bug, which returns indices 16, 17, 17, 102, and 110 for
those cases.

**Smallest change:** Add fixed xterm RGB generation, squared-distance
calculation, and lower-index nearest search for ANSI-256; add the `ansi256`
column to compiled probe output.

**Loop budget:** `O(R × P256) = 18 × 240 = 4,320` distance evaluations per
on-demand theme resolution. Each evaluation performs three bounded integer
subtractions/squares; total work is below 10^6 operations and uses zero
syscalls.

**Wall budget:** Not applicable; resolution is not always on.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; exhaustive fixed-palette search is
the behavioral constraint.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture returns the five sentinel indices and 18/18 agreement.
- [ ] Prove-it oracle reports `AGREE ansi256 18/18` against compiled output.
- [ ] Loop and wall budgets hold at 4,320 distance evaluations.

## Slice 4: Project onto canonical ANSI-16 named colors

**Claim:** Every RGB role maps to the minimum-distance canonical ANSI-16 entry
and then to the corresponding Ratatui named color.

**Oracle:** Python brute-forces the canonical 16-entry RGB table; an independent
literal index-to-Ratatui-variant table checks the compiled named output.

**Stress fixture:** Project all 18 roles, including cyan accent to index 14,
muted `#8c8c8c` to index 8, selection `#323246` to index 4, user message to
index 7, and agent message to index 8. Expected: 18/18 index agreement and exact
named variants. This defeats the current threshold heuristic, which disagrees
on 9/18 roles.

**Smallest change:** Add the canonical table, nearest search, and exhaustive
index-to-Ratatui-color conversion; add `ansi16` index and variant evidence to
tests while retaining the oracle's TSV index column.

**Loop budget:** `O(R × P16) = 18 × 16 = 288` distance evaluations per on-demand
resolution, below 10^6 operations with zero syscalls.

**Wall budget:** Not applicable; resolution is not always on.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; table completeness and nearest-entry
behavior govern implementation.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture returns all five sentinels and exact Ratatui variants.
- [ ] Prove-it oracle reports `AGREE ansi16 18/18` against compiled output.
- [ ] Loop and wall budgets hold at 288 distance evaluations.

## Slice 5: Lock deterministic lower-index tie-breaking

**Claim:** Equal-distance palette candidates choose the lower index in both
ANSI modes.

**Oracle:** A Python brute-force fixture enumerates every minimum-distance
candidate and independently selects `min(index)`.

**Stress fixture:** Project ANSI-256 RGB `(13,13,13)`, equidistant from grayscale
indices 232 and 233, and ANSI-16 RGB `(64,0,0)`, equidistant from indices zero
and one. Expected: indices 232 and 0. This fails under a last-wins comparison,
reversed candidate traversal without index in the key, or `<=` replacement
logic.

**Smallest change:** Make `(distance, index)` the explicit comparison key and add
the two adversarial regression cases; no new public API.

**Loop budget:** The fixture performs one 240-entry and one 16-entry search:
`O(P256 + P16) = 256` evaluations. Production asymptotics remain those of
Slices 3–4.

**Wall budget:** Not applicable; no always-on phase is introduced.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; the comparison must make lower-index
ties explicit rather than relying on traversal order.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture returns ANSI-256 index 232 and ANSI-16 index 0.
- [ ] Prove-it oracle still agrees with all compiled role projections.
- [ ] Loop and wall budgets hold at 256 fixture evaluations.

## Slice 6: Remove every color in explicit no-color mode

**Claim:** No-color resets all 19 UI roles and removes the syntax component.

**Oracle:** Serialize the compiled no-color result and independently count
19 `Reset` values plus a missing syntax component.

**Stress fixture:** Resolve the complete Cyril Dark source, including duplicate
message/status values and reset canvas. Expected: 19 resets, zero explicit
foreground/background colors, and no syntax identifier. This fails if only
accent roles reset, canvas is special-cased incorrectly, or syntax remains.

**Smallest change:** Add the no-color mapping and focused exhaustive count test.

**Loop budget:** `O(R) = 19` role visits per on-demand resolution, below budget
with zero syscalls.

**Wall budget:** Not applicable; resolution is not always on.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; exhaustive fixed-role mapping is the
required behavior.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture reports 19 resets and no syntax component.
- [ ] Prove-it oracle still agrees with the compiled true-color and ANSI rows.
- [ ] Loop and wall budgets hold at `R = 19`.

## Slice 7: Validate the typed Syntect component

**Claim:** Cyril Dark's typed syntax identifier resolves in Syntect's loaded
default theme set.

**Oracle:** Syntect's packaged default-theme catalog independently contains the
exact key `base16-eighties.dark`.

**Stress fixture:** Validate the real typed identifier and a test-only typo
`base16-eighties.drak`. Expected: real identifier present, typo absent. This
fails if validation silently accepts a missing theme or the source carries an
unchecked arbitrary string.

**Smallest change:** Add the typed identifier-to-name projection and contract
test against `ThemeSet::load_defaults`; production resolution remains
panic-free and performs no catalog lookup.

**Loop budget:** Production adds no loop. Test setup loads Syntect's fixed
packaged catalog once and performs two expected-`O(1)` map lookups; it is
test-only and introduces no production syscall.

**Wall budget:** Not applicable; validation is a test gate, not always-on work.

**Files:**

- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; prefer a typed identifier and a
single name projection over arbitrary strings.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture accepts the exact key and rejects the one-character typo.
- [ ] Prove-it oracle still agrees with all compiled projection rows.
- [ ] Loop and wall budgets hold; production adds zero catalog scans.

## Slice 8: Enforce the ticket's configuration and consumption boundary

**Claim:** Public UI configuration remains its existing four fields and no
production widget consumes the new seam.

**Oracle:** Compare serialized default configuration keys and AST references
with committed `HEAD`.

**Stress fixture:** Deserialize a TOML document with non-default existing values
(`max_messages = 1000`, mouse disabled), serialize the UI configuration, and
scan every production widget source. Expected: existing values survive, the key
set is exactly the committed four keys, and theme-module references equal zero.
This fails on the current dirty draft because it adds theme/color fields; it also
fails if a widget is wired prematurely.

**Smallest change:** Remove the unapproved core config/theme additions, add an
exact schema regression test, and add a test-only bounded source-reference
fence for this expand ticket.

**Loop budget:** Production adds no loop. The test-only source scan is `O(B)`
for at most 300,000 bytes across 16 files, below 10^6 character checks and
16 file reads. The scan is not an always-on phase.

**Wall budget:** Not applicable; all added work is test-only.

**Files:**

- `crates/cyril-core/src/types/config.rs`
- `crates/cyril-ui/src/theme.rs`

**Code (advisory):** No code is pre-typed; remove only the unapproved draft and
keep all architecture enforcement test-only.

**Verification:**

- [ ] Unit tests pass.
- [ ] Stress fixture preserves non-default config, emits exactly four keys, and
      finds zero widget references.
- [ ] Prove-it oracle still agrees with all compiled projection rows.
- [ ] Loop budget holds at `B ≤ 300,000` bytes and at most 16 file reads.

## Slice 9: Fence the default-idle render buffer

**Claim:** Adding and exporting the unused seam changes zero symbols and zero
styles in the default-idle 80×24 frame.

**Oracle:** Apply the same test-only scene harness to an isolated clean worktree
at committed `HEAD`; compare its Ratatui debug buffer with the ticket revision
cell by cell.

**Stress fixture:** Render the default `MockTuiState` at exactly 80×24 and
snapshot every cell's symbol and style. Expected: 1,920/1,920 cells identical.
This fails if the seam is accidentally installed as a default renderer theme or
changes fallback colors.

**Smallest change:** Add one full-buffer debug snapshot assertion to the existing
render tests; change no production rendering branch.

**Loop budget:** New work is test-only `O(W × H) = 1,920` cell serialization.
No production loop or syscall is introduced.

**Wall budget:** Not applicable; the snapshot is test-only.

**Files:**

- `crates/cyril-ui/src/render.rs`
- `crates/cyril-ui/src/snapshots/cyril_ui__render__tests__theme_seam_idle.snap`

**Code (advisory):** No production code change is permitted; use the smallest
full-buffer snapshot fixture that preserves symbols and styles.

**Verification:**

- [ ] Unit and snapshot tests pass.
- [ ] Stress fixture reports 1,920/1,920 symbol/style matches against clean
      `HEAD`.
- [ ] Prove-it oracle still agrees with all compiled projection rows.
- [ ] Loop budget holds at 1,920 test cells.

## Slice 10: Fence the active tool-diff render buffer

**Claim:** Adding and exporting the unused seam changes zero symbols and zero
styles in the active-conversation-with-tool-diff 80×24 frame.

**Oracle:** Run the identical test-only tool-diff scene in a clean `HEAD`
worktree and compare the full Ratatui debug buffers.

**Stress fixture:** Render an active conversation containing one write tool with
both removed and added Unicode-bearing lines, syntax highlighting, line numbers,
and status color. Expected: 1,920/1,920 cells identical. This fails if theme
construction alters syntax, diff tinting, semantic labels, or default styles.

**Smallest change:** Add one adversarial full-frame tool-diff snapshot test to
the existing renderer test module; change no production rendering branch.

**Loop budget:** New work is test-only `O(W × H + D)`, where `W × H = 1,920`
and fixture diff input `D ≤ 20` lines, for fewer than 2,000 cell/line visits.
No production loop or syscall is introduced.

**Wall budget:** Not applicable; the snapshot is test-only.

**Files:**

- `crates/cyril-ui/src/render.rs`
- `crates/cyril-ui/src/snapshots/cyril_ui__render__tests__theme_seam_tool_diff.snap`

**Code (advisory):** No production code change is permitted; fixture construction
may reuse existing domain constructors.

**Verification:**

- [ ] Unit and snapshot tests pass.
- [ ] Stress fixture reports 1,920/1,920 symbol/style matches against clean
      `HEAD`.
- [ ] Prove-it oracle still agrees with all compiled projection rows.
- [ ] Loop budget holds below 2,000 test visits.

## Slice 11: Fence the open-picker render buffer

**Claim:** Adding and exporting the unused seam changes zero symbols and zero
styles in the open-picker 80×24 frame.

**Oracle:** Run the identical test-only picker scene in a clean `HEAD` worktree
and compare the full Ratatui debug buffers.

**Stress fixture:** Render a picker with current/non-current options, groups, and
a selected option carrying a description. Expected: 1,920/1,920 cells identical.
This fails if the new seam leaks into popup background, border, selection, text,
or description styling.

**Smallest change:** Add one adversarial full-frame picker snapshot test to the
existing renderer test module; change no production rendering branch.

**Loop budget:** New work is test-only `O(W × H + O)`, with 1,920 cells and
`O = 4` options, for 1,924 visits. No production loop or syscall is introduced.

**Wall budget:** Not applicable; the snapshot is test-only.

**Files:**

- `crates/cyril-ui/src/render.rs`
- `crates/cyril-ui/src/snapshots/cyril_ui__render__tests__theme_seam_picker.snap`

**Code (advisory):** No production code change is permitted; fixture construction
may reuse the existing picker state.

**Verification:**

- [ ] Unit and snapshot tests pass.
- [ ] Stress fixture reports 1,920/1,920 symbol/style matches against clean
      `HEAD`.
- [ ] Prove-it oracle still agrees with all compiled projection rows.
- [ ] Loop budget holds at 1,924 test visits.

## Plan self-review

### Every loop

- Source/true-color/no-color role mapping: `O(R) = 19` per on-demand
  resolution; within budget.
- ANSI-256 projection: `O(R × P256) = 4,320`; within budget.
- ANSI-16 projection: `O(R × P16) = 288`; within budget.
- Tie fixture: 256 distance evaluations; within budget.
- Syntax validation: two expected-`O(1)` test lookups; no production loop.
- Architecture fence: test-only `O(B) ≤ 300,000` character checks and at most
  16 file reads; within the 10^6-operation and 10^3-syscall limits.
- Render fixtures: 1,920–1,924 test visits each; no production loop.
- **Gaps:** none.

### Every stress fixture

- Fixed contract defeats missing roles, deduplication by color, named source
  colors, and extra reset roles.
- True-color fixture defeats channel arithmetic and reset-to-black conversion.
- ANSI-256 fixture defeats cube-only projection.
- ANSI-16 fixture defeats threshold/color-family heuristics.
- Tie fixture defeats last-wins and traversal-order bugs.
- No-color fixture defeats partial reset and retained syntax.
- Syntax fixture defeats unchecked typo fallback.
- Scope fixture defeats leaked configuration and premature widget wiring.
- Three render fixtures defeat default, diff/syntax, and modal style changes.
- **Gaps:** none.

### Every doc-comment precondition

No planned public doc comment imposes a caller precondition. Source validity is
load-bearing and enforced structurally by `SourceColor`; theme completeness is
enforced by fixed fields; palette ranges are private constants. No unenforced
precondition remains.

### Every write target

- Library resolution writes nothing.
- Test-binary probe rows are **data** and go to stdout between markers.
- Oracle agreement is **data** on stdout; disagreement is **diagnostic** on
  stderr with nonzero exit.
- Insta snapshot files are deterministic **test data**.
- Cargo/compiler messages remain diagnostics on stderr.
- **Gaps:** none.

### Every tracker reference

- Widget migrations: `cyril-ghuu`, `cyril-nrnq`, `cyril-dij8` — verified.
- Additional palettes: `cyril-fkke` — verified.
- Configuration, detection, and selection: `cyril-qaq0` — verified.
- Legacy palette contraction: `cyril-6r3a` — verified.
- Arbitrary operator-defined palettes are a settled rejection under ADR-0005.
- **Gaps:** none.

## Approval

The requester approved this 11-slice plan and the bounded Slice 1 dead-code
ledger on 2026-07-10. Strict warning-free enforcement resumes in Slice 2.
