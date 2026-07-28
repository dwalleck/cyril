# cyril-nd4h — budgeted plan

Design: `.cyril-nd4h/design.md` (approved 2026-07-27). Approved decisions:
split (honor `mouse_capture`, remove the other two); source-scan fences with
CRLF normalization; `StreamBuffer` left to **cyril-ell0**.

**Constraint discovered while planning:** `App` cannot be constructed in a test
today — `App::new` takes a `BridgeHandle` whose fields are private with no
constructor, and `app.rs`'s existing test module only exercises free functions
(`classify_notification_route`). Claims C1/C2 are behavioral and were approved
as "ordinary tests", so Slice 4 adds a minimal test-support seam rather than
downgrading them to source scans.

**Ordering note:** in `main.rs`, `App::new` (line 72) runs *before* the terminal
`execute!` (lines 79-82), so `app.mouse_captured()` is available at the point
the mouse mode is set. `EnableBracketedPaste` currently shares that one
`execute!` and must stay unconditional when the mouse arm becomes conditional.

---

## Slice 1: Remove the two dead config fields, fenced by a legacy-config test

**Claim:** C5 — a `config.toml` still naming the removed keys loads
successfully, with every surviving field taking its file-specified value.
**Oracle:** the TOML file's literal text (independent of the code path under
test).
**Stress fixture:** a legacy config carrying **all three** hazards at once —
the two removed keys, a key cyril never knew, **and `max_messages = 999`**.
The 999 is the load-bearing part: `load_from_path` swallows parse errors and
returns `Self::default()`, so asserting "a Config came back" passes under both
the accepting and the rejecting deserializer. Expected `max_messages == 999`
(parsed, unknowns ignored); `== 500` means it silently fell back and C5 is
false. Second case: a wrong-typed `mouse_capture = "yes"` must yield defaults,
not a panic.
**Loop budget:** no new loop.
**Wall budget:** n/a (not an always-on phase).
**Files:**
- `crates/cyril-core/src/types/config.rs` (drop 2 fields, their defaults, and
  their assertions in the existing test module)
- `crates/cyril-core/tests/nd4h_legacy_config_compat.rs` (new — promotes
  `.cyril-nd4h/falsifier-c5.py` from a one-shot into a permanent fence)

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (999, not 500)
- [ ] probe oracle still agrees (`probe2.py` → cache peak still 256)
- [ ] Budgets hold (no loop introduced)

---

## Slice 2: Fence the Ctrl+M toggle from both starting states

**Claim:** C4 — Ctrl+M toggles correctly from **either** starting state; the
first press always changes the mode.
**Oracle:** independently computed `!initial`, not the toggle's own return.
**Stress fixture:** drive the toggle from `false` (expect `true`) **and** from
`true` (expect `false`). The bug class: a fixture that only starts from the
current hardcoded default (`true`) cannot detect an inverted or
initial-state-ignoring toggle — which is precisely the failure the existing
`app.rs:80` comment warns about. Both directions, or the fence is vacuous.
**Loop budget:** no new loop.
**Wall budget:** n/a.
**Files:**
- `crates/cyril-ui/src/state.rs` (test module only — `UiState` is trivially
  constructible, so this claim needs no bridge)

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (both directions)
- [ ] Oracle agrees
- [ ] Budgets hold

---

## Slice 3: Wire `mouse_capture` through a single read

**Claim:** C1 (default stays on), C2 (`false` honored), C3 (terminal mode
derived from `App` state, not a second config read), C6 (a new `UiConfig` field
cannot compile without a consumption decision).
**Oracle:** the compiler for C6 (exhaustive destructure); source text for C3.
**Stress fixture:** deferred to Slices 4-5, which fence this slice's behavior;
this slice is the code change those fixtures run against. Its own check is that
`cargo check --all-features --all-targets` still passes with the destructure
carrying no `..`.
**Loop budget:** no new loop.
**Wall budget:** n/a.
**Files:**
- `crates/cyril/src/app.rs` (`App::new(bridge, ui: &UiConfig, cwd)`, exhaustive
  `let UiConfig { max_messages, mouse_capture } = *ui;`, set initial state from
  `mouse_capture`, add `pub fn mouse_captured(&self) -> bool`; `ui_state` stays
  private)
