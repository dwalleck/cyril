# Budgeted implementation plan — cyril-q9dx

## Preconditions

- Approved design: `.cyril-q9dx/design.md`, requester approval recorded
  2026-07-11.
- Cheapest falsifier: `.cyril-q9dx/design-falsifier-output.txt` records
  `C1 PASS user=LightBlue agent=LightGreen system=LightMagenta` from a command
  that launches the Rust probe against the actual resolver.
- Prototype agreement: 116/116 resolver rows and 3/3 runtime identity markers.

## Execution budget

**Planned slice count:** 3.

Each slice is expected to take 30–60 minutes of implementation and checkpoint
work. The plan adds no temporary compatibility layer and stays below both review
thresholds: 3 slices is below 12, and approximately 229 seconds of repeated gate
time is below four hours.

### Measured baseline on the planning machine

Measured 2026-07-11 against the current workspace with warm build artifacts.
These are planning evidence, not CI timing assertions.

<!-- markdownlint-disable MD013 -->

| Gate | Measured | Planned allowance per invocation |
| --- | ---: | ---: |
| Focused `cyril-ui` theme tests | 3.707s cold | 10s |
| Full `cyril-ui` tests | 0.360s warm | 10s |
| Live cheapest resolver falsifier | 0.210s warm | 10s |
| Detached release probe | 17.159s cold | 30s |
| Render oracle over captured runtime rows | 0.026s | 5s |
| `cargo fmt --all -- --check` | 0.325s | 2s |
| Workspace Clippy | 0.308s warm | 30s |
| Workspace tests | 1.931s warm | 15s |

<!-- markdownlint-enable MD013 -->

### Repeated-gate estimate

<!-- markdownlint-disable MD013 -->

| Category | Duration allowance | Invocations | Budgeted time | Reuse |
| --- | ---: | ---: | ---: | --- |
| Focused unit commands | 10s | 3 | 30s | Each command also executes its slice's stress fixture at the identical code state. |
| Stress commands | 0s additional | 0 additional | 0s | Exact-state reuse of the focused command; no fixture is inferred from a different run. |
| Independent probe/oracle commands | registry 2s; resolver 10s; renderer 5–10s | 11 total | 92s | No cross-slice reuse; each oracle consumes fresh output after that slice. |
| Manual budget measurement | 30s | 1 | 30s | Slice 2 includes the detached workspace's cold release build; no CI timing assertion. |
| Crate regression fence | 10s | 3 | 30s | Rerun after every slice. |
| Final formatting | 2s | 1 | 2s | No reuse. |
| Final workspace Clippy | 30s | 1 | 30s | No reuse. |
| Final workspace tests | 15s | 1 | 15s | No reuse. |
| **Total** | | **21 command invocations** | **229s (~3.8m)** | |

<!-- markdownlint-enable MD013 -->

Compilation after source edits may exceed the warm measurements, so allowances
are 1.7×–97× the observed values. Wall budgets below are checkpoint evidence;
none becomes a permanent elapsed-time assertion.

## Shared fixtures and oracles

- Existing semantic-role enumeration helper: `resolved_roles` in
  `crates/cyril-ui/src/theme.rs` tests.
- Existing prototype binaries: `.cyril-q9dx/probe.rs` and
  `.cyril-q9dx/render-probe.rs`.
- Existing signed prototype outputs: resolver and renderer TSV files under
  `.cyril-q9dx/`.
- Existing live cheapest falsifier: `.cyril-q9dx/design-falsifier.py`.
- Slice 1 adds an independent registry oracle; its macro-versus-`ALL`
  comparison is embedded in the Slice 2 acceptance oracle so C0 is rechecked
  after the Slice 2, Slice 3, and final integration source states.
- Slice 2 adds the role-aware production acceptance oracle shared by Slice 3
  and final integration.
- Slice 3 adds the expanded runtime render oracle used by final integration.

