# Budgeted plan: Semantic conversation colors

Status: approved with execution amendment (2026-07-11)

The requester approved execution by replying `proceed` on 2026-07-11 after the
checkpoint preflight surfaced ordering, oracle, latency-budget, estimate, and
dirty-worktree concerns.

## Execution amendment

This amendment overrides conflicting slice text below without changing any
design claim, stress expectation, or regression fence.

1. **Execution order:** run Slices 1–4, then baseline Slices 15A–15D, then
   Slices 5–14, 15E, and 16–21. Slices 5–11 consume baseline artifacts and may
   not begin before all four baseline scenes exist.
2. **Compiled oracle:** every checkpoint extracts `BEGIN_THEME_PROBE` through
   `END_THEME_PROBE` from the compiled `cyril-ui` test binary and compares its
   rows with `.cyril-ghuu/projection-oracle.py`. Theme and no-color emitters run
   in separate `cargo test ... --exact --nocapture` invocations; combining them
   permits Rust's parallel test runner to interleave marker-delimited stdout.
   The lexical probe/oracle remains a separate source-migration oracle; it does
   not substitute for compiled evidence. Renderer slices additionally run their
   compiled Ratatui buffer fixture against the pinned baseline or marker oracle
   named by that slice.
3. **Budgets:** the approved specification explicitly introduces no latency
   target. Therefore the sub-millisecond wall figures below are observational,
   not release gates. Checkpoints enforce the stated operation, candidate,
   field-read, cell-visit, cache-operation, display-cap, byte, and syscall
   budgets. No slice may add an unplanned always-on loop.
4. **Time estimates:** Slices 1–4 are estimated at 45 minutes each; 15A–15D at
   90 minutes each; 5–14 and 15E–19B at 75 minutes each; and 20–21 at 90 minutes
   each. A slice halts if elapsed implementation time exceeds twice its class
   estimate.
5. **Working tree:** the requester authorized proceeding with the disclosed
   dirty tree. Slice commits stage only their named files and execution
   artifacts. Existing unrelated changes in `.cyril-ixua/probe.rs`,
   `.pi-subagents/`, and `docs/kiro-agent-schema-2.8.1-kas-0.3.257.md` remain
   unstaged. The pre-existing rustfmt-only `theme.rs` diff is absorbed by Slice
   1 because that slice necessarily rewrites the same test regions.

## Planning basis

This plan implements all 12 claims in the approved
`.cyril-ghuu/design.md`. The cheapest design falsifier passes with 13 required
legacy colors and zero missing values. The ticket-start 81-row inventory is
frozen in `.cyril-ghuu/legacy-color-baseline.tsv` so migration cannot make the
coverage oracle vacuous.

At every checkpoint, the prove-it comparison writes current results to temporary
files rather than overwriting the frozen baseline:

```text
python .cyril-ghuu/probe.py > target/cyril-ghuu-probe-current.tsv
bash .cyril-ghuu/oracle.sh > target/cyril-ghuu-oracle-current.tsv
diff -u target/cyril-ghuu-oracle-current.tsv \
  target/cyril-ghuu-probe-current.tsv
python .cyril-ghuu/cheapest-falsifier.py
```

The first `diff` may contain fewer rows as migration proceeds, but it must remain
empty because the independent mechanisms must agree on the current source. The
coverage falsifier must continue to read the frozen ticket-start inventory and
report `missing=0`.

## Slice 1: Add the first five compatibility roles

**Claim:** Design claim 1 begins with fixed source and resolved fields for
emphasis, tertiary accent, quaternary accent, quinary accent, and subdued.

**Oracle:** The signed role table supplies exact RGB values; the frozen legacy
inventory independently proves that each value is required.

**Stress fixture:** Resolve a source with all five values adjacent and distinct,
including `#000080`, `#008080`, `#800080`, and `#808080`; swapping any two fields
must produce a role-labeled mismatch. Expected: 24/24 intermediate roles resolve
and the five new labels match exactly.

**Loop budget:** No new production loops. Fixed-field construction and
projection dispatch add 5 constant-time assignments; scale is one theme.

**Wall budget:** Not always-on; source resolution occurs at state construction
or explicit selection. Added work is O(5) and below 10 µs in a focused release
benchmark.

**Files:** `crates/cyril-ui/src/theme.rs`

**Code (advisory):** Add five fields exhaustively to `SourceTheme`, `Theme`,
source construction, resolution, and role inspection; intermediate tests pin 24
roles so the slice compiles independently.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture produces 24/24 roles and five exact labeled values
- [ ] Prove-it probe/oracle agree on current source and coverage reports `missing=0`
- [ ] Loop and wall budgets hold at one-theme fixture scale

## Slice 2: Complete the 29-role contract

**Claim:** Design claim 1 completes with subdued-positive, subdued-negative,
soft-accent, positive-accent, and inset-background fields, yielding exactly 29
roles.

**Oracle:** `.cyril-ghuu/cheapest-falsifier.py` parses the frozen 81-row legacy
inventory and independently compares its 13 canonical values with the signed
role tables.

**Stress fixture:** Use the rejected 26-role value set as the negative control,
then the 29-role set. Expected: the negative control reports four missing base
ANSI values; production reports `required=13 ... missing=0` and 29/29 labeled
roles.