- `crates/cyril/src/main.rs` (pass `&config.ui`; split the `execute!` so
  `EnableBracketedPaste` stays unconditional while the mouse arm becomes
  `if app.mouse_captured()`)

**Code (advisory):**
```rust
// app.rs
pub fn new(bridge: BridgeHandle, ui: &UiConfig, cwd: PathBuf) -> Self {
    // Exhaustive on purpose (cyril-nd4h): no `..`. A new UiConfig field must
    // fail compilation here rather than join the ranks of the silently ignored.
    let UiConfig { max_messages, mouse_capture } = *ui;
    ...
    ui_state.set_mouse_captured(mouse_capture);
}

// main.rs -- ONE read of the config value, then derive.
crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
if app.mouse_captured() {
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
}
```

**Verification:**
- [ ] Unit tests pass
- [ ] `cargo check --all-features --all-targets` clean
- [ ] probe oracle still agrees
- [ ] Budgets hold

---

## Slice 4: Make `App` constructible in tests, and fence C1/C2

**Claim:** C1, C2 — the behavioral half of the wire-through.
**Oracle:** the TOML literal / `UiConfig::default()`, read independently of
`App`.
**Stress fixture:** three configs — `mouse_capture = false`, `= true`, and
**absent**. The bug class this defeats is a one-sided fence: a test that only
checks `false` passes under an implementation hardcoded to `false`, and a test
that only checks the default passes under today's hardcoded `true`. All three,
or the fence is one-sided. **C2's `false` case must fail against pre-Slice-3
code** — verify that by stashing Slice 3 and watching it go red; a fence that
passes before the fix is decoration.
**Loop budget:** no new loop.
**Wall budget:** n/a.
**Files:**
- `crates/cyril-core/src/protocol/bridge.rs` (`#[doc(hidden)] pub fn
  BridgeHandle::for_tests()` over dummy channels — test-only support, no
  correctness precondition, so `#[doc(hidden)]` is the whole enforcement)
- `crates/cyril/src/app.rs` (test module)

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (all three configs)
- [ ] C2 confirmed RED against pre-Slice-3 code
- [ ] Budgets hold

---

## Slice 5: Source-scan fences, CRLF-normalized

**Claim:** C3 (single read), C6 (no `..`), C8 (caches still 256), C9 (docs
match).
**Oracle:** source text — independent of anything the runtime does.
**Stress fixture:** the scanner must be proven able to *fail*. Feed it (a) a
synthetic buffer containing `config.ui.mouse_capture` inside a mock
`EnableMouseCapture` arm → must report a violation; (b) the same content with
`\r\n` line endings → must reach the identical verdict. Hazard (b) is not
hypothetical: **cyril-xi4a** was a P1 where exactly this class of scanner
reddened Windows CI on a CRLF checkout. Normalize with
`content.replace("\r\n", "\n")` before matching, and assert equality of the two
verdicts so the normalization itself is fenced.
**Loop budget:** `O(files × lines)`; production scale `files = 4`
(`main.rs`, `app.rs`, `highlight.rs`, `widgets/markdown.rs`) × `lines ≲ 3,000`
≈ **1.2×10⁴ ops**, plus 4 file reads. Two orders under the 10⁶ op / 10³ syscall
ceiling. Test-only, not an always-on phase.
**Wall budget:** n/a (not always-on).
**Files:**
- `crates/cyril/tests/nd4h_source_fences.rs` (new)

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (violation detected; LF and CRLF
      verdicts identical)
- [ ] Oracle agrees
- [ ] Loop budget holds (1.2×10⁴ ≪ 10⁶)

---

## Slice 6: Correct the documentation

