# cyril-8tq6 — falsifiable design: WSL-internal path translation for a Windows host

## Purpose

On a Windows host running `wsl kiro-cli acp` against a WSL-native workspace, every
KAS `fs/*` host callback carries a WSL-internal POSIX path (`/home/...`, `/tmp/...`).
cyril's `platform::path::wsl_to_win` translates only `/mnt/<drive>` paths, so these
pass through unchanged and fail `-32603 NotFound` (probe Part A: **11/11 absolute
paths in the real 2.10.0 capture are WSL-internal** — the entire workspace, not an
edge case). The fix: translate WSL-internal paths to `\\wsl$\<distro>\...` UNC
paths (and the reverse), with an explicit, testable distro-resolution rule.

## What the probes established (`PROVE-IT.md`, `probe-output.txt`, `probe2-output.txt`)

- Prototype rule reproduces Microsoft's own `wslpath` conformance suite 16/16
  (canonical emission) and 5/5 (compat `\\wsl$` emission); oracle =
  microsoft/WSL `test/linux/unit_tests/wslpath.c`.
- Round-trip holds 11/11 on real capture paths.
- Distro resolution prototype passes all 7 input shapes.
- cyril never injects `wsl` itself → the distro name cannot come from cyril's
  own spawn; it needs an explicit source.
- `translate_paths_in_json` has zero production callers; the fs path goes
  `to_native_checked` → `to_native` → `wsl_to_win`.

## Architecture

All changes live in `cyril-core/src/platform/path.rs` (+ one Windows-gated
integration-test file). No signature changes to `to_native` / `to_agent` /
`to_native_checked` → **zero ripple** into `kas/{host_io,kiro_fs,terminal_io}`.

```
pub fn wsl_to_win_in(path: &str, distro: Option<&str>) -> PathBuf   // pure, testable everywhere
pub fn win_to_wsl_in(path: &Path, distro: Option<&str>) -> PathBuf  // pure, testable everywhere
pub fn resolve_wsl_distro(env: Option<&str>, cwd: Option<&Path>) -> Option<String>  // pure
fn wsl_distro() -> Option<&'static str>  // OnceLock glue; ALWAYS None on non-Windows (cfg!)
// existing pub fns become thin wrappers: wsl_to_win(p) = wsl_to_win_in(p, wsl_distro()), etc.
```

**Translation rule (to Windows):** drive translation (`/mnt/<letter>`) first,
byte-identical to today; any other `/`-rooted path with a configured distro →
`\\wsl$\<distro>` + tail with `/`→`\` (root keeps a trailing `\`); no distro →
passthrough (today's behavior) + a `warn!` (once) on Windows so the failure is
diagnosable (implementation detail, not a claim — see C5 note).

**Reverse rule:** strings starting `\\wsl.localhost\` or `\\wsl$\` with an
**exact** distro-segment match → POSIX tail (both slash kinds accepted; bare
`\\wsl$\<d>` → `/`). Foreign/colliding/empty segments and all other inputs:
unchanged (translation stays total — no new error paths in `to_agent`).

**Distro resolution (once per process, at first use):** non-empty
`CYRIL_WSL_DISTRO` env → else the process cwd, when it sits under a WSL UNC
prefix, donates its distro segment (cyril launched *from* the WSL-native
workspace — exactly the failing scenario) → else `None`. No `wsl.exe` query in
this PR (UTF-16LE output, localized `--status`, needs a real host to validate)
— that is **cyril-trkw**. Distro stays a plain non-empty `String` (guaranteed by
`resolve_wsl_distro`), not a newtype: it is process-lifetime config with a
single producer, not a domain identifier that travels through APIs.

**JSON layer:** `Direction::WinToWsl` heuristic additionally recognizes the two
WSL UNC prefixes (a string starting `\\wsl$\` is unambiguously a path).
`Direction::WslToWin` stays `/mnt/<drive>`-only: a bare `/`-rooted string in
JSON can be file *content* (`"content": "/etc/hosts is..."`), and blind
translation would corrupt writes. Asymmetry is deliberate and tested.

## Input shapes

`wsl_to_win_in` (POSIX→Win): drive `/mnt/c/...`; bare `/mnt` and `/mnt/`;
non-drive `/mnt/data` (multi-char ⇒ WSL-internal); root `/`; `/home|/tmp|/root`
tails; trailing-slash tail `/proc/1/`; relative; empty string; Unicode/space
segments; each × distro `Some`/`None`.

`win_to_wsl_in` (Win→POSIX): drive `C:\...`; `\\?\C:\...`; WSL UNC × {canonical,
compat} × {backslash, forward-slash, root-no-tail, root-trailing}; foreign
distro `Ubuntu-other`/`UbuntuX`; blank segment `\\wsl$\`; generic UNC
`\\server\share`; relative; each × distro `Some`/`None`.

`resolve_wsl_distro`: env {set, unset, empty} × cwd {WSL-UNC-compat,
WSL-UNC-canonical, UNC-root, drive, none} — 7 production-reachable cells
(probe 2 table).

Out of scope shapes: POSIX paths containing `\` or `:` characters (wslpath
PUA-escapes these; we don't — see Negative space); non-UTF-8 path bytes
(`to_string_lossy` boundary, pre-existing).

## Invariant sweep (step 2b)

The change is additive-with-behavior-change: two pub functions get *wider*
translation. Consumers of the newly-widened outputs:

| consumer | pre-change input | post-change input | verdict |
|---|---|---|---|
| `tokio::fs` read (`host_io.rs:34`) | `/home/...` (fails NotFound) | `\\wsl$\...` (openable via P9) | improvement; on-host proof = C9 (manual) |
| `write_atomic` canonicalize/tempfile (`host_io.rs:102`) | same | UNC dir; temp lives in target's own dir ⇒ no EXDEV | same manual bucket (C9); noted in related-issues.md |
| `terminal_io::create` cwd (`terminal_io.rs:105`) | nonexistent POSIX cwd (fails) | UNC cwd (program-dependent) | no regression; **cyril-f2fv** filed |
| `io_err` display | POSIX path | UNC path | cosmetic |
| `to_agent` of a `\\wsl$` cwd (`bridge.rs:1018`) | garbage `//wsl$/Ubuntu/home` | `/home/...` | behavior change is the *fix*; C3 fences it |