**Loop budget:** No new production loops. Five more constant-time field
assignments produce 29 fixed roles total.

**Wall budget:** Not always-on; added O(5) resolution work remains below the
Slice 1 10 µs budget.

**Files:** `crates/cyril-ui/src/theme.rs`,
`.cyril-ghuu/cheapest-falsifier.py`

**Code (advisory):** Finish the fixed-field expansion and add a compiled
negative-control contract fixture without changing the frozen baseline input.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture rejects 26 roles and accepts 29/29 roles with `missing=0`
- [ ] Prove-it probe/oracle agree on current source and the frozen coverage oracle passes
- [ ] Loop and wall budgets hold at one-theme fixture scale

## Slice 3: Prove all explicit projections

**Claim:** Design claim 2 projects 28 RGB roles correctly in both ANSI modes and
resets 29/29 roles with no syntax component in no-color mode.

**Oracle:** A new Python oracle parses the signed 29-role table, independently
generates xterm 16–255 and canonical ANSI-16 palettes, and brute-forces minimum
squared distance with lower-index ties.

**Stress fixture:** Include exact palette hits, grayscale-nearest values,
duplicate RGB roles, lower-index ties, and the reset canvas. Expected: 56/56
ANSI projections agree, all 29 no-color roles are reset, and syntax is `None`.
A projector that skips grayscale must fail at least the muted/subdued family.

**Loop budget:** Production projection keeps existing O(R×P): ANSI-256 is
28×240 = 6,720 candidate checks; ANSI-16 is 28×16 = 448; both are below 10^6
and run only when resolving a theme. The Python oracle performs the same bounded
7,168 checks independently.

**Wall budget:** Not always-on; each explicit resolution must remain below 2 ms
at R=28 in a release test.

**Files:** `crates/cyril-ui/src/theme.rs`,
`.cyril-ghuu/projection-oracle.py`

**Code (advisory):** Extend exhaustive test emission to all 29 roles and compare
compiled output with the independent script.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture reports 56/56 projections, 29 resets, and no syntax
- [ ] Prove-it probe/oracle and the independent projection oracle agree with compiled output
- [ ] 7,168 candidate checks and the 2 ms resolution wall budget hold

## Slice 4: Put one resolved theme in renderer state

**Claim:** Design claim 3 stores Cyril Dark true-color in `UiState` and exposes
one copyable resolved theme through `TuiState` and `MockTuiState`.

**Oracle:** Direct comparison with pure
`resolve(ThemeId::CyrilDark, ColorMode::TrueColor)` independently defines the
production default; a synthetic marker theme verifies the mock adapter.

**Stress fixture:** Construct production and mock states, with the mock theme
differing in every role. Expected: production equals Cyril Dark true-color,
mock returns all marker fields unchanged, and `TuiState` remains object-safe.
A default trait method that silently resolves Cyril Dark must fail the mock
assertion.

**Loop budget:** No new loops. One 29-field `Copy` per accessor call is O(29) =
constant, 29 field copies.

**Wall budget:** Always-on accessor overhead is budgeted at ≤1 µs per frame at
30 frames/s; state construction retains the Slice 3 ≤2 ms resolution budget.

**Files:** `crates/cyril-ui/src/traits.rs`,
`crates/cyril-ui/src/state.rs`

**Code (advisory):** Add a required read-only trait method rather than a default
resolver so tests and production are two real adapters at the seam.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture distinguishes production and all-marker mock themes
- [ ] Prove-it probe/oracle agree and compiled default matches the pure resolver
- [ ] O(29) copy and ≤1 µs accessor wall budgets hold

## Slice 5: Thread one theme through chat message rendering

**Claim:** Design claims 3 and 10 route the frame's copied theme through chat
identity, thought, plan, command, steer, and activity rendering without changing
non-color output.

**Oracle:** Hand-pinned marker colors identify each message/status role, while
baseline symbol/modifier rows independently define labels, italics, bolding,
spinner text, and ordering.

**Stress fixture:** One compact chat contains every non-tool `ChatMessageKind`,
all four steer statuses, and all six activities across parameterized draws.
Expected: every colored span uses its mapped marker; symbols and modifiers match
baseline. A helper that resolves its own theme or maps applied steer to agent
identity must fail distinctly.

**Loop budget:** No new loops; existing message traversal remains O(M) with
M≤500 configured messages. Theme threading adds O(1) field reads per colored
span, at most 2,000 reads for this fixture and below 10^6.

**Wall budget:** Always-on added overhead is ≤0.5 ms for M=500 in an 80×24
release render; full existing render cost is not re-budgeted.

**Files:** `crates/cyril-ui/src/render.rs`,
`crates/cyril-ui/src/widgets/chat.rs`

**Code (advisory):** Read `state.theme()` once in `draw_inner`, pass `&Theme` to
chat, and keep tool helpers on legacy colors until Slice 6.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture covers every non-tool message, steer status, and activity
- [ ] Prove-it probe/oracle agree on the reduced current inventory
- [ ] O(M), 2,000-read, and 0.5 ms added-overhead budgets hold

## Slice 6: Migrate tool, diff, and output colors

**Claim:** Design claims 4 and 10 map every tool kind/status, diff tag, optional
output, error, and truncation path to the frame theme while preserving content.

