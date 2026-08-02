# Falsifiable design: KAS host shell

## Purpose

Make KAS’s reported shell family and Cyril’s terminal executor one immutable startup decision. A local Cyril operator can select a supported shell in `[agent] shell`, or use host detection; invalid KAS shell configuration stops startup before the bridge thread or agent process exists.

The prove-it artifacts in this directory establish the baseline: a genuine KAS `{command:"echo",args:["done-42"]}` request produces the same result through `/usr/bin/bash -lc` as direct argv, and an exact `|` token can remain syntax while other tokens are quoted. They also show that `$SHELL` is unset in the actual ship worktree, so the Unix fallback branch is production-reachable.

## Architecture

### Deep module and seam

Add `protocol/kas/host_shell.rs` as the deep module for host-shell selection and command construction. Its crate-private interface is:

```rust
HostShell::resolve(configured: Option<&str>) -> Result<HostShell, HostShellError>
HostShell::wire_name(&self) -> &'static str
HostShell::command(&self, command: &str, args: &[String]) -> Result<tokio::process::Command, acp::Error>
```

Callers do not learn platform detection order, executable probing, quoting rules, profile flags, or PowerShell exit-code preservation. Tests exercise the same interface through a private `HostEnvironment` seam with two adapters: `SystemHostEnvironment` for production and a table-backed fake for all Unix/Windows matrix cells. The seam stays internal because environment access is local-substitutable.

`HostShell` owns a runnable executable path and a closed shell kind: POSIX, fish, PowerShell 7, or Windows PowerShell. Bash/sh/dash/zsh/ksh share POSIX quoting and KAS token `posix`; both PowerShell executables share PowerShell quoting and KAS token `powershell`.

### Startup wiring

`AgentConfig` gains `shell: Option<String>`. Keeping the external TOML value as a string is deliberate: unknown strings must survive deserialization so the KAS startup resolver can reject them with the exact configured value. A serde enum would make the existing whole-file fallback silently erase an unsupported value before semantic validation.

`SpawnConfig` carries that optional string. A private resolved-startup bundle validates it synchronously in `spawn_bridge` only for `AgentEngine::Kas`, before channels, the bridge thread, or the agent process are created. Resolution errors map to `ErrorKind::InvalidConfig` with `HostShellError` as the source. V2 carries no `HostShell` and performs no shell/environment probes.

The resolved shell is moved to the bridge thread and into `KasEngine`. A KAS engine is constructible only with a `HostShell`; V2 has none. The existing `Engine` seam exposes the optional KAS shell to `KiroClient`, which constructs one `TerminalRegistry` around it. This keeps KAS capability advertisement, shell response, and executor availability consistent by construction.

### Response and execution

`TerminalRegistry` stores the immutable `HostShell`. Its shell-type responder serializes `HostShell::wire_name()`, and `create` calls `HostShell::command()` before applying the existing cwd translation, request environment, null stdin, piped stdout/stderr, and `kill_on_drop` lifecycle.

Command rendering allocates one command string. Exact operator tokens (`|`, `>`, `>>`, `<`, `&&`, `||`, `;`, `&`, `2>`, `2>>`, `2>&1`) remain unquoted. Pure environment-variable tokens also remain syntax: POSIX accepts `$NAME` and `${NAME}`, fish accepts `$NAME`, and PowerShell accepts `$env:NAME` and `${env:NAME}`, with `NAME = [A-Za-z_][A-Za-z0-9_]*`. All other tokens use the selected family’s literal quoting, including embedded forms such as `prefix-$HOME`, globbing, and command substitution. POSIX/fish use their single-quote form; PowerShell doubles embedded single quotes. This implements the signed decisions that shell syntax includes variable expansion and that syntax wins when KAS has erased whether an operator-looking token was quoted, without treating every metacharacter-containing argument as executable source.

POSIX and fish launch as non-interactive login shells with `-l -c`. PowerShell launches with `-NoLogo -NonInteractive -Command` and intentionally omits `-NoProfile`. The PowerShell command wrapper clears stale `$LASTEXITCODE`, executes the rendered command, captures `$?` and `$LASTEXITCODE` immediately, and exits with the external code when present or `0/1` for PowerShell-native success/failure. This preserves the terminal exit contract that plain `-Command` would otherwise collapse to `0/1`.

An empty `command` is rejected as ACP invalid params before shell construction. A non-empty but nonexistent inner command is no longer a `terminal/create` spawn failure: the shell starts, `create` returns an id, and wait/output carry the shell’s native not-found diagnostic and exit status. Failure to start the selected shell remains a create error, though startup executable validation makes that path primarily a race with filesystem changes.

## Input shapes