## Temporary states between slices

1. **After Slice 1:** production remains on universal nearest-RGB projection;
   only the authoritative registry and exhaustive observation seam exist. This
   is the currently supported behavior and is safe.
2. **After Slice 2:** resolver behavior fully satisfies C0, C1, C2, C3, C5, C6,
   and C7. Existing renderer consumers automatically receive the fixed fields;
   the focused multi-shape C4 regression fence is not yet present, but the
   prototype's committed-message runtime boundary still passes.
3. **After Slice 3:** all eight design claims have permanent regression fences.

No slice introduces a temporary default, adapter, duplicate resolver, or public
compatibility path.

## Claim coverage

<!-- markdownlint-disable MD013 -->

| Design claim | Owning slice |
| --- | --- |
| C0 exhaustive theme registry | Slice 1 |
| C1 fixed speaker slots | Slice 2 |
| C2 muted-family collision rejection | Slice 2 |
| C3 exact change surface | Slice 2 |
| C4 runtime identity bindings | Slice 3 |
| C5 geometric projection remains authoritative | Slice 2 |
| C6 deterministic resolution | Slice 2 |
| C7 syntax preservation | Slice 2 |

<!-- markdownlint-enable MD013 -->

## Slice 1: Establish an exhaustive bundled-theme registry

**Claim:** C0 — one macro invocation generates every `ThemeId` variant and the
unique, declaration-ordered `ThemeId::ALL` registry consumed by contract tests
and probes.

**Oracle:** A new `.cyril-q9dx/registry-oracle.py` independently extracts variant
names from the macro invocation, launches the compiled registry probe, and
compares names, order, count, and uniqueness item by item.

**Stress fixture:** Invoke the same parameterized declaration macro for a
three-variant test enum (`Alpha`, `Beta`, `Gamma`). Expected output is exactly
`Alpha,Beta,Gamma`, with 3 unique entries in declaration order. This fixture
fails a plausible manual-registry implementation that omits the middle variant
or duplicates an endpoint.

**Loop budget:**

- Production registry construction is a static slice with no runtime loop.
- Registry contract checks iterate `T` variants: `O(T)`, with planned bundled
  scale `T = 6` after `cyril-fkke`, for 6 visits.
- The compiled source probe may iterate `T × M × R`, where `T = 6`, `M = 4`
  modes, and `R = 29` roles: 696 visits, below `10^6` operations and with zero
  production syscalls.

**Wall budget:** N/A. This slice adds no always-on computation; registry and
probe loops run only in tests or acceptance tooling.

**Files:**

- Modify `crates/cyril-ui/src/theme.rs`.
- Create `.cyril-q9dx/registry-oracle.py`.
- Create `.cyril-q9dx/registry-oracle-output.txt` as captured gate evidence.

**Atomicity:** The enum declaration, generated registry, exhaustive test seam,
and independent oracle establish one representation invariant. Splitting them
would leave either an unobserved registry or an oracle with no authoritative
compiled surface. Production color behavior remains unchanged, so this slice is
revertible without a compatibility state.

**Gate reuse:** The focused registry test supplies both unit and stress evidence
because command, code state, three-variant fixture, and expected output are
identical. No evidence is reused after this slice; Slice 2 changes production
resolution and must rerun every oracle and regression gate.

**Code (advisory):**

```text
declare_bundled_theme_ids! {
    pub enum ThemeId { CyrilDark }
}

// Macro expansion owns both the enum variants and ThemeId::ALL.
// cyril_dark_source retains an exhaustive match over ThemeId.
```

**Output classification:** Probe rows and `C0 PASS ...` are data on stdout.
Source-parse, compile, or mismatch details are diagnostics on stderr.

**Doc-comment preconditions:** Variant uniqueness is load-bearing and enforced
by Rust's duplicate-variant compile error; registry inclusion is structural
because the same repetition generates `ALL`. No unenforced caller precondition
is added.