**Oracle:** The signed legacy-to-semantic table supplies marker roles; baseline
fixtures supply tool icons, labels, line numbers, diff prefixes, and limits.

**Stress fixture:** Eight tools cover all kinds and all four statuses, with
missing/present paths, commands, old diff text, outputs, errors, zero/non-zero
exit codes, 21 diff lines, and 6 output lines. Expected: every branch has the
mapped marker, diff truncates at 20, output at 5, and no symbol/order changes.
Using bright status roles instead of subdued compatibility roles must fail color
labels while leaving content labels passing.

**Loop budget:** No new loops. Existing work is O(T + D + O), bounded in the
fixture by T=8, displayed D≤20, displayed O≤5; under M≤500, rendering remains
below 10^6 displayed-span operations.

**Wall budget:** Always-on added role lookups are ≤0.5 ms at M=500 and the
existing truncation limits.

**Files:** `crates/cyril-ui/src/widgets/chat.rs`

**Code (advisory):** Pass `&Theme` only into tool helpers that select colors;
do not alter label or truncation logic.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture covers 8 kinds, 4 statuses, optional data, and both limits
- [ ] Prove-it probe/oracle agree on the reduced current inventory
- [ ] O(T+D+O), display-limit, and 0.5 ms budgets hold

## Slice 7: Migrate message-input colors

**Claim:** Design claims 3, 4, and 11 render cursor, border, and title from the
single frame theme while preserving multiline input geometry.

**Oracle:** Marker theme values independently identify primary text, subdued,
and quinary accent; existing input buffer geometry defines symbols and rows.

**Stress fixture:** Render empty, ASCII multiline, Unicode multiline, cursor at
start/middle/end, and cursor beyond byte length. Expected: no panic for
production-valid boundaries, identical rows, and exactly the three mapped role
colors. Hard-coded white cursor or bright cyan title must fail.

**Loop budget:** No new loops. Existing split/line traversal is O(B) for input
bytes B; stress B=100 KiB gives ≤100,000 character/byte operations, below 10^6.

**Wall budget:** Always-on added work is three role reads and ≤0.1 ms at B=100
KiB in a release widget render.

**Files:** `crates/cyril-ui/src/render.rs`,
`crates/cyril-ui/src/widgets/input.rs`

**Code (advisory):** Pass the already-copied frame theme; do not query state a
second time or change cursor normalization.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture preserves all input rows and three marker mappings
- [ ] Prove-it probe/oracle agree on the reduced current inventory
- [ ] O(B), 100 KiB, and 0.1 ms added-overhead budgets hold

## Slice 8: Migrate autocomplete colors

**Claim:** Design claims 3, 4, and 11 render suggestion names, descriptions, and
selection background from the same frame theme without changing its 10-row
window.

**Oracle:** Marker roles independently identify primary, muted, soft accent,
subdued, and inset background; existing expected visible labels define window
position.

**Stress fixture:** Use 21 suggestions with duplicate labels, Unicode, spaces,
present/absent descriptions, and selected values `None`, 0, 10, 20, and 999.
Expected: at most 10 rows, safe out-of-range behavior, correct center window,
and exact marker roles. Reusing user-message or code-background roles must fail.

**Loop budget:** Existing render traversal is O(min(S,10)); S=21 and S=10,000
both render at most 10 entries. Added role work is ≤50 field reads.

**Wall budget:** Always-on added overhead is ≤0.1 ms at S=10,000 because only 10
entries render.

**Files:** `crates/cyril-ui/src/render.rs`,
`crates/cyril-ui/src/widgets/suggestions.rs`

**Code (advisory):** Pass the frame theme into rendering only; `height_for`
remains theme-independent.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture preserves the 10-row window for all selection shapes
- [ ] Prove-it probe/oracle agree on the reduced current inventory
- [ ] O(min(S,10)), 50-read, and 0.1 ms budgets hold

## Slice 9: Add themed syntax adapters and cache identity

**Claim:** Design claims 7 and 9 add explicit themed block/line highlighting,
complete-theme cache keys, and primary-text fallback while retaining a temporary
default wrapper for existing Markdown callers.

**Oracle:** Syntect's packaged catalog independently supplies colored token RGB;
marker `theme.text` supplies `None`, missing-catalog, and `Err` fallback output.

**Stress fixture:** Render identical Rust under true-color then no-color and in
reverse order; also inject catalog `None` and highlighter `Err`. Expected:
colored runs contain catalog RGB, fallback/no-color runs contain only the marker
or reset, and order never changes output. A content/language-only key must fail.

**Loop budget:** No new content loops. Cache hashing adds O(R)=29 field hashes
per block. At 500 visible code blocks, 14,500 field hashes remain below 10^6.

**Wall budget:** Always-on added hashing/fallback dispatch is ≤0.5 ms for 500
cached blocks in an 80×24 release render.

**Files:** `crates/cyril-ui/src/highlight.rs`

**Code (advisory):** Add private `Option`/`Result` normalization helpers for
failure injection; the temporary wrapper must be removed in Slice 12.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture passes colored-first, no-color-first, `None`, and `Err`
- [ ] Prove-it probe/oracle agree and Syntect/catalog expectations match compiled output
- [ ] 14,500-hash and 0.5 ms budgets hold

