# route.md — cyril-k8wc: KAS `shell_type` reply must name a concrete POSIX shell

- Change: rivets `cyril-k8wc` (P1 bug, labels: kas, kiro-2-32-0, ready-for-agent); blocks cyril-auxt.
- Date: 2026-10-18
- Branch: `fix/cyril-k8wc-shell-type`
- Request source: the ticket's description, fix direction and acceptance criteria (`rivets show cyril-k8wc`), plus the dispatching session's scope note (reply with a shell identifier KAS recognises for the shell cyril actually runs; keep fish/powershell/cmd behaviour; reported dialect must match the execution interpreter).

## Route tests

### T1 Empirical premise — NO

The one external fact the design depends on — which `shellType` strings KAS accepts and what it does with them — is answered by the shipped KAS source, extracted for both supported releases in [`receipts/kas-shell-type-mapper.txt`](receipts/kas-shell-type-mapper.txt):

- Normaliser (`FNe` 0.66.34 / `DMe` 0.66.43): basename, lowercase, strip `.exe`, strip a leading `-`.
- Dialect mapper (`cb` / `jh`, byte-identical logic): `bash|logbash|gitbash|wsl|zsh|logzsh|sh|dash|ksh|ash|mksh|pdksh|oksh|loksh|yash|busybox|tcsh|csh` → `posix`; `fish` → `fish`; `powershell|pwsh|pwsh-preview` → `powershell`; `cmd` → `cmd`; else `unknown`. The literal `posix` is not an input.
- System-prompt label (`hB` / `QB`): for a posix dialect KAS shows the reply's basename (`zsh`, `bash`, …); for `unknown` it shows the generic word `shell`.
- Tool selection (`Z9` / `B5`, isWindowsShell): `execute_pwsh` only for dialect cmd/powershell — unchanged by this fix for every reply cyril can send.
- `unknown` → `policy.shell.type.unknown` warning and, per the 2.32.0 audit (ticket resource `experiments/conductor-spike/audit-2.32.0/kas/FINDINGS.md`), `ask` for every non-denied command with `persistableConsent:false`.

The live before/after run in this change is acceptance evidence, not an unresolved premise.

### T2 Structural module shape — NO

