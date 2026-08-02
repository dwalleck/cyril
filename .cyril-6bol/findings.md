# Prove-it prototype findings

## Smallest question

Can Cyril take the genuine KAS `terminal/create` capture at `.cyril-7bdu/fixtures/terminal__create.json`, resolve the current Unix host shell, execute the split `{command,args}` through that shell in non-interactive login mode, and preserve the current direct-argv result?

Expanded twice: can an exact operator token remain shell syntax while every literal token is quoted, and can a pure `$NAME` argument expand without blanket-unquoting literal values?

## Probe

`.cyril-6bol/probe.py` is 61 lines. It reads the committed live KAS capture, resolves the actual host environment using the proposed Unix rule, constructs commands without Cyril production abstractions, and runs the captured command, an expanded pipeline, and a pure environment-variable token through `bash -lc`.

Observed output:

```json
{"command":"echo done-42","exit":0,"expansion_command":"echo $CYRIL_SHELL_PROBE","expansion_exit":0,"expansion_stderr":"","expansion_stdout":"expanded-42\n","family":"posix","operator_command":"echo done-42 | tr a-z A-Z","operator_exit":0,"operator_stderr":"","operator_stdout":"DONE-42\n","shell":"/usr/bin/bash","shell_env":null,"source":".cyril-7bdu/fixtures/terminal__create.json","stderr":"","stdout":"done-42\n"}
```

## Oracle

`.cyril-6bol/oracle.sh` is independent of the probe’s JSON parser, resolver, renderer, and login-shell invocation. It hand-transcribes the one captured request, resolves Bash with the host command lookup, direct-executes `/usr/bin/echo done-42`, computes the operator result with the native `/usr/bin/echo | /usr/bin/tr` pipeline, and computes environment lookup with `/usr/bin/printenv`. Both outputs are canonicalized with `jq -S` and compared with `diff`.

Result:

```text
AGREEMENT
```

Probe and oracle agree item-by-item on the resolved shell/family, reconstructed command, stdout, stderr, and exit status for the genuine capture, the operator expansion, and the pure environment-variable expansion.

## What I learned

The actual ship-worktree environment has `$SHELL` unset even though `/usr/bin/bash` is runnable, so Unix fallback is a production path rather than a theoretical edge. Ordinary `shlex.join` quotes both `|` and `$NAME` as literals, so the syntax seam must recognize a closed operator set and pure family-valid environment-variable forms while continuing to quote every other token.

## Implementation finding

The original terminal lifecycle fences used a tail-exec-friendly `sh -c` command, so killing the tracked PID also killed the command. A selected-shell operator pipeline forced the outer shell to retain child processes and proved that `terminal/release` left the pipeline child alive long enough to write a delayed marker. The final implementation starts Unix terminals in a fresh process group, owns that group with the child, and terminates the tree on kill, release, cancellation, and owner drop. The native-Windows explicit cleanup path uses `taskkill.exe /T /F` before the existing direct-child fallback. Filed as `cyril-2z9g`; fixed in this branch by the `release_kills_child_and_frees_id` and `cancel_reaps_sessions_running_terminals` fences.