## Slice 10: Add theme-aware Markdown and migrate prose constructs

**Claim:** Design claims 4, 8, and 11 add a themed Markdown entry point whose
cache key includes the complete theme and migrate headings, lists, quotes,
links, tables, rules, and inline code.

**Oracle:** Marker roles define each explicit prose color; baseline lines define
text, modifiers, table widths, and wrapping independently.

**Stress fixture:** Render one document containing headings 1–6, nested lists,
Unicode quote, repeated links, a duplicate-value table, inline code, and rule at
widths 0, 1, 79, 80, 120, and 121 under two marker themes. Expected: cache
outputs differ only by mapped colors and all text/geometry is identical.

**Loop budget:** Existing parser/layout remains O(N + R×C log C) for N Markdown
events, R table rows, C columns. Stress N=10,000, R=1,000, C=20 gives roughly
10,000 + 20,000×log2(20) ≈96,500 comparison/visit operations, below 10^6.
Theme hashing adds 29 operations per cache lookup.

**Wall budget:** Always-on added theme hashing and role lookup is ≤0.5 ms for the
stress document in release mode; existing Markdown layout time is unchanged.

**Files:** `crates/cyril-ui/src/widgets/markdown.rs`

**Code (advisory):** Keep code-block legacy colors and the old default entry
point temporarily; isolate cache keys by complete resolved theme immediately.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture preserves geometry across six widths and two marker themes
- [ ] Prove-it probe/oracle agree on the reduced current inventory
- [ ] O(N+R×C log C), ~96,500-operation, and 0.5 ms budgets hold

## Slice 11: Migrate Markdown code blocks and syntax calls

**Claim:** Design claims 4, 7, 8, and 11 route code background, borders,
language badge, syntax component, and fallback through the themed Markdown and
highlight interfaces.

**Oracle:** Baseline code-block geometry and Syntect catalog output are
independent of the role mapping; marker themes define code, subdued, quinary,
and primary-text colors.

**Stress fixture:** Render fenced Rust, unknown language, absent language, an
empty block, a 500-column line, Unicode code, and injected missing syntax at
widths 0, 7, 80, 120, and 200. Expected: border cap 120, background fills width,
no panic, correct fallback, and no stale cache colors.

**Loop budget:** Existing syntax work is O(L) in highlighted bytes and padding is
O(W) per emitted line. Stress L=100 KiB and W=200 stays below 10^6 byte/cell
operations; no nested theme-dependent loop is added.

**Wall budget:** Always-on added role selection is ≤0.5 ms at L=100 KiB; Syntect
runtime itself is existing behavior and separately bounded by current tests.

**Files:** `crates/cyril-ui/src/widgets/markdown.rs`,
`crates/cyril-ui/src/highlight.rs`

**Code (advisory):** Replace every code-block palette or named color and call
the themed highlighter; retain wrappers only until their callers move.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture passes five widths and six syntax/code shapes
- [ ] Prove-it probe/oracle and Syntect/catalog oracle agree with compiled output
- [ ] O(L+W×lines), 100 KiB/200-column, and 0.5 ms budgets hold

## Slice 12: Remove temporary highlighting defaults

**Claim:** Design claims 3 and 7 leave no syntax entry point that independently
resolves a default theme.

**Oracle:** LSP references and compiler errors independently prove that every
production caller supplies `&Theme`; the marker-theme syntax test proves the
value is observed.

**Stress fixture:** Search production references after deleting wrappers, then
compile all targets. Expected: zero calls to a no-theme highlight entry point
and marker syntax remains visible. A leftover default wrapper is a distinct
source-fence failure.

**Loop budget:** No new loops; deletion removes constant-time wrapper work.

**Wall budget:** Always-on syntax dispatch cannot increase; expected added wall
cost is 0 µs.

**Files:** `crates/cyril-ui/src/highlight.rs`

**Code (advisory):** Delete transitional wrappers and rename themed functions to
the approved small interface if that reduces caller complexity.

**Verification:**

- [ ] Unit tests and all-target compilation pass
- [ ] Stress fixture reports zero production no-theme highlight calls
- [ ] Prove-it probe/oracle agree on current source
- [ ] No loops are added and wall cost does not increase

## Slice 13: Connect chat to themed Markdown and remove its default

**Claim:** Design claims 3, 4, and 8 make committed and streaming agent Markdown
use the frame theme, with no Markdown entry point that resolves a second theme.

**Oracle:** A marker theme with unique prose and syntax values must appear in
both committed and streaming chat; LSP references independently enumerate all
Markdown callers.

**Stress fixture:** Render the same Markdown once committed and once streaming,
under marker then no-color themes with warm caches. Expected: identical symbols,
distinct requested colors, no stale values, and zero no-theme Markdown calls.
A chat helper retaining the default wrapper must fail the streaming or committed
label separately.

**Loop budget:** No new loops. Existing chat traversal O(M) and Markdown parsing
O(N) remain; theme propagation adds one reference per Markdown call, at most
M=500.

**Wall budget:** Always-on added propagation is ≤0.1 ms at M=500; removing the
wrapper avoids duplicate resolution.

**Files:** `crates/cyril-ui/src/widgets/chat.rs`,
`crates/cyril-ui/src/widgets/markdown.rs`