| Input | Production-reachable shapes | Claim coverage |
|---|---|---|
| Engine | V2; KAS | C1, C10 |
| `[agent] shell` | absent; `auto`; each valid explicit value; unknown/`cmd`; wrong platform; wrong TOML type | C1, C2, C3, C10 |
| Unix `$SHELL` | absent/empty; supported absolute runnable path; supported relative/path lookup; supported stale path; unsupported basename; Unicode/spaces; bash fallback present/absent | C2 |
| Windows discovery | pwsh on PATH; pwsh only in Program Files; Windows PowerShell signaled+runnable; signal without executable; neither; poisoned `COMSPEC` with cmd present | C3 |
| Shell kind | POSIX; fish; PowerShell 7; Windows PowerShell | C4–C7, C9 |
| Shell-type request | each resolved kind; V2/no terminal callback | C4, C5, C10 |
| `command` | empty; ASCII; Unicode; spaces; executable missing after startup | C6, C8 |
| `args` | empty; one; multiple; duplicates; spaces/newlines/Unicode; embedded quote; exact operator; operator-looking literal; pure family-valid environment variable; embedded variable text; glob; command substitution | C6 |
| `cwd` | `None`; absolute existing; absolute missing; relative | C8 |
| request environment | empty; one value; multiple distinct values; profile overrides one value | C7, C8 |
| lifecycle | immediate exit; non-zero exit; long-running; concurrent terminals; cancel/kill/release during wait | C8, C9 |

Wrong-typed TOML retains the repository’s established whole-file fallback because parsing never produces an `AgentConfig`; semantic string errors reach the new startup failure path. Arbitrary executable paths and shell families outside the fixed vocabulary are deliberately rejected.

## Removed-invariant sweep

The core move is subtractive: running split argv through a shell removes the direct-argv constraint.

| Direct-argv fact that existed before | Result after the change | Claim |
|---|---|---|
| Shell metacharacters and expansions were literal arguments. | Exact recognized control tokens and pure family-valid environment-variable tokens become native shell syntax; embedded expansions, globs, command substitution, and other tokens remain literal. | C6 |
| Login/profile files never ran. | One non-interactive login/profile startup runs per terminal command; its output and failure are observable. | C7 |
| `terminal/create` directly spawned `req.command`. | Empty command remains a pre-spawn invalid-params error; otherwise create spawns the startup-resolved shell and a nonexistent inner executable becomes a completed terminal with native failure. | C8 |
| The tracked child handle named the requested executable. | It names the shell, but the registry’s documented lifecycle only promises control of its tracked child; that lifecycle already supports `sh -c` requests and remains unchanged. | C8 |
| Argument boundaries could not be reparsed. | Literal quoting preserves every token except the closed operator and pure-variable forms; the signed operator-looking-literal ambiguity intentionally favors syntax. | C6 |
| A spawn error named `req.command`. | Empty command is invalid params; a shell-start race names the resolved executable; a non-empty inner-command error is terminal output/status. | C8 |

The existing ordering and concurrency constraints stay in force: create still returns before wait, `RefCell` borrows never cross await, output draining remains in wait, terminal ids remain unique, and cancel/release/kill retain their current ordering.

## Claims

- **C1 — Startup gate:** KAS resolves exactly one supported runnable shell before bridge-thread creation; resolution failure returns `InvalidConfig`; V2 resolves none.
- **C2 — Unix selection:** automatic Unix resolution follows supported runnable `$SHELL` exactly, otherwise runnable PATH Bash, while explicit Bash/fish is platform-checked and never silently substituted.
- **C3 — Windows selection:** automatic native-Windows resolution is PATH pwsh → Program Files pwsh → signaled+runnable Windows PowerShell → error; `COMSPEC`, cmd, Bash, and fish never select a Windows shell.
- **C4 — Single snapshot:** one immutable `HostShell` supplies both the shell-type response and every terminal launch for the bridge lifetime, even if environment/PATH/config changes afterward.
- **C5 — Wire vocabulary:** POSIX emits `posix`, fish emits `fish`, and both PowerShell variants emit `powershell`; no supported state emits `bash` or `cmd`.
- **C6 — Command meaning:** literal values survive as one argument, the closed operator set uses native grammar, and only pure family-valid environment-variable tokens expand; embedded variable text, globs, and command substitution remain literal.
- **C7 — Profile posture:** each command gets one non-interactive login/profile startup; missing profiles succeed, profile output is captured, and profile failure causes no retry.
- **C8 — Lifecycle preservation:** shell execution rejects empty command before spawn and retains immediate create, cwd/env/stdin/output, unique-id, wait, cancel, kill, release, and tracked-child cleanup behavior, with a non-empty nonexistent inner command reported as terminal status rather than create failure.
- **C9 — Exit fidelity:** POSIX/fish and both PowerShell variants surface the selected shell’s final native exit result, including an external exit code greater than one.
- **C10 — V2 isolation:** adding any shell string to config changes zero V2 startup, capability, process, or terminal behavior.

## Falsification