**Claim:** C9 — docs name exactly the surviving fields with their true
effective values, and no longer describe `HashCache` as an "LRU".
**Oracle:** `grep` over the doc files, independent of the code.
**Stress fixture:** grep for `highlight_cache_size` and
`stream_buffer_timeout_ms` across `AGENTS.md` and `.agents/summary/` → must
return **zero** hits; grep for `LRU` near the cache row → zero hits. The bug
class: a change that edits `AGENTS.md:160` (the prose line) but forgets
`.agents/summary/codebase_info.md:55-57` (the table), which is a *separate*
file listing all three fields. Both surfaces or the claim is half-done.
**Loop budget:** no new loop (grep in the Slice 5 scanner).
**Wall budget:** n/a.
**Files:**
- `AGENTS.md` (line ~160)
- `.agents/summary/codebase_info.md` (lines ~55-57 — drop two rows, fix the
  third's description, drop "LRU")

**Verification:**
- [ ] Slice 5 scanner passes on the edited docs
- [ ] Stress fixture produces expected outcome (zero hits, both files)
- [ ] Oracle agrees
- [ ] Budgets hold

---

## Slice 7: Confirm the unchanged failure posture

**Claim:** C10 — malformed / unreadable config still falls back to defaults
rather than failing startup.
**Oracle:** `UiConfig::default()` literals, read independently.
**Stress fixture:** three inputs — syntactically invalid TOML, a wrong-typed
value (`mouse_capture = "yes"`), and a path that does not exist. Each must
yield defaults with no panic. The bug class: a refactor that "tightens" error
handling into `?` or `.expect()` and turns a warn-and-continue into a startup
crash — a regression a happy-path config test would never see. Extend the
existing `config.rs` load tests rather than duplicating them.
**Loop budget:** no new loop.
**Wall budget:** n/a.
**Files:**
- `crates/cyril-core/src/types/config.rs` (test module)

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (all three → defaults, no panic)
- [ ] Oracle agrees
- [ ] Budgets hold

---

## Slice 8: Final audit — no ignored fields remain

**Claim:** C7 — every `UiConfig` field has ≥1 production consumer; C8 — caches
still hold 256.
**Oracle:** `probe.py` (compiler rename-mutation) and `probe2.py` (behavioral
execution) — both independent of the test suite written in Slices 1-7.
**Stress fixture:** re-run `probe.py` unchanged. Every surviving field must
report `consumers > 0`. The bug class this catches: Slice 3 wires
`mouse_capture` into `App::new` but a later slice's refactor drops the
assignment — the destructure still compiles (the binding is just unused), so
C6's compiler fence stays green while the field silently goes dead again.
`probe.py` is the only check that would notice.
**Loop budget:** no new loop (probe re-run: 2 `cargo check` passes).
**Wall budget:** n/a (developer audit, not a shipped phase).
**Files:** none — verification only; results recorded in
`.cyril-nd4h/audit.md`.

**Verification:**
- [ ] `probe.py`: every field `consumers > 0`
- [ ] `probe2.py`: cache peak still 256
- [ ] Full gate green (`cargo nextest run`, clippy `-D warnings`, `fmt
      --check`, doctests), run with `--all-features` per cyril-ykkc
- [ ] Budgets hold

---

## Plan Self-Review

**1. Every loop.** One new loop in the entire plan — Slice 5's source scanner:
`O(files × lines)` = 4 × ≲3,000 ≈ 1.2×10⁴ ops and 4 syscalls, against ceilings
of 10⁶ / 10³. Test-only, not always-on. Slices 1-4 and 6-8 introduce no loops.
**No gaps.**

**2. Every fixture — and the bug class it fails under.**
| Slice | Bug class the fixture is designed to catch |
|---|---|
| 1 | Silent fallback to defaults masquerading as a successful parse (`999` vs `500`) |
| 2 | Inverted / initial-state-ignoring toggle, invisible if you only start from the default |
| 3 | (fenced by 4-5) `..` in the destructure |
| 4 | One-sided fence that passes under a hardcoded value; C2 verified RED pre-fix |
| 5 | CRLF divergence (cyril-xi4a, P1) + a scanner that cannot actually fail |
| 6 | Editing the prose file but forgetting the summary table |
| 7 | A refactor turning warn-and-continue into a startup crash |
| 8 | A dropped assignment that leaves the destructure compiling but the field dead |
**No happy-path-only fixtures. No gaps.**

**3. Every doc-comment precondition.** One introduced:
`BridgeHandle::for_tests()` (Slice 4). Classified **sanity hint, not
load-bearing for correctness** — it constructs dummy channels; misuse in
production yields a handle whose sends go nowhere, which no sane caller reaches
and which corrupts no data. Enforcement is `#[doc(hidden)]` plus the doc
comment. No `debug_assert!` needed, and no runtime check is warranted because
no output is silently wrong. **No gaps.**

**4. Every write target.** Slices 1-4 and 6-7 write only to source/doc files.
Slice 5's scanner and all tests emit through the test harness (diagnostic,
stderr on failure) — no `println!` to stdout that a downstream pipe would
consume. Slice 8 writes `.cyril-nd4h/audit.md` (audit-trail data, a file, not a
stream). Production behavior change is confined to one conditional
`crossterm::execute!` on stdout, which is terminal control, not pipeline data.
**No gaps.**