**Code (advisory):** Move both chat call sites together, delete the default
Markdown wrapper, and keep one explicit themed interface.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture distinguishes committed/streaming and marker/no-color paths
- [ ] Prove-it probe/oracle agree on current source
- [ ] O(M+N), M=500, and 0.1 ms budgets hold

## Slice 14: Install the static migration fence

**Claim:** Design claim 4 permanently rejects palette access and hard-coded
displayed colors in all five production modules, with only the signed conversion
bodies allowed in highlight.

**Oracle:** Rustfmt validates parseability; an independent raw-source lexical
scanner supplies the expected token inventory per module.

**Stress fixture:** Feed the audit helper synthetic modules containing a palette
import, `.fg(Color::White)`, a legal Syntect RGB conversion, and a legal diff
RGB conversion. Expected labels: first two rejected, last two accepted. Running
against production must report five clean module labels.

**Loop budget:** Source audit is O(B) over five production files. Current total
is below 100 KiB; stress at 1 MiB performs ≤1,048,576 byte visits, justified as
a test-only phase, with exactly 5 file reads and no production cost.

**Wall budget:** Not always-on; CI-only source audit budget is ≤1 s for 1 MiB.

**Files:** `crates/cyril-ui/tests/conversation_theme_sources.rs`

**Code (advisory):** Use explicit function-body exceptions rather than global
token-count exceptions so adding one hardcode while removing another cannot
cancel out.

**Verification:**

- [ ] Unit/integration tests pass
- [ ] Stress fixture rejects both bug snippets and accepts both conversion snippets
- [ ] Prove-it probe/oracle agree and production reports five clean labels
- [ ] O(B), 5-syscall, and ≤1 s CI budgets hold

## Slice 15A: Establish baseline normalization and the message scene

**Claim:** Design claim 5 first captures the scoped message scene from the exact
ticket-start commit with canonical named-ANSI normalization.

**Oracle:** A shell generator creates an isolated worktree at
`80f3ffa5a7ced20e33c9b98c782c08af704407d5`, injects a temporary cfg-test
harness, and runs the baseline crate; the current renderer cannot produce
expected cells.

**Stress fixture:** Normalize base/bright named colors, RGB, indexed, and the
rendered default; absent style colors and explicit reset both become `DEFAULT`
because Ratatui has collapsed them to `Color::Reset` in `Buffer::Cell`. Generate
the message scene twice. Expected: canonical table rows pass, both 1,920-cell
outputs are byte-identical, and the header contains the pinned commit. Treating
base cyan as bright cyan must fail distinctly.

**Loop budget:** O(W×H)=1,920 cell visits. One temporary worktree lifecycle, one
Cargo invocation, and one streamed write use fewer than 20 orchestration
syscalls outside Cargo.

**Wall budget:** Not always-on; generation budget is ≤60 s with warm workspace
dependencies.

**Files:** `.cyril-ghuu/generate-baseline.sh`,
`crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv`

**Code (advisory):** Put the temporary Rust harness in the shell heredoc and
start with normalization plus the compact message/activity scene only.

**Verification:**

- [ ] Generator self-checks and baseline message-scene test pass
- [ ] Stress fixture produces two byte-identical 1,920-cell outputs
- [ ] Prove-it probe/oracle and frozen baseline coverage oracle pass
- [ ] 1,920-cell, <20 orchestration-syscall, and ≤60 s budgets hold

## Slice 15B: Add the baseline tool scene

**Claim:** Design claim 5 extends the pinned fixture with the compact all-tool
scene without changing the message scene.

**Oracle:** The isolated ticket-start worktree renders all 8 tool kinds, 4
statuses, diff tags, and output paths; the prior message bytes are a frozen
prefix.

**Stress fixture:** Regenerate twice with a 21-line diff and 6-line output.
Expected: 3,840 total cells, byte-identical runs, unchanged message prefix, and
visible baseline truncation at 20/5. Omitting one tool kind must fail its label.

**Loop budget:** O(2×W×H)=3,840 cell visits; tool display remains capped at 20
diff and 5 output lines. Orchestration syscall count remains below 20.

**Wall budget:** Not always-on; generation remains ≤60 s with a warm build.

**Files:** `.cyril-ghuu/generate-baseline.sh`,
`crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv`

**Code (advisory):** Add only tool-scene construction and append its normalized
cells to the existing combined format.

**Verification:**

- [ ] Baseline message and tool scene tests pass
- [ ] Stress fixture produces stable 3,840-cell output and unchanged prefix
- [ ] Prove-it probe/oracle and frozen baseline coverage oracle pass
- [ ] 3,840-cell, truncation, syscall, and ≤60 s budgets hold

## Slice 15C: Add the baseline Markdown scene

**Claim:** Design claim 5 extends the pinned fixture with all signed Markdown
constructs and syntax fallback while preserving the first two scenes.

**Oracle:** The isolated ticket-start renderer and Syntect catalog produce the
Markdown cells; existing fixture bytes independently lock prior scenes.

**Stress fixture:** Include all constructs, fenced Rust, unknown language,
Unicode, and width-boundary content in one 80×24 scene. Expected: 5,760 total
cells, deterministic output, unchanged first 3,840 cells, and both syntax and
fallback labels present.

