# cyril-8tq6 — budgeted plan

Design: `.cyril-8tq6/falsifiable-design.md` (approved: emit `\\wsl$`; env+cwd
distro resolution; C9 manual fence accepted). Claims C1-C9. C1 (drive behavior
frozen) is verified in EVERY slice by the unmodified pre-existing 17 tests; C9
is the approved manual fence (PR body). Slices below cover C2-C8.

Shared budget notes: every new loop is a single pass over one path string
(len ≤ 4096 bytes ⇒ ≤ 10^4 ops/call) or the existing JSON-value recursion
(O(nodes), unchanged); no new syscalls in any translation path except the
one-time `env::var` + `current_dir` in slice 4's OnceLock init (2 syscalls,
once per process). Nothing approaches the 10^6-ops / 10^3-syscalls budget.
No new output streams: production diagnostics go through `tracing` (stderr),
no `println!` outside tests.

---

## Slice 1: POSIX→UNC core (`wsl_to_win_in`)

**Claim:** C2 (+ C5 forward half: distro `None` ⇒ passthrough).
**Oracle:** MS wslpath conformance rows under `\\wsl$` emission (probe 2 §F-C2',
already passed by the prototype; the slice's tests re-encode them against the
real fn) + probe 1 §A passthrough list for the `None` rows.
**Stress fixture:** the drive/internal boundary + shape zoo, expected outputs
written first:
| input | distro | expected |
|---|---|---|
| `/mnt/data/x` | `Ubuntu` | `\\wsl$\Ubuntu\mnt\data\x` (multi-char ⇒ NOT drive `d`!) |
| `/mnt` | `Ubuntu` | `\\wsl$\Ubuntu\mnt` |
| `/mnt/` | `Ubuntu` | `\\wsl$\Ubuntu\mnt\` |
| `/mnt/c/Users` | `Ubuntu` | `C:\Users` (drive branch still wins) |
| `/` | `Ubuntu` | `\\wsl$\Ubuntu\` |
| `/proc/1/` | `Ubuntu` | `\\wsl$\Ubuntu\proc\1\` |
| `/home/ü ser/f x.txt` | `Ubuntu` | `\\wsl$\Ubuntu\home\ü ser\f x.txt` |
| `rel/path`, `""` | `Ubuntu` | unchanged |
| `/home/u/f` | `None` | unchanged |
| `/home/u/f` | `Some("")` | unchanged (empty = unset, defensive) |
Bug classes targeted: branch-order (drive vs UNC), multi-char `/mnt/*`
misparsed as a drive, ASCII assumption, root/trailing separator loss.
**Loop budget:** one `str::replace('/', "\\")` pass, O(len) ≤ 4096 ops/call;
call sites = per fs callback (human-interaction rate) ⇒ ≪ 10^6.
**Wall budget:** n/a (not always-on).
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Precondition enforcement:** doc says distro, when `Some`, is non-empty —
LOAD-BEARING (empty would silently emit `\\wsl$\<nothing>\...`): runtime guard
treats `Some("")` as `None` with a `debug!` log (resolve_wsl_distro also never
produces it; belt + suspenders is 2 lines).
**Code (advisory):** new `pub fn wsl_to_win_in(path: &str, distro: Option<&str>) -> PathBuf`;
existing `wsl_to_win` body moves in unchanged as the drive branch; UNC branch
appended per the probe prototype. Existing `pub fn wsl_to_win` NOT yet re-wired
(slice 4) — this slice only adds the `_in` fn, so existing tests are untouched.

**Verification:**
- [ ] New unit tests (table above) pass
- [ ] 17 pre-existing tests pass unmodified (C1)
- [ ] `cargo nextest run -p cyril-core`, clippy `-D warnings`, fmt, doctests
- [ ] Budgets hold (inspection: single-pass, no syscalls)

---

## Slice 2: UNC→POSIX core (`win_to_wsl_in`) + round-trip

**Claim:** C3, C4 (+ C5 reverse half).
**Oracle:** MS wslpath from-Windows conformance rows (probe 1 §B) with
passthrough semantics (probe 2 §F-C3'); round-trip vs the jq-extracted capture
path list (probe 1 §C, 11/11).
**Stress fixture:** expected outputs written first:
| input | distro | expected |
|---|---|---|
| `\\wsl$\Ubuntu\home\u` | `Ubuntu` | `/home/u` |
| `\\wsl.localhost\Ubuntu\proc\stat` | `Ubuntu` | `/proc/stat` |
| `\\wsl$\Ubuntu/proc/stat` (fwd) | `Ubuntu` | `/proc/stat` |
| `\\wsl$\Ubuntu\proc/stat` (MIXED) | `Ubuntu` | `/proc/stat` |
| `\\wsl$\Ubuntu`, `\\wsl$\Ubuntu\` | `Ubuntu` | `/` |
| `\\wsl$\Ubuntu-other\foo`, `\\wsl$\UbuntuX\foo` | `Ubuntu` | unchanged (exact-segment guard) |
| `\\wsl$\` | `Ubuntu` | unchanged (blank segment) |
| `\\wsl$\Ubuntu\home\u` | `None` | unchanged |
| `\\server\share\f` | any | `//server/share/f` (LEGACY behavior preserved — existing `test_unc_path_not_mangled`) |
| `C:\Users\u` | any | `/mnt/c/Users/u` (drive branch untouched) |
Round-trip: 4 distinct capture paths + `/`, `/proc/1/` through
`win_to_wsl_in(wsl_to_win_in(p))` = p, and UNC→POSIX→UNC = canonical `\\wsl$` form.
Bug classes: naive string-prefix distro match, generic-UNC regression,
mixed-slash normalization, blank-segment panic (`rest[..0]`).
**Loop budget:** one `replace` + one `find` pass, O(len) ≤ 4096 ops/call. Same
call rate as slice 1.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Precondition enforcement:** same empty-distro guard as slice 1 (shared
helper or duplicated 2-liner — implementer's choice).
**Code (advisory):** `pub fn win_to_wsl_in(path: &Path, distro: Option<&str>) -> PathBuf`;
WSL-UNC interception BEFORE the existing generic branch; existing `win_to_wsl`
not yet re-wired (slice 4).

**Verification:**
- [ ] New unit tests (table + round-trip) pass
- [ ] 17 pre-existing tests pass unmodified (C1)
- [ ] Full gate (nextest, clippy -D, fmt, doctests)
- [ ] Budgets hold (inspection)

---

## Slice 3: distro resolution (`resolve_wsl_distro`)

**Claim:** C6.
**Oracle:** probe 2 §F-C6 table (7/7 passed by prototype); tests re-encode it
against the real fn.
**Stress fixture:** the 7-shape table from probe 2, PLUS adversarial rows
written first:
| env | cwd | expected |
|---|---|---|
| `Some("Ubuntu")` | `None` | `Some("Ubuntu")` |
| `None` | `\\wsl$\Debian\home\u` | `Some("Debian")` |
| `None` | `\\wsl.localhost\Debian\home\u` | `Some("Debian")` |
| `Some("Ubuntu")` | `\\wsl$\Debian\...` | `Some("Ubuntu")` (env wins) |
| `None` | `None` | `None` |
| `Some("")` | `C:\Users\u` | `None` (empty = unset) |
| `None` | `\\wsl$\Ubuntu` | `Some("Ubuntu")` (root, no tail) |
| `None` | `\\wsl$\` | `None` (blank segment) |
| `None` | `\\wsl$\Ubuntu/sub` (fwd tail) | `Some("Ubuntu")` |
| `Some(" Ubuntu ")` | `None` | `Some(" Ubuntu ")` — NO trimming; the env var is taken literally (documented; a wrong name degrades to passthrough, same as unset) |
Bug classes: precedence inversion (cwd before env), empty-env acceptance,
blank-segment panic, separator-kind assumption in cwd parsing.
**Loop budget:** 2 prefix-strips + 1 split, O(len(cwd)) ≤ 4096 ops, runs ONCE
per process.
**Wall budget:** n/a (one-shot at first translation).
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Precondition enforcement:** return type guarantees non-empty `String` (the
blank-segment row) — enforced by construction + test, no assert needed.
**Code (advisory):** `pub fn resolve_wsl_distro(env: Option<&str>, cwd: Option<&Path>) -> Option<String>`
per probe 2's `proto_resolve_distro`.

**Verification:**
- [ ] Table tests pass
- [ ] Full gate
- [ ] Budgets hold (one-shot)

---

## Slice 4: wiring + JSON heuristic

**Claim:** C7; plus re-pointing the existing pub fns (`wsl_to_win`,
`win_to_wsl`) at the `_in` cores with the process distro (completing C2-C5's
reach into `to_native`/`to_agent` without signature changes).
**Oracle:** for C7, serde_json equality against hand-written expected JSON
(independent of the translation fns); for the wiring, the C1 suite (Linux
behavior must be bit-identical: `process_wsl_distro()` is `None` on Linux by
`cfg!`, so every existing test doubles as a wiring no-op proof).
**Stress fixture:** JSON fixtures with expected output written first:
- WinToWsl: `{"path": "\\\\wsl$\\Ubuntu\\home\\u", "alt": "\\\\wsl.localhost\\Ubuntu\\x", "drive": "C:\\Users\\u", "keep": "\\\\server\\share"}`
  → path=`/home/u`, alt=`/x`, drive=`/mnt/c/Users/u`, keep UNCHANGED
  (generic UNC not translated by the JSON layer — matches existing
  `test_translate_json_unc_path_not_translated` posture).
- WslToWin with distro configured: `{"path": "/mnt/c/f", "content": "/etc/hosts is a file\n", "posix": "/home/u"}`
  → path=`C:\f`, content UNCHANGED, posix UNCHANGED (content-safety asymmetry).
- Nested arrays/objects of the above.
Bug classes: heuristic broadened to bare `/`-rooted strings (content
corruption), foreign-distro JSON strings translated, WinToWsl regression on
drive strings.
**Loop budget:** JSON recursion unchanged O(nodes); per-string prefix checks
O(len) — same class as existing heuristics.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Precondition enforcement:** `process_wsl_distro()` documented "always None on
non-Windows" — LOAD-BEARING for Linux no-op guarantee: enforced by `cfg!` in
code (compile-constant, not an assert) + slice 5's Linux no-op test.
**Code (advisory):** `fn process_wsl_distro() -> Option<&'static str>` via
`OnceLock<Option<String>>`, init = `cfg!(windows).then(|| resolve_wsl_distro(env, cwd)).flatten()`;
one-time `warn!` when a WSL-internal path passes through untranslated on
Windows (diagnostic → tracing/stderr); JSON WslToWin translate-eligibility for
strings stays `looks_like_wsl_mount_path` (drive-only) — WinToWsl gains
`looks_like_wsl_unc_path`.
**Impact analysis (semantics change of pub fns):** run
`tethys callers cyril_core::platform::path::wsl_to_win --lsp` (+ `win_to_wsl`,
qualified names, `tethys index` first) and confirm the caller set is exactly
{`to_native`/`to_agent`, `translate_paths_in_json`, tests} as grep found; any
extra caller gets read before the wiring lands.

**Verification:**
- [ ] JSON fixture tests pass; C1 suite passes unmodified
- [ ] tethys caller sweep matches grep (no unexamined callers)
- [ ] Full gate
- [ ] Budgets hold

---

## Slice 5: wiring integration fences (`tests/win_wsl_wiring.rs`)

**Claim:** C8.
**Oracle:** CI logs on BOTH runners (Linux runner proves the no-op arm; Windows
runner — repo has one, cyril-xi4a — proves the translating arm). The test
binary is a separate process, so OnceLock/env state cannot leak from other
tests.
**Stress fixture:** three fences, expected outcomes first:
1. (all OS) `to_native("/home/u")` == `/home/u` AND `to_agent("/home/u")` ==
   `/home/u` on Linux (`cfg!(not(windows))` arm) — the no-op guarantee.
2. (Windows only) `to_native("/mnt/c/x")` == `C:\x` — drive wiring through the
   real chain, distro-independent.
3. (Windows only) self-exec child: `Command::new(current_exe())` with
   `CYRIL_WSL_DISTRO=Ubuntu` env (safe `Command::env`, NOT `set_var` — the
   workspace forbids unsafe and Rust 2024 `set_var` is unsafe), filtered to the
   child test: asserts `to_native("/home/u")` == `\\wsl$\Ubuntu\home\u` and
   `to_agent(r"\\wsl$\Ubuntu\home\u")` == `/home/u`. Parent asserts child
   exit success + captures output on failure.
Bug classes: inverted `cfg!` gate, OnceLock init ordering, env read wired to
the wrong parameter (also excluded by types: env is `&str`, cwd is `&Path`).
**Loop budget:** none (assertions + one child spawn, test-only).
**Wall budget:** n/a (test binary; child spawn ≈ ms).
**Files:** `crates/cyril-core/tests/win_wsl_wiring.rs` (new).
**Precondition enforcement:** n/a (test code).
**Code (advisory):** child-test guarded by an env marker
(`CYRIL_8TQ6_WIRING_CHILD`); parent test cfg-gated `target_os = "windows"`.

**Verification:**
- [ ] Fence 1 passes locally (Linux)
- [ ] Fences 2-3 compile under `cargo check --all-targets` locally (execution
      proof lands on Windows CI in the PR run — checked at the CI-watch stage)
- [ ] Full gate
- [ ] Budgets hold (n/a)

---

## Slice 6: docs + module contract

**Claim:** none new — makes C2-C6 discoverable (CLAUDE.md "Path Translation"
section + `path.rs` module docs now describe UNC translation, `CYRIL_WSL_DISTRO`,
cwd derivation, and the passthrough default; the design's open-decision
outcomes are recorded where the next contributor will look).
**Oracle:** n/a (prose); reviewed against the design doc at the pre-PR review.
**Stress fixture:** n/a — docs-only slice, exempt per plan rules (no logic).
**Loop budget:** n/a. **Wall budget:** n/a.
**Files:** `CLAUDE.md`, `crates/cyril-core/src/platform/path.rs` (doc comments
only).

**Verification:**
- [ ] Full gate still green (doc comments compile: doctests)
- [ ] CLAUDE.md section matches shipped behavior (no aspirational claims)

---

## Plan Self-Review

1. **Loops:** slice 1 (one replace pass, O(len)≤4096/call), slice 2 (replace +
   find, O(len)), slice 3 (2 strips + split, O(len), once/process), slice 4
   (JSON recursion unchanged + O(len) per-string checks). All ≪ 10^6 ops; the
   only new syscalls are 2, once per process (slice 4 init). **No gaps.**
2. **Fixtures:** every logic slice names its bug classes (branch order,
   multi-char `/mnt`, exact-segment guard, blank-segment panic, precedence
   inversion, content corruption, cfg inversion) with expected outputs written
   before implementation. Docs slice exempt (no logic). **No gaps.**
3. **Preconditions:** non-empty distro = load-bearing → runtime guard
   (slices 1-2); resolve's non-empty return = by construction + test
   (slice 3); Linux-always-None = compile-constant `cfg!` + fence 1
   (slices 4-5). **No gaps.**
4. **Write targets:** production writes = `tracing` diagnostics only (stderr);
   test output via test harness. No new stdout writers. **No gaps.**
5. **Tracker refs:** cyril-trkw (auto-detect, verified this session),
   cyril-f2fv (terminal UNC cwd, verified), cyril-lwpm (auth store,
   pre-existing open). Claim coverage: C1 every slice, C2 s1, C3 s2, C4 s2,
   C5 s1+s2, C6 s3, C7 s4, C8 s5, C9 manual (approved). **No gaps.**