<!-- markdownlint-disable MD013 -->

**Verification:**

- [ ] Unit/stress command passes:
      `cargo test -p cyril-ui theme::tests::bundled_theme_registry_is_complete_and_unique -- --exact`
      and reports the three-variant fixture as `Alpha,Beta,Gamma`.
- [ ] Registry oracle passes:
      `python .cyril-q9dx/registry-oracle.py` with
      `C0 PASS themes=1 unique=1`.
- [ ] Live resolver prototype still agrees:
      `python .cyril-q9dx/design-falsifier.py` reports C1 PASS.
- [ ] Live renderer prototype is regenerated and agrees:
      `cargo run --quiet --manifest-path .cyril-q9dx/Cargo.toml --bin q9dx-render-probe > .cyril-q9dx/render-probe-output.tsv`, then
      `python .cyril-q9dx/render-oracle.py` reports 3/3 agreement.
- [ ] Loop budget holds: registry oracle reports `themes <= 6` for current
      planned bundled scale and source-probe visits `<= 696`.
- [ ] Crate regression passes: `cargo test -p cyril-ui`.
- [ ] `lens_diagnostics(mode="all")` reports no blocking errors in changed
      files.

<!-- markdownlint-enable MD013 -->

## Slice 2: Apply and validate role-aware ANSI-16 semantics

**Claim:** C1, C2, C3, C5, C6, and C7 — ANSI-16 resolution validates the four
muted-family roles, assigns the three fixed speaker slots, preserves geometric
projection elsewhere, remains deterministic, and retains syntax selection.

**Oracle:** A new `.cyril-q9dx/acceptance-oracle.py` launches compiled probes
rather than trusting captured rows. It runs the multi-theme `emit_source_probe`
twice, a 12-row synthetic collision probe, and the existing synthetic tie probe.
The source probe contains theme id, role, source RGB/reset, ANSI-256 output,
ANSI-16 output, and syntax selection. Before checking role rows, the oracle
independently extracts variants from the declaration macro and compares them
with compiled `ThemeId::ALL`, preserving the Slice 1 C0 oracle at the new code
state. The oracle then:

- emits a distinct C0 result for registry count, order, and uniqueness;
- expects indexes 12, 10, and 13 for user, agent, and system;
- checks all 12 collision rows against an independent role-slot set
  intersection and verifies the reported role/color;
- brute-forces the canonical 16-color table for 25 non-speaker RGB roles;
- brute-forces fixed xterm entries 16–255 for all 28 RGB ANSI-256 roles;
- checks the lower-index tie probe independently;
- verifies all 26 non-speaker ANSI-16 and 87 non-ANSI-16 role values are stable;
- byte-compares the two fresh source-probe runs for deterministic output;
- verifies each theme's syntax component survives ANSI-16 and no-color remains
  `None`.

The oracle prints distinct C0/C1/C2/C3/C5/C6/C7 sections so one failing claim
is localized without interpreting a combined boolean.

**Stress fixture:** A table-driven unit fixture sets each of `muted`, `border`,
`subdued`, and `diff_context` to each of LightBlue, LightGreen, and
LightMagenta: 12 invalid combinations. Every row must identify the injected role
and color before speaker assignment. A valid Cyril Dark row must report no
collision, preserve all four DarkGray muted-family outputs, and assign exactly
the three signed speaker slots. The fixture also retains the existing
`(64, 0, 0)` lower-index tie case so a semantic exception cannot weaken generic
palette math.

**Loop budget:**

- Production collision scan: `O(K)` with fixed `K = 4` muted-family roles, for
  exactly 4 membership checks per ANSI-16 resolution.
- Speaker assignment: `O(1)`, exactly 3 field writes.
- Exhaustive contract matrix: `O(T × M × R)` with `6 × 4 × 29 = 696` role
  visits at planned bundled scale.