No serialization point, guard, ordering, or uniqueness property is removed; no
other reader assumes "to_native output is drive-or-unchanged" (grep: the only
consumers are the five above).

## Claims

1. **C1** Existing drive/`\\?\`/generic-UNC/relative behavior is byte-identical
   to main: all 17 existing `path.rs` tests pass unmodified.
2. **C2** With a configured distro, `wsl_to_win_in` maps every `/`-rooted
   non-drive path to `\\wsl$\<distro>\<tail>` per the MS conformance rows
   (root → trailing `\`; trailing separators preserved; `/mnt/data` → UNC).
3. **C3** `win_to_wsl_in` maps both-prefix, both-slash, root-form WSL UNC paths
   with an exact distro match to the POSIX tail, and passes foreign-distro /
   colliding-prefix / blank-segment inputs through unchanged.
4. **C4** Round-trip is identity over the 4 distinct capture paths and the
   conformance set (POSIX→UNC→POSIX, and UNC→POSIX→UNC modulo emitted prefix).
5. **C5** With distro `None`, every translation output equals its input
   (today's passthrough), byte-identical.
6. **C6** `resolve_wsl_distro` implements env-wins / cwd-derives / else-None
   with empty-env-as-unset, per the 7-shape table.
7. **C7** JSON `WinToWsl` translates WSL-UNC strings; JSON `WslToWin` never
   touches a bare `/`-rooted string (content safety).
8. **C8** Wiring: on Windows (CI), env-configured distro makes
   `to_native("/home/u")` yield `\\wsl$\<d>\home\u` and `to_agent` invert it;
   on Linux both remain no-ops for the same inputs.
9. **C9** On a real Windows+WSL host, a KAS fs read+write against a WSL-native
   workspace succeeds end-to-end (AC bullet 4).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | C1 drive behavior frozen | run existing 17 tests on new code | pre-change test suite (written against main) | 1m | passed (baseline) | existing `platform::path` tests, unmodified |
| 2 | C2 UNC emission conformant | MS conformance rows vs impl output | microsoft/WSL `wslpath.c` expectations | 5m | **passed (probe 1 §B 16/16, probe 2 §C2' 5/5)** | unit tests `wsl_internal_to_unc_*` |
| 3 | C3 reverse + exact-segment | conformance + foreign-distro rows | same + probe 2 §C3' passthrough 5/5 | 5m | **passed (probe)** | unit tests `unc_to_wsl_*`, `foreign_distro_passthrough` |
| 4 | C4 round-trip identity | capture paths through both directions | jq-extracted capture path list (11/11, probe 1 §C) | 5m | **passed (probe)** | unit test `roundtrip_wsl_internal_capture_paths` |
| 5 | C5 None ⇒ passthrough | capture + conformance inputs, distro None, diff vs input | probe 1 §A (11/11 PASSTHRU is today's behavior) | 5m | passed (probe A = current behavior) | unit test `no_distro_is_passthrough` |
| 6 | C6 resolution order | 7-shape table | probe 2 §F-C6 (7/7) | 5m | **passed (probe 2)** | unit tests `resolve_distro_*` |
| 7 | C7 JSON asymmetry | fixtures both directions incl. `/`-rooted content string | serde_json equality on untouched fields | 10m | pending (build) | unit tests `json_win_to_wsl_unc`, `json_wsl_to_win_ignores_bare_posix` |
| 8 | C8 cfg wiring | dedicated integration-test process, Windows CI + Linux assert | CI logs on both OS runners (repo has Windows CI — cyril-xi4a) | rides CI | pending (build) | `tests/win_wsl_wiring.rs` |
| 9 | C9 on-host end-to-end | live KAS session on Windows+WSL, WSL-native workspace | the file appears/reads back in WSL | needs real host | pending | **manual** — requires explicit approval (below) |

Cheapest falsifiers (rows 2-6) ran before this design was presented — all passed.

Non-vacuity (buggy impl each fence kills): row 1 — UNC branch checked before
drive branch (`/mnt/c` → `\\wsl$\d\mnt\c`); row 2 — forward-slash tail or
dropped root separator; row 3 — naive `strip_prefix("\\\\wsl$\\Ubuntu")`
string-prefix match (accepts `Ubuntu-other`); row 4 — prefix emitted without
separator normalization; row 5 — hardcoded fallback distro when unset; row 6 —
cwd checked before env, or empty env accepted; row 7 — heuristic broadened to
any `/`-rooted string (content corruption); row 8 — `cfg!` gate inverted or
OnceLock initialized after first translation. Each claim has its own named
test(s) → failures localize.

## Open decisions (for the design pause)

1. **Emission prefix** — recommend **`\\wsl$`** (works on every WSL2 system
   incl. Win10 inbox WSL; `\\wsl.localhost` needs store-WSL/Win11). Both always
   accepted inbound. Alternative: canonical `\\wsl.localhost` (matches modern
   `wslpath` output).
2. **Distro sources** — recommend env `CYRIL_WSL_DISTRO` + cwd-derivation only
   (no `wsl.exe` query — cyril-trkw). A `--wsl-distro` CLI flag is a trivial
   follow-up if wanted; env suffices for V1.
3. **C9 fence = manual** — CI has no WSL (GitHub runners can't nest it), so the
   on-host AC bullet cannot be a CI test. Needs your explicit OK per the
   design rules.
4. **Foreign-distro reverse = passthrough + warn**, not an error (MS `wslpath`
   errors). Rationale: the translation layer stays total; erroring inside
   `to_agent` would add a failure path to cwd translation that today cannot fail.
5. **JSON `WslToWin` stays narrow** (no bare-POSIX translation) — content-safety
   rationale above.

## Negative space (deliberately not done)

- **No `wslpath`-style PUA escaping** of `\`/`:` in filenames (U+F05C/U+F03A).
  Shared with the existing drive translation since inception; failure mode for
  such filenames is today's honest NotFound. Boundary, not deferred work.
- **No `wsl.exe` invocation, ever, in this PR** — no new subprocess, no UTF-16
  parsing, no localization risk (auto-detect is cyril-trkw).
- **No case-insensitive distro matching** — exact match only, as in MS's own
  conformance tests; a mismatched-case env value falls to passthrough, same
  failure mode as unset.
- **No `_kiro/fs/*` dialect coverage** — cyril answers bare-ACP `fs/*` only
  (cyril-kf2g owns the paginated dialect); this change is beneath that layer
  anyway (`to_native_checked` serves both).
- **No terminal-cwd special-casing** — same translation applies; Windows-host
  verification is cyril-f2fv.

## Deferrals (all tracked)

- cyril-trkw — auto-detect default distro via `wsl.exe`/registry (filed this run).
- cyril-f2fv — verify `terminal/create` UNC cwd on a real Windows host (filed this run).
- cyril-lwpm — WSL-aware auth-store path (pre-existing, open); this PR's UNC
  helpers are its likely substrate but it is untouched here.