**Loop budget:** O(3×W×H)=5,760 normalized cells. Fixture Markdown stays below
10 KiB, so parser visits remain below 10,000; orchestration syscalls stay below
20.

**Wall budget:** Not always-on; generation remains ≤60 s with a warm build.

**Files:** `.cyril-ghuu/generate-baseline.sh`,
`crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv`

**Code (advisory):** Add only the compact Markdown scene and its presence
self-checks.

**Verification:**

- [ ] All three baseline scene tests pass
- [ ] Stress fixture produces stable 5,760-cell output and unchanged prefix
- [ ] Prove-it probe/oracle and Syntect catalog checks pass
- [ ] Cell, parser, syscall, and ≤60 s budgets hold

## Slice 15D: Complete the baseline with input and autocomplete

**Claim:** Design claim 5 completes the immutable four-scene fixture with
multiline input, cursor, and autocomplete suggestions.

**Oracle:** The isolated pinned renderer produces the fourth scene; the first
5,760 fixture cells remain an immutable prefix.

**Stress fixture:** Use multiline Unicode input, mixed suggestion descriptions,
duplicate labels, and an in-window selection. Expected: exactly 7,680 cells,
stable repeated generation, unchanged prior prefix, visible cursor, and 10 or
fewer suggestion rows.

**Loop budget:** O(4×W×H)=7,680 cell visits; suggestion rendering is
O(min(S,10)) with S=21. Orchestration syscalls stay below 20.

**Wall budget:** Not always-on; generation remains ≤60 s with a warm build.

**Files:** `.cyril-ghuu/generate-baseline.sh`,
`crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv`

**Code (advisory):** Add the fourth scene, final cell-count check, and exact
commit header check; generator data goes to the fixture and diagnostics stderr.

**Verification:**

- [ ] All four baseline scene tests pass
- [ ] Stress fixture produces two identical 7,680-cell fixtures
- [ ] Prove-it probe/oracle and frozen baseline coverage oracle pass
- [ ] 7,680-cell, suggestion, syscall, and ≤60 s budgets hold

## Slice 15E: Fence migrated rendering against the baseline

**Claim:** Design claim 5 requires zero normalized symbol, modifier, foreground,
or background differences between migrated true-color scenes and Slices
15A–15D.

**Oracle:** The immutable fixture comes from the isolated pinned commit; the
migrated side is produced only by the current compiled renderer.

**Stress fixture:** Compare against a deliberately bright-cyan migrated cell and
a one-modifier mutation before production. Expected: scene-specific failures
for color and modifier, followed by 0/7,680 differences for production.

**Loop budget:** Comparison is O(S×W×H)=7,680 cells with constant-time field
comparison.

**Wall budget:** CI-only comparison budget is ≤1 s for 7,680 cells.

**Files:** `crates/cyril-ui/src/render.rs`

**Code (advisory):** Enforce the fixture's pinned-commit header with a
release-active runtime assertion; a debug assertion is insufficient.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture detects both mutations, then reports 0/7,680
- [ ] Prove-it probe/oracle and frozen baseline coverage oracle pass
- [ ] 7,680-cell and ≤1 s CI budgets hold

## Slice 16: Exercise the four-mode scene matrix

**Claim:** Design claim 6 renders 16/16 scoped scene-mode combinations from
resolved roles or syntax output, with zero non-reset colors in no-color.

**Oracle:** The independent Python projection output supplies allowed UI colors;
Syntect's catalog supplies allowed token RGB values.

**Stress fixture:** Inject one hard-coded cyan cell and one RGB syntax cell into
each mode classifier. Expected: hardcoded UI cyan is rejected in all modes;
syntax RGB is accepted in true-color/ANSI modes and rejected in no-color. The
production matrix reports 16 distinct passes and four no-color zero counts.

**Loop budget:** O(M×S×W×H) = 4 modes×4 scenes×80×24 = 30,720 cell visits;
allowed-set lookup is O(1) average with at most 29 UI colors.

**Wall budget:** CI-only mode matrix budget is ≤2 s for 30,720 cells.

**Files:** `crates/cyril-ui/src/render.rs`

**Code (advisory):** Reuse the scoped scene builders from Slice 15; do not render
chrome or modal widgets owned by `cyril-dij8` and `cyril-nrnq`.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture distinguishes UI hardcodes, legal syntax RGB, and no-color
- [ ] Prove-it probe/oracle and independent projection oracle agree with compiled output
- [ ] 30,720-cell and ≤2 s CI budgets hold

## Slice 17: Fence every chat and tool input shape

**Claim:** Design claim 10 preserves symbols, modifiers, ordering, optional-data
behavior, and limits for every chat/activity/tool shape.

**Oracle:** Pinned baseline shape rows from the ticket-start commit are compared
without color fields, independently of the themed renderer's role choices.

**Stress fixture:** Parameterize all 8 message kinds, 4 steer statuses, 6
activities, 8 tool kinds, 4 statuses, all optional data present/absent, diff
21 lines, and output 6 lines. Expected: one labeled pass per shape and exact
20/5 truncation limits. Deleting one match arm must fail only its label.

**Loop budget:** Fixture generation is O(K) over fewer than 100 shape cases;
each rendered diff/output remains capped at 20/5, for below 10,000 span visits.