| # | Claim | Falsifier | Independent oracle | Killed buggy implementation | Cost | Status | Regression fence |
|---|---|---|---|---|---|---|---|
| 1 | C6 | Run the genuine capture, an added `| tr` operator, and a pure `$CYRIL_SHELL_PROBE` argument through the 61-line probe; compare every output field. Any semantic diff falsifies C6. | Direct `/usr/bin/echo`, native `/usr/bin/echo | /usr/bin/tr`, and `/usr/bin/printenv` in `.cyril-6bol/oracle.sh` | Generic argv quoting turns `|` and `$NAME` into literals; blanket unquoting splits literal values. | <1s | passed — 2026-08-01: `AGREEMENT` | `terminal_io::pipeline_operator_is_interpreted_and_literals_stay_grouped` and `terminal_io::pure_environment_variable_expands` |
| 2 | C5 | Serialize one response per shell kind; any body outside the three exact JSON objects falsifies C5. | Hand-authored KAS covenant token table | Retaining hardcoded `bash` or returning executable name `pwsh`. | <1s | pending | `host_shell::wire_name_matrix` and `client::shell_type_ext_request_routes` |
| 3 | C2 | Run the complete fake Unix environment matrix; any selected path/token or error differs from the input table. | Table derived from signed spec, not resolver code | Treating every unknown `$SHELL` as POSIX or trusting a stale path. | <1s | pending | `host_shell::unix_resolution_matrix` |
| 4 | C3 | Run the complete fake Windows matrix including `COMSPEC=cmd.exe`; any cmd selection or priority inversion falsifies C3. | Explicit expected-path table plus production source search for `COMSPEC` | Reading COMSPEC or preferring Windows PowerShell over Program Files pwsh. | <1s | pending | `host_shell::windows_resolution_matrix` |
| 5 | C4 | Resolve once, mutate the fake environment and executable table, then request wire name and launch plan; either changing falsifies C4. | Original immutable expected tuple `(path, token)` | Re-running detection separately in responder and create. | <1s | pending | `host_shell::resolution_is_a_startup_snapshot` |
| 6 | C1 | Configure KAS with unavailable/unsupported shell and a fake agent command that writes a marker; any marker/thread callback or non-`InvalidConfig` result falsifies C1. | Marker-file absence and returned domain error | Resolving inside the bridge after thread/process spawn or silently falling back. | 2s | pending | `bridge::invalid_kas_shell_fails_before_agent_spawn` |
| 7 | C7 | Use controlled profile markers for installed POSIX/fish and Windows PowerShell shells; assert one marker, captured profile output, and zero retry after a failing profile. | Marker-file count plus command-side-effect count | Passing `--noprofile`, using non-login mode, or retrying after error. | 3s | pending | `terminal_io::profile_loads_once_and_failure_does_not_retry` (platform-gated) |
| 8 | C9 | Run commands exiting 42 through each installed family; any reported status other than 42 falsifies C9. | Direct shell invocation and OS exit status | Plain PowerShell `-Command` collapsing external 42 to 1. | 3s | pending | `host_shell::launch_preserves_external_exit_code` (platform-gated) |
| 9 | C8 | Replay existing lifecycle suite plus empty-command rejection and pipeline release/cancel with delayed marker; any changed error code, cwd/env/stdin/ordering/id/output, or post-cancel marker falsifies C8. | Exact ACP error assertion, filesystem markers, elapsed-time bound, and OS process liveness | Spawning an empty shell command, awaiting in create, inheriting stdin, dropping env/cwd, or bypassing registry kill. | 8s | pending | Existing `terminal_io` lifecycle tests plus `create_empty_command_is_invalid_params` and `release_stops_shell_pipeline` |
| 10 | C10 | Start the in-process V2 harness twice with absent vs invalid shell string and diff initialize/process/notification frames; any diff falsifies C10. | Raw captured frame diff | Validating KAS-only shell config on V2. | 10s | pending | `bridge::shell_config_is_inert_on_v2` |

## Cheapest falsifier command

```sh
diff <(.cyril-6bol/probe.py | jq -S .) <(.cyril-6bol/oracle.sh | jq -S .)
```

The command exited 0 and emitted `PASSED` on 2026-08-01 after the expansion fixture was added; row 1 records the observed pass.

## Negative space

- Cyril does not support cmd.exe because KAS presents its PowerShell model tool for `cmd`, recreating the grammar mismatch.
- Cyril does not accept arbitrary executable paths or shell families outside `auto|bash|fish|pwsh|powershell`.
- Cyril does not translate grammar between POSIX, fish, and PowerShell.
- Cyril does not reload shell configuration or environment during a running bridge.
- Cyril does not change permission policy, output byte limiting (`cyril-1rpv`, verified open P4), or shipped cancel/reap policy (`cyril-3lh8`, verified closed).
- Cyril does not interpret embedded variable text, globs, or command substitution; only the closed operator set and pure family-valid environment-variable tokens cross the syntax seam.

## Primary references

- GNU Bash startup files: <https://www.gnu.org/software/bash/manual/html_node/Bash-Startup-Files.html>
- fish invocation (`-l`, `-c`, config loading): <https://fishshell.com/docs/current/cmds/fish.html>
- PowerShell 7 `pwsh` invocation and exit semantics: <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_pwsh?view=powershell-7.5>
- Windows PowerShell 5.1 invocation and exit semantics: <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_powershell_exe?view=powershell-5.1>