- Independent palette oracle: `O(T × R_rgb × (P256 + P16))`, with
  `T = 6`, `R_rgb = 28`, `P256 = 240`, and `P16 = 16`, for
  `6 × 28 × 256 = 43,008` distance evaluations, below `10^6`.
- Stress collision matrix: `4 × 3 = 12` cases.
- The production path performs zero syscalls.

**Wall budget:** Warm total `resolve_ansi16` time must be at most 100µs per
resolution at `R = 29`; theme resolution occurs at startup or explicit theme
change, not per rendered frame. Measure 20 warmed batches of 5,000 resolutions
with `std::hint::black_box`, report median ns/resolution across the 100,000 total
calls, and record evidence without a CI elapsed-time assertion.

**Files:**

- Modify `crates/cyril-ui/src/theme.rs`.
- Modify `.cyril-q9dx/Cargo.toml` to register the budget probe binary.
- Create `.cyril-q9dx/resolution-budget.rs`.
- Create `.cyril-q9dx/acceptance-oracle.py`.
- Create `.cyril-q9dx/acceptance-probe-output.tsv` as compiled-row evidence.
- Create `.cyril-q9dx/acceptance-oracle-output.txt` as localized agreement
  evidence.

**Atomicity:** Collision validation, speaker assignment, exact change-surface
tests, geometric regression checks, syntax preservation, deterministic checks,
and the independent role-aware oracle define one supported resolver behavior.
Separating them would either ship semantic output without its contract gate or
install a rejecting validator before the fixed behavior exists. The public
resolver signature remains unchanged.

**Gate reuse:** The single focused `theme::tests::` command executes unit tests
and the 12-case stress matrix at one exact code state. The acceptance oracle
invokes a fresh compiled probe and also supplies loop-count evidence. No Slice 1
oracle result is reused because production resolver code changes here. No Slice
2 result is reused in Slice 3 because the test binary and renderer test surface
change there.

**Code (advisory):**

```text
resolve_ansi16(id):
    geometric = resolve_with(id, SourceColor::ansi16)
    collision = first protected slot used by a muted-family role
    reject the internal bundled-theme invariant if collision exists
    overwrite user, agent, and system with their fixed named slots
    return the otherwise unchanged Theme
```

**Output classification:** Compiled probe rows and per-claim `C* PASS` summaries
are data on stdout. Collision, parse, subprocess, and mismatch details are
diagnostics on stderr. The budget probe's ns/resolution measurement is data on
stdout.

**Doc-comment preconditions:** The muted-family exclusion is load-bearing: a
violation would silently erase speaker identity, so a release-surviving runtime
invariant check names and rejects the role. The finalizer has no undocumented
"already geometric" precondition that can silently corrupt output; it is
private, called once from `resolve_ansi16`, and repeated application is
idempotent for speaker fields.

<!-- markdownlint-disable MD013 -->

**Verification:**

- [ ] Unit/stress command passes:
      `cargo test -p cyril-ui theme::tests:: -- --test-threads=1 --nocapture`, including all
      12 role-slot collision rows, exact speaker slots, change surface,
      determinism, syntax preservation, and generic tie-breaking.
- [ ] Stress fixture reports 12/12 invalid combinations rejected with the exact
      injected role and color; valid Cyril Dark reports four DarkGray muted
      roles and three protected speaker assignments.
- [ ] Production acceptance oracle passes:
      `python .cyril-q9dx/acceptance-oracle.py` with distinct C0, C1, C2, C3,
      C5, C6, and C7 PASS sections for the independently checked registry and
      every `ThemeId::ALL` entry.
- [ ] Live render boundary is regenerated and still agrees:
      run `q9dx-render-probe` into `render-probe-output.tsv`, then
      `python .cyril-q9dx/render-oracle.py` reports 3/3 agreement.
- [ ] Budget probe passes checkpoint review:
      `cargo run --release --quiet --manifest-path .cyril-q9dx/Cargo.toml --bin q9dx-resolution-budget`
      reports warm total `<= 100µs/resolve`; no elapsed assertion is added to
      CI.