All change is inside `crates/cyril-core/src/protocol/kas/host_shell.rs` (private `ShellKind`, `pub(crate) HostShell::wire_name`) and its test pin in `terminal_io.rs`. No public interface, schema, seam, dependency direction or ownership change: the registry stays the sole owner of the one startup-resolved `HostShell` that both answers `shell_type` and executes `terminal/create` (cyril-6bol's structural agreement is preserved). The private `ShellKind::Posix` variant gains the concrete POSIX shell it was classified from, so the reported name cannot diverge from the interpreter cyril runs.

### T3 Production-scale risk — NO

A string chosen once per process at startup. No latency, throughput, memory, concurrency or volume effect.

### T4 Explicit behavior — YES

Outcome contract (ticket AC + scope note):

| Resolved host shell | `_kiro/terminal/shell_type` reply | KAS dialect |
|---|---|---|
| POSIX via `$SHELL` basename `sh` / `bash` / `dash` / `zsh` / `ksh` | that basename (`{"shellType":"zsh"}` …), never `posix` | posix |
| POSIX via the `bash`-on-PATH fallback or config `[agent] shell = "bash"` | `bash` | posix |
| fish | `fish` (unchanged) | fish |
| Windows pwsh / Windows PowerShell | `powershell` (unchanged) | powershell |
| no resolved shell | existing refusal (unchanged) | — |

- The reply names the interpreter `terminal/create` actually runs (same `HostShell` snapshot), so KAS 0.66.43's approval-time `shellType` and run-time dialect agree (no `InterpreterChangedError`).
- cmd: cyril never resolves cmd (6bol's "never COMSPEC" decision); nothing to change.
- Supersedes one row of the cyril-6bol spec (`.cyril-6bol/spec.md` "Wire vocabulary: exactly 3 response tokens: posix, fish, powershell"): that row encoded the bug — `posix` is KAS's OUTPUT dialect, not an accepted input. The requester's own ticket (cyril-k8wc AC) replaces it. Everything else 6bol fixed is kept: real host detection instead of a hardcoded `"bash"`, Windows reports `powershell`, responder/executor agreement through one snapshot.

Out of scope (tracker taxonomy): widening `classify_unix` to more KAS-known shells (ash, mksh, yash, busybox, tcsh/csh) — a permanent non-goal for this fix; csh-family shells would need different quoting, not just a name.

## Selected route: **Local**

T1=no, T2=no, T3=no, T4=yes. Normal repository TDD: red-first test pinning the concrete name per POSIX shell, fix, full ci.yml gate, Windows VM run, live before/after acceptance.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| `route.md` | change-workflow | this file |
| `receipts/` | evidence for T1, red-first, gate, Windows, live | see Results |
| spec / design / plan | — | N/A — Local route |

## Terminal criterion

Requested reply behavior delivered; red-first test fails on base and passes after; every ci.yml lane passes; Windows VM run passes the host_shell/terminal_io tests; live KAS session with `SHELL=/bin/bash` shows (before) no "Always" option / re-ask with an allow rule, and (after) persistable consent and auto-run under a matching V3 allow rule; independent review satisfied (PR review); no blocking defect open.

## Results

Delivered at `7ae63f66` (fix commit on base `85becf9f`). `ShellKind::Posix` carries the `PosixShell` (`sh`/`bash`/`dash`/`zsh`/`ksh`) it was classified from — one table drives both `$SHELL` classification and the wire name — and `HostShell::wire_name()` reports it; fish → `fish`, pwsh/Windows PowerShell → `powershell` unchanged.

| Check | Result | Receipt |
|---|---|---|
| KAS accepted-vocabulary premise (0.66.34 + 0.66.43) | PASS — mapper byte-identical; `posix` → `unknown` | [`receipts/kas-shell-type-mapper.txt`](receipts/kas-shell-type-mapper.txt) |
| Red-first | PASS — on base, 10 POSIX cases + the registry pin FAIL (`left: "posix"`), 4 fish/powershell cases pass | [`receipts/red-first.txt`](receipts/red-first.txt) |
| Green after fix | PASS — 14/14 | [`receipts/green-after-fix.txt`](receipts/green-after-fix.txt) |
| Gate: all 10 ci.yml lanes at `7ae63f66` | PASS — every lane `-OK` | [`receipts/gate.txt`](receipts/gate.txt) |
| Windows VM (rustc 1.94.0 msvc), host_shell + terminal_io incl. `windows_powershell_lifecycle_smoke` | PASS — 33/33 | [`receipts/windows-tests.txt`](receipts/windows-tests.txt) |
| Windows clippy (not a CI lane) | no new warning: base 18 pre-existing errors, branch 17 (subset) | same file |
| Live before (`85becf9f`, SHELL=/bin/bash, KAS 0.66.43) | bug reproduced: overlay offers only Allow/Deny; V3 rule `echo AUDITX*` still asks (`effect:"ask"`); kiro.log `policy.shell.type.unknown {"shellName":"posix"}` | [`receipts/live/before-bash-r2/`](receipts/live/before-bash-r2/) |
| Live after (`7ae63f66`, SHELL=/bin/bash) | overlay offers Allow / Always allow / Deny / Always deny; rule turn auto-runs with 0 prompts (`effect:"allow","capability":"shell"`); no `policy.shell.type.unknown` | [`receipts/live/after-bash/`](receipts/live/after-bash/) |
| Live after (`7ae63f66`, SHELL=/usr/bin/zsh) | same as bash | [`receipts/live/after-zsh/`](receipts/live/after-zsh/) |
| Independent review | PENDING — PR reviewer | — |

No `InterpreterChangedError` / `policy.shell.interpreter_mismatch` in any arm's kiro.log. Live binaries and driver: [`receipts/live/binaries.txt`](receipts/live/binaries.txt), `receipts/live/k8wc-*.sh`.