**Wall budget:** CI-only shape matrix budget is ≤2 s; production loops are
unchanged from Slices 5–6.

**Files:** `crates/cyril-ui/src/widgets/chat.rs`

**Code (advisory):** Prefer parameterized fixtures and distinct failure labels
over one large disjunctive assertion.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture reports every shape separately and exact truncation limits
- [ ] Prove-it probe/oracle agree on current source
- [ ] <10,000 visits and ≤2 s CI budgets hold

## Slice 18: Fence Markdown input shapes

**Claim:** Design claim 11 preserves all enumerated Markdown constructs, widths,
Unicode content, and empty behavior.

**Oracle:** Baseline text/geometry fixtures supply lines, widths, modifiers, and
panic outcomes while deliberately ignoring color fields.

**Stress fixture:** Combine all Markdown constructs with duplicate table widths,
Unicode, empty text, 100 KiB code, and widths 0, 1, 7, 79, 80, 120, 121, and
200. Expected: one label per construct/width, no panic, and identical geometry.
A theme change that styles raw text must fail a modifier/style-presence label.

**Loop budget:** Existing O(N + R×C log C + W×lines) work uses N≤10,000,
R=1,000, C=20, W≤200, and ≤1,000 lines: approximately 296,500 visits, below
10^6.

**Wall budget:** CI-only matrix budget is ≤3 s in release mode; no new
production loop is introduced.

**Files:** `crates/cyril-ui/src/widgets/markdown.rs`

**Code (advisory):** Extend existing rstest coverage; do not duplicate the color
compatibility oracle from Slice 15.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture covers every construct and eight width boundaries
- [ ] Prove-it probe/oracle agree on current source
- [ ] ~296,500 visits and ≤3 s CI budgets hold

## Slice 19A: Fence message-input shapes

**Claim:** Design claim 11 preserves input behavior for empty, multiline,
Unicode, spaces, cursor boundaries, and cursor-beyond-length shapes.

**Oracle:** Baseline symbol/geometry buffers independently define cursor rows,
text placement, and panic outcomes.

**Stress fixture:** Use 100 KiB Unicode multiline input with cursor at start,
middle, end, and beyond length. Expected: identical rows, no panic for
production-valid UTF-8 boundaries, and unchanged cursor placement.

**Loop budget:** O(B) with B=100 KiB = 102,400 visits, below 10^6; no new loop is
introduced.

**Wall budget:** CI-only matrix budget is ≤1 s; production theme overhead remains
within Slice 7's budget.

**Files:** `crates/cyril-ui/src/widgets/input.rs`

**Code (advisory):** Keep UTF-8 cursor-boundary ownership in `UiState`; do not
add a contradictory widget precondition or silent repair path.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture preserves all 100 KiB input rows and cursor shapes
- [ ] Prove-it probe/oracle agree on current source
- [ ] 102,400-visit and ≤1 s CI budgets hold

## Slice 19B: Fence autocomplete shapes

**Claim:** Design claim 11 preserves autocomplete behavior for empty, duplicate,
Unicode, description, cardinality, and selection-window shapes.

**Oracle:** Baseline geometry independently defines panel height, visible labels,
description text, and selected-row position.

**Stress fixture:** Build 10,000 suggestions with duplicates, Unicode, spaces,
and mixed descriptions; select `None`, first, middle, last, and 999. Expected:
rendering remains capped at 10 rows, center-scroll labels match, empty consumes
0 rows, and no selection shape panics.

**Loop budget:** Fixture construction is O(S)=10,000 test-only; production render
is O(min(S,10)) and visits at most 10 entries, below 10^6.

**Wall budget:** CI-only matrix budget is ≤1 s; production theme overhead remains
within Slice 8's budget.

**Files:** `crates/cyril-ui/src/widgets/suggestions.rs`

**Code (advisory):** Extend existing selection-window tests with distinct labels
rather than one disjunctive assertion.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture preserves 0/10-row boundaries at S=10,000
- [ ] Prove-it probe/oracle agree on current source
- [ ] 10,000-construction/10-render visits and ≤1 s CI budgets hold

## Slice 20: Prove Markdown cache resilience

**Claim:** Design claims 8 and 12 preserve 256-entry capacity, insertion-order
eviction, deterministic theme identity, mutex serialization, and
compute-on-lock-failure for Markdown.

**Oracle:** A local ledger independently predicts oldest-half eviction; direct
uncached marker rendering defines expected output after lock failure.

**Stress fixture:** Insert 256 unique themed documents, insert the 257th, repeat
a key, issue 8 threads×100 alternating-theme reads, and poison an injectable
local mutex. Expected: oldest 128 evicted, newer 129 retained, no duplicate-order
drift, 800 correct thread results, and poisoned path returns correct uncached
output without panic.

**Loop budget:** Cache stress is O(C + T×Q): C=257, T=8, Q=100, plus one
oldest-half eviction loop of 128; total 1,185 operations, below 10^6. Production
lock syscalls are one lock attempt per render call, unchanged.

**Wall budget:** Always-on added theme hashing remains ≤0.5 ms for 500 cache
lookups; CI stress budget is ≤2 s.

**Files:** `crates/cyril-ui/src/widgets/markdown.rs`

