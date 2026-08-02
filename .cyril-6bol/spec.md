# Feature: Match KAS shell reporting and execution

## What this is

Cyril will resolve one runnable host shell at startup when the KAS engine owns terminal host callbacks. It will report that shell’s KAS-normalized family and execute every `terminal/create` command through the same shell, so the model’s shell syntax and the host executor agree.

## Users

- **Local Cyril operator**: a developer running Cyril with the KAS engine on their own Unix or native-Windows host. They need model-generated commands to use the shell selected by their host or Cyril configuration, and they see a startup error when Cyril cannot select that shell safely.

## Behavior

### Resolve the Unix host shell automatically
- **Given**: KAS is selected, `[agent] shell` is absent or `"auto"`, and Cyril runs on Unix.
- **When**: Cyril starts the bridge.
- **Then**: Cyril uses the runnable executable named by `$SHELL` when its basename is `bash`, `sh`, `dash`, `zsh`, `ksh`, or `fish`; reports `fish` only for fish and `posix` for every listed POSIX shell; falls back to a runnable `bash` from `PATH` when `$SHELL` is absent, unsupported, or stale; and exits with a diagnostic when no runnable fallback exists.

### Resolve the Windows host shell automatically
- **Given**: KAS is selected, `[agent] shell` is absent or `"auto"`, and Cyril runs natively on Windows.
- **When**: Cyril starts the bridge.
- **Then**: Cyril selects runnable PowerShell 7 first (`pwsh.exe` from `PATH`, then the standard `%ProgramFiles%\PowerShell\7\pwsh.exe` location), otherwise runnable Windows PowerShell when `PSModulePath` indicates its installation; reports `powershell` for either executable; and exits with a diagnostic if neither is runnable. `COMSPEC` does not affect the decision. `cmd` is never selected.

### Honor an explicit shell configuration
- **Given**: `[agent] shell` is one of `"auto"`, `"bash"`, `"fish"`, `"pwsh"`, or `"powershell"`.
- **When**: Cyril starts the bridge.
- **Then**: `"auto"` follows host detection; Unix accepts runnable `bash` or `fish`; native Windows accepts runnable `pwsh` or `powershell`; a cross-platform value or missing executable causes startup to fail with a diagnostic naming the configured value and failure. There is no `cmd` value and no arbitrary executable-path value.

### Report the resolved KAS shell family
- **Given**: startup resolved one shell executable and normalized family.
- **When**: KAS sends `_kiro/terminal/shell_type`.
- **Then**: Cyril returns exactly one of `{"shellType":"posix"}`, `{"shellType":"fish"}`, or `{"shellType":"powershell"}`, matching the executable retained at startup.

### Execute through the reported shell
- **Given**: startup resolved a shell and KAS sends `terminal/create` with `{command, args, cwd}`.
- **When**: Cyril creates the terminal.
- **Then**: Cyril invokes the retained shell in non-interactive login/profile command mode; executes in `cwd`; preserves non-operator argument values including spaces; exposes that shell’s native pipe, redirection, conditional, and variable-expansion semantics; returns the existing terminal identifier immediately; and preserves the existing output, wait, kill, release, cancellation, and exit-status lifecycle.

### Load shell profiles once
- **Given**: the selected shell has its normal login/profile startup files.
- **When**: a `terminal/create` command starts.
- **Then**: the shell loads those files once before the command. Profile-defined environment is visible to the command and profile output is captured as terminal output. A missing profile is not an error. If profile startup code fails, Cyril returns the shell’s native output and exit status without retrying the command.

### Resolve operator-token ambiguity
- **Given**: KAS tokenization produces an argument equal to a shell operator such as `|`, `>`, `>>`, `<`, `&&`, or `||`, and the wire carries no metadata distinguishing quoted literal text from syntax.
- **When**: Cyril constructs the shell command.
- **Then**: shell syntax wins: the token is interpreted by the selected shell. A literal operator argument is unsupported unless probe evidence finds wire metadata that preserves the distinction.

### Keep shell grammar native
- **Given**: a command uses syntax not supported by the selected shell version.
- **When**: the shell executes it.
- **Then**: the shell returns its native error and exit status. Cyril does not translate Bash, fish, PowerShell, or cmd grammar.

## Success criteria