- [ ] Loop budget holds: acceptance oracle reports at most 43,008 distance
      evaluations and production validation reports fixed `K = 4`.
- [ ] Crate regression passes: `cargo test -p cyril-ui`.
- [ ] `lens_diagnostics(mode="all")` reports no blocking errors in changed
      files.

<!-- markdownlint-enable MD013 -->

## Slice 3: Fence every runtime speaker-identity consumer

**Claim:** C4 — visible committed user, committed agent, system, main streaming
agent, and subagent streaming agent markers use their resolved speaker roles in
an 80×24 Ratatui buffer across empty and Unicode message shapes.

**Oracle:** A new `.cyril-q9dx/acceptance-render-oracle.py` launches a compiled
runtime identity probe, independently extracts each relevant `chat.rs`
arm-to-role binding, compares fixed semantic expectations with actual
`TestBackend` cells, and emits one C4 result per path.

**Stress fixture:** A five-case table renders:

1. empty committed user text, expecting visible `You:` in LightBlue;
2. empty committed agent text, expecting visible `Kiro:` in LightGreen;
3. system text `系统 status`, expecting every visible non-space system glyph in
   LightMagenta;
4. non-empty main streaming agent text, expecting `Kiro:` in LightGreen;
5. non-empty subagent streaming text, expecting its dynamic agent-name label in
   LightGreen.

Each case uses 80×24 and locates markers by symbol rather than fixed coordinates,
so the fixture tests visibility/color without becoming a layout snapshot. It
fails missing streaming coverage, stale `state.theme()` use, hard-coded gray,
ASCII-only marker scanning, and the empty-content branch.

**Loop budget:**

- No production loop is added; Slice 3 adds regression and oracle code only.
- Runtime test inspection is `O(C × W × H)` with `C = 5`, `W = 80`, and
  `H = 24`: `5 × 80 × 24 = 9,600` cell visits, below `10^6`.
- Lexical oracle inspection is `O(S)` over one `chat.rs` source file of less
  than 2,000 lines and performs zero production syscalls.

**Wall budget:** N/A. Production rendering code is unchanged. Test/oracle time
is included in the focused and oracle gate allowances, with no CI timing
assertion.

**Files:**

- Modify only the `crates/cyril-ui/src/widgets/chat.rs` test module. If the
  runtime fixture disproves the approved C4 model, stop the slice and return to
  design rather than adding unplanned production edits.
- Create `.cyril-q9dx/acceptance-render-oracle.py`.
- Create `.cyril-q9dx/acceptance-render-output.tsv` as runtime cell evidence.
- Create `.cyril-q9dx/acceptance-render-oracle-output.txt` as per-path agreement
  evidence.

**Atomicity:** C4 crosses a distinct renderer/library boundary from resolver
projection. Its five rows belong together because splitting adjacent identity
paths would duplicate the same state, 80×24 fixture, buffer scanner, and oracle
while leaving a partially fenced renderer. A separate test-focused slice is
justified because it establishes the independent runtime boundary identified by
the prototype.

**Gate reuse:** The focused C4 command supplies unit and adversarial fixture
evidence at the same exact state. The expanded render oracle consumes fresh
runtime rows. Slice 2 resolver oracle and crate regression are rerun because the
test binary and source state change; no prior result is reused.

**Code (advisory):**

```text
for each identity-path fixture:
    render through chat::render into TestBackend(80, 24)
    locate the path's marker by symbols
    report marker, role, and foreground
compare runtime report with an independent source-binding oracle
```

**Output classification:** Runtime marker rows and per-path C4 PASS rows are data
on stdout. Missing markers, source-binding mismatches, and parse failures are
diagnostics on stderr.