**5. Every tracker reference.** Three appear in this plan:
**cyril-ell0** (Slice header — dead `StreamBuffer`, out of scope; verified
present on `main`, chore/P3, `discovered-from cyril-nd4h`), **cyril-xi4a**
(Slice 5 — CRLF hazard; verified closed, P1), **cyril-ykkc** (Slice 8 —
mandates `--features kas` in the gate; verified open, P3). All three resolve to
issues whose content covers the thing cited. No uncited deferrals. **No gaps.**

**Claim coverage vs. design:** C1 (S3,S4) · C2 (S3,S4) · C3 (S3,S5) · C4 (S2) ·
C5 (S1) · C6 (S3,S5) · C7 (S8) · C8 (S5,S8) · C9 (S5,S6) · C10 (S7). All ten
design claims are covered; no slice implements a claim the design does not
list.

---

## Deviations during execution

The plan is a hypothesis; these are the places reality corrected it. Recorded
so the commit history and this document do not disagree.

1. **Slice order swapped (3 ↔ 4).** As written, Slice 3 had no fixture of its
   own and deferred to Slices 4-5 — but the per-slice gate requires running
   *this* slice's fixture. The test-support seam (`bridge.rs`) landed first
   instead, so the wire-through slice could carry its fixtures in the same
   commit. Every slice ended up ≤2 files regardless.

2. **Slices 5 and 6 merged, and reordered.** The scanner asserts the docs are
   already correct, so shipping it before the docs edit would have failed its
   own gate. Docs first, then the fence, in one commit.

3. **Three doc surfaces, not two.** The plan named `AGENTS.md` and
   `.agents/summary/codebase_info.md`; `.agents/summary/data_models.md` also
   listed the removed fields. The fixture's own rationale ("both surfaces or
   the claim is half-done") is what caught it — the count was just wrong.

4. **The scanner needed comment-stripping.** Its first run flagged `main.rs`'s
   own explanatory comment, which names the config field while not reading it.
   A fence that forces code to go undocumented in order to pass is shaping the
   source for the scanner's convenience, so the scan now judges code only.

5. **`probe.py` changed twice in Slice 8** — hardcoded field list made
   self-deriving, and `--all-targets` dropped after it produced a false pass
   that reported only test-file consumers. Both are recorded in `audit.md`.

6. **Slice 3's "unused `pub fn`" irony.** `BridgeHandle::for_tests` would have
   shipped as public API with no test — the exact disease this ticket treats —
   so it gained a fixture fencing its documented no-bridge-behind-it contract.