- **Selection correctness**: 100% of the Unix and Windows cases enumerated above select the named executable/family or the named startup failure, measured by a parameterized resolver test with one case per platform/environment/configuration branch.
- **Response/executor agreement**: 100% of supported explicit and automatic selections use one startup-resolved value for both `_kiro/terminal/shell_type` and `terminal/create`, measured by state-transition tests that poison environment/PATH values after resolution.
- **Command semantics**: 5/5 behavioral fixtures per runnable supported shell family pass—spaced argument, pipe, redirection, environment-variable expansion, and profile-defined environment—measured by terminal lifecycle integration tests on the host shells available to each CI platform.
- **Failure fidelity**: 4/4 failure fixtures pass—empty command, missing configured executable, profile startup error, and native shell non-zero exit—measured by exact error/exit assertions with no command retry.
- **COMSPEC independence**: 0 shell-selection branches read `COMSPEC`, measured by a poisoned-`COMSPEC` resolver fixture and source search of production shell-selection code.
- **Regression fence**: 100% of existing workspace tests and lints pass, measured by the repository’s full per-slice gate.

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| Empty `command` | Reject before spawning and return the existing ACP invalid-request/error path. | An empty program cannot produce defined shell behavior. |
| `$SHELL` absent or empty on Unix | Resolve runnable `bash` from `PATH`; fail startup if absent. | Bash is the specified Unix default. |
| `$SHELL` names an unsupported shell | Ignore it and resolve runnable `bash` from `PATH`. | KAS has no matching normalized family. |
| `$SHELL` names a supported but missing executable | Resolve runnable `bash` from `PATH`; fail startup if absent. | A stale environment value must not create a delayed terminal failure. |
| Windows has PowerShell 7 in both `PATH` and the standard location | Use the `PATH` result. | The documented priority is deterministic. |
| Windows has only Windows PowerShell | Use it only when `PSModulePath` indicates availability and the executable is runnable. | This follows the requested availability signal without treating a signal as proof of executability. |
| Windows has only `COMSPEC`/cmd.exe | Fail startup. | KAS maps `cmd` to its PowerShell model tool, so cmd execution would misstate grammar. |
| Explicit shell is invalid for the platform | Fail startup and name the configured value. | Explicit configuration must not be silently discarded. |
| Explicit shell executable is unavailable | Fail startup and name the missing executable. | Falling back would violate the operator’s selection. |
| Config changes while Cyril runs | Keep the startup snapshot; changes take effect after restart. | Reporting and execution cannot drift during a session. |
| PATH or environment changes while Cyril runs | Keep the startup snapshot. | The responder and executor must use one value. |
| Profile file is absent | Run the command normally. | Shell profiles are optional. |
| Profile startup prints output | Capture it in the terminal output before command output. | The profile is part of the selected shell’s native startup. |
| Profile startup fails | Preserve the shell’s output and exit status; do not retry. | A retry could execute a side-effecting command twice. |
| Argument contains spaces or newlines | Quote it as one literal argument unless it is a recognized operator token. | Token boundaries must survive reconstruction. |
| Quoted literal equals an operator token | Interpret it as syntax when the wire cannot distinguish it. | The requester chose shell behavior over literal-operator fidelity. |
| Shell version lacks an operator | Return the shell’s native error and exit status. | Cross-shell translation is excluded. |
| Native shell exits non-zero | Preserve the exit code through the existing terminal wait response. | Shell failure is data, not a bridge failure. |
| Shell process cannot start | Return a terminal-create error and create no registry entry. | Partial registration would create an unusable terminal id. |
| Multiple terminals start concurrently | Use the same immutable resolved shell; each create receives its existing unique terminal id. | Resolution is host configuration, not per-terminal state. |
| Duplicate `terminal/create` request | Spawn a distinct process and terminal id, matching existing non-idempotent behavior. | ACP terminal creation has no idempotency key. |
| Permission denied by existing KAS policy | Preserve the current permission path; this change does not bypass it. | Shell selection does not redefine authorization. |
| Cancellation during execution | Preserve the shipped session reap/kill behavior from `cyril-3lh8`. | Process lifecycle is already fenced. |
| Maximum concurrent terminal count | Preserve current registry/channel behavior; add no new limit. | The change adds one immutable shell selection, not per-terminal discovery. |
| Soft-deleted data | Not applicable; no persistent records are read. | Terminal execution has no soft-delete model. |
| Multi-tenancy boundary | One Cyril process uses one host/config shell snapshot for its sessions. | Cyril is a local process, not a multi-tenant service. |
| Time zone or DST change | No effect. | Selection and command construction do not use wall-clock time. |
| Replication lag | Not applicable. | No replicated store participates. |
| Cache invalidation | Restart is the invalidation boundary for the startup shell snapshot. | Mid-session re-resolution would permit reporting/execution drift. |