**Doc-comment preconditions:** Marker visibility is conditional on a non-empty
rendered marker. User and agent have labels even with empty content; empty system
text has no cell and is explicitly represented by the resolver-level C1
contract rather than a false rendering precondition. No unenforced non-empty
contract is added.

<!-- markdownlint-disable MD013 -->

**Verification:**

- [ ] Unit/stress command passes:
      `cargo test -p cyril-ui widgets::chat::tests::ansi16_identity_consumers_use_speaker_roles -- --exact --nocapture`.
- [ ] Stress output reports 5/5 paths with the expected marker and named color,
      including Unicode system text and both empty-label branches.
- [ ] Expanded render oracle passes:
      `python .cyril-q9dx/acceptance-render-oracle.py` with five distinct C4 PASS
      rows.
- [ ] Production resolver oracle still agrees with the current binary:
      `python .cyril-q9dx/acceptance-oracle.py` reports all per-claim sections
      passing for every registered theme.
- [ ] Loop budget holds: runtime probe reports `cells_visited <= 9,600`.
- [ ] Wall budget is N/A with written reason: no production code or loop is
      introduced by this slice.
- [ ] Crate regression passes: `cargo test -p cyril-ui`.
- [ ] `lens_diagnostics(mode="all")` reports no blocking errors in changed
      files.

<!-- markdownlint-enable MD013 -->

## Final integration gate

Run after Slice 3 at the final exact code state; no prior result is reused:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python .cyril-q9dx/acceptance-oracle.py
python .cyril-q9dx/acceptance-render-oracle.py
```

Also run `lens_diagnostics(mode="all")` over every changed file. Completion
requires zero blocking diagnostics and all per-claim oracle sections passing.

## Verified tracker references

Verified against `.rivets/issues.jsonl` during planning:

<!-- markdownlint-disable MD013 -->

| ID | Why it appears in this plan |
| --- | --- |
| `cyril-fkke` | Defines the planned six-theme production scale: Cyril Dark plus five additional bundled palettes. |
| `cyril-qaq0` | Owns startup mode/theme activation; q9dx changes the resolver contract it consumes. |
| `cyril-x5xi` | Owns structural cache-key completeness; q9dx changes values already hashed by existing keys. |
| `cyril-xv3e` | Owns consolidation of projection and conversation-fixture plumbing; q9dx adds only claim-specific evidence. |

<!-- markdownlint-enable MD013 -->

No uncited implementation promise is moved outside these three slices.

## Plan Self-Review

1. **Every loop:** No gaps. Registry `O(T)`, contract `O(T × M × R)`, collision
   `O(4)`, palette oracle `O(T × R_rgb × 256)`, and render inspection
   `O(C × W × H)` are stated and remain below `10^6` operations at scale.
2. **Every fixture:** No gaps. Three-variant middle-entry omission, 12 muted-role
   collisions, preserved tie-breaking, empty identity labels, Unicode system
   text, and both streaming paths each target a named plausible bug.
3. **Every doc-comment precondition:** No gaps. Duplicate variants are enforced
   by the compiler; muted collision is load-bearing and survives release;
   empty-system visibility is not documented as a false precondition.
4. **Every write target:** No gaps. Probe/oracle/measurement rows are stdout
   data; mismatch and subprocess details are stderr diagnostics; TSV/TXT files
   are captured data artifacts.
5. **Every tracker reference:** No gaps. `cyril-fkke`, `cyril-qaq0`,
   `cyril-x5xi`, and `cyril-xv3e` exist and their descriptions cover the named
   adjacent work.
6. **Slice economy:** No gaps. Registry foundation, atomic resolver behavior,
   and the distinct runtime renderer boundary have independent rollback and
   falsification surfaces; no slice only adds another row to an existing
   fixture.
7. **Execution cost:** No gaps. Three slices and approximately 229 seconds of
   gate time stay below approval thresholds; no temporary compatibility layer
   exists.

The plan is ready for `checkpointed-build` review.