**Code (advisory):** Extract only a private injectable cache helper needed to
exercise lock failure; no public cache interface or capacity setter.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture reports `EVICTION`, `CONCURRENT`, and `POISON` separately
- [ ] Prove-it probe/oracle agree and uncached marker oracle matches compiled output
- [ ] 1,185-operation, one-lock-per-call, and wall budgets hold

## Slice 21: Prove highlight cache resilience

**Claim:** Design claims 9 and 12 preserve 256-entry capacity, insertion-order
eviction, deterministic theme identity, mutex serialization, and
compute-on-lock-failure for highlighted blocks.

**Oracle:** A local insertion ledger predicts eviction; Syntect catalog output
and no-color reset independently define alternating-theme results.

**Stress fixture:** Insert 256 unique code/language/theme keys, cross capacity,
repeat a key, run 8 threads×100 colored/no-color reads, and poison an injectable
local mutex. Expected: oldest 128 evicted, 800 mode-correct results, and poisoned
path computes correct output without panic.

**Loop budget:** O(C + T×Q + C/2) = 257 + 800 + 128 = 1,185 cache operations.
Highlighted fixture lines are one short line each, keeping syntax work below
10,000 byte visits. Production lock count remains one attempt per block.

**Wall budget:** Always-on added hashing remains ≤0.5 ms for 500 cached blocks;
CI stress budget is ≤2 s.

**Files:** `crates/cyril-ui/src/highlight.rs`

**Code (advisory):** Reuse the private cache-helper shape from Markdown only if
that does not add a public seam or touch a third file; otherwise keep the helper
local.

**Verification:**

- [ ] Unit tests pass
- [ ] Stress fixture reports `EVICTION`, `CONCURRENT`, and `POISON` separately
- [ ] Prove-it probe/oracle agree and Syntect/no-color oracle matches compiled output
- [ ] 1,185-operation, byte-visit, lock-count, and wall budgets hold

## Claim coverage

- Claim 1: Slices 1–2
- Claim 2: Slice 3
- Claim 3: Slices 4–5, 7–9, 12–13
- Claim 4: Slices 5–14
- Claim 5: Slices 15A–15E
- Claim 6: Slice 16
- Claim 7: Slices 9, 11–12
- Claim 8: Slices 10–11, 13, 20
- Claim 9: Slices 9, 21
- Claim 10: Slices 5–6, 17
- Claim 11: Slices 7–8, 10–11, 18, 19A–19B
- Claim 12: Slices 20–21

All 12 approved design claims have at least one completing slice and one
claim-specific stress fixture.

## Plan boundaries and verified tracker references

- Theme configuration and selection remain with verified `cyril-qaq0`.
- Additional palettes remain with verified `cyril-fkke`.
- Modal and chrome migration remain with verified `cyril-nrnq` and
  `cyril-dij8`.
- Global palette removal remains with verified `cyril-6r3a`.
- Responsive 60×16 layout remains with verified `cyril-a14l`.
- Navigation and undo/redo remain with verified `cyril-4vvw`.

Each issue was re-read in Rivets on 2026-07-11 and its description covers the
named work.

## Doc-comment contract review

- The baseline fixture's pinned-commit header is load-bearing for correctness;
  Slice 15 requires a release-active runtime assertion that rejects a wrong
  header.
- Width zero, empty content, absent suggestions, missing syntax, and poisoned
  cache locks are supported inputs, not preconditions.
- UTF-8 cursor boundaries remain an existing `UiState` correctness invariant;
  this plan adds no new unenforced widget doc comment about them.
- No other planned doc comment states a caller precondition. If implementation
  introduces one, the slice stops until it classifies and enforces it according
  to the skill rule.

## Output-stream review

- Probe, lexical oracle, coverage falsifier, and projection oracle outputs are
  **data** and go to stdout; parse failures and diagnostics go to stderr.
- The normalized baseline fixture is **data** written to its explicit fixture
  path; generation diagnostics go to stderr.
- Rust test failure messages are **diagnostics** handled by the test harness.
- Production rendering writes only to Ratatui buffers; no stdout/stderr writes
  are added.

## Plan self-review

1. **Loops:** No unbudgeted new loop remains. Fixed role work is O(29),
   projection is 7,168 checks, source audit is O(bytes), cell checks are at most
   30,720 visits, cache stresses are 1,185 operations each, and touched existing
   loops have explicit production or fixture scales below 10^6. The test-only
   1 MiB source scan is explicitly justified and uses 5 syscalls.
2. **Fixtures:** Every slice names a plausible bug and an adversarial input:
   swapped fields, rejected 26-role contract, grayscale/ties, hidden default
   resolver, all variants, boundaries, Unicode, stale caches, hard-coded color,
   bright/base confusion, capacity crossing, concurrency, and poisoned locks.
3. **Doc comments:** One load-bearing fixture-header precondition has a runtime
   check. Supported edge inputs are not documented as preconditions. No gap
   remains.
4. **Write targets:** All script/fixture data and diagnostics are classified;
   production adds no process-stream writes. No gap remains.
5. **Tracker references:** All seven cited Rivets IDs exist and cover their
   named work. No uncited deferral or invisible follow-up remains.

## Approval

The requester approved this plan with the execution amendment above by replying
`proceed` on 2026-07-11.