## Out of scope

This change does NOT include:

- cmd.exe support or PowerShell-to-cmd translation.
- Arbitrary shell executable paths or shells outside the fixed configuration values.
- Interactive shell mode or interactive-only rc files.
- Cross-shell grammar translation.
- Changes to KAS permission policy or command approval.
- Output-byte-limit behavior tracked by `cyril-1rpv`.
- Terminal cancellation/reaping behavior already shipped by `cyril-3lh8`.
- Changes to v2 engine spawning or its WSL transport.
- Runtime config reload.

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| Wire vocabulary | Exactly 3 response tokens: `posix`, `fish`, `powershell` | Serialized-response tests |
| Configuration vocabulary | Exactly 5 values: `auto`, `bash`, `fish`, `pwsh`, `powershell` | TOML parse matrix |
| Model/executor mismatch | 0 supported configurations | Resolver-to-response-to-launch matrix |
| Shell discovery frequency | Exactly 1 successful resolution per Cyril startup | Resolver injection/counting test |
| Profile loading | Exactly 1 login/profile startup per terminal command, 0 retries | Launch-spec and profile-marker assertions |
| v2 behavior changes | 0 | Existing v2 tests and config tests |
| `COMSPEC` discrimination | 0 reads in production selection code | Poisoned environment fixture plus source search |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | No configured shell and no PowerShell exists on native Windows: fail or use cmd? | Fail terminal use; later sharpened to fail startup. | cmd must not be selected silently. |
| 2 | Interpret shell operators or keep direct argv? | Execute through the selected shell. | Reporting shell syntax while bypassing that shell is inconsistent. |
| 3 | Where is the explicit override? | Cyril TOML config only. | Reuses the existing `[agent]` configuration surface without another CLI precedence rule. |
| 4 | Unix `$SHELL` names zsh/sh/dash: which executable runs? | Run the exact executable and report `posix`. | Host choice and execution stay aligned. |
| 5 | Unix `$SHELL` is unsupported? | Fall back to bash. | KAS lacks a matching normalized family. |
| 6 | Allow shell families on either platform? | Restrict values by host platform. | Prevents a configured model/executor mismatch. |
| 7 | Explicit shell is invalid or missing? | Fail startup. | Explicit intent must not be discarded. |
| 8 | Auto-detection fails on Windows? | Fail startup. | Starting with an advertised terminal that cannot choose a matching shell is invalid. |
| 9 | Which config values are accepted? | Fixed values; originally six, then revised by decision 13 to remove cmd. | An enum is testable and excludes arbitrary grammar. |
| 10 | Load shell profiles? | Initially no; superseded by decision 15. | The requester revised this behavior before sign-off. |
| 11 | Operator token is indistinguishable from a quoted literal? | Shell syntax wins. | Pipe/redirection behavior takes precedence. |
| 12 | Translate operators across shells? | No; use native semantics. | Translation would create a second shell implementation. |
| 13 | Support explicit cmd despite KAS mapping it to `execute_pwsh`? | Reject cmd. | Supporting it would recreate the model/executor mismatch this issue removes. |
| 14 | Primary human role? | Local Cyril operator. | The feature controls shell execution on the operator’s own host. |
| 15 | Which profile mode? | Non-interactive login/profile startup. | Load profile-defined environment without forcing a TTY. |
| 16 | Retry without profiles when profile startup fails? | No retry. | Retrying could execute side effects twice. |

## Sign-off

The requester typed, verbatim:

> This defines shell behavior for Cyril. On Windows, powershell should be used. The shell used for Windows or Linux is configurable, but there is also an auto option. Shell configuration will try to be loaded, but if it fails the command will not be retried. The shell will interpret control symbols

The requester clarified “shell configuration,” verbatim:

> Yes, login profile

Date: 2026-08-01
