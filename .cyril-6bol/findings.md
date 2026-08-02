# Prove-it prototype findings

## Smallest question

Can Cyril take the genuine KAS `terminal/create` capture at `.cyril-7bdu/fixtures/terminal__create.json`, resolve the current Unix host shell, execute the split `{command,args}` through that shell in non-interactive login mode, and preserve the current direct-argv result?

Expanded once: can an operator token appended to the captured request remain shell syntax while every non-operator token is quoted as a literal argument?

## Probe

`.cyril-6bol/probe.py` is 50 lines. It reads the committed live KAS capture, resolves the actual host environment using the proposed Unix rule, constructs the shell command without Cyril production abstractions, and runs both the captured command and an expanded pipeline through `bash -lc`.

Observed output:

```json
{"command":"echo done-42","exit":0,"family":"posix","operator_command":"echo done-42 | tr a-z A-Z","operator_exit":0,"operator_stderr":"","operator_stdout":"DONE-42\n","shell":"/usr/bin/bash","shell_env":null,"source":".cyril-7bdu/fixtures/terminal__create.json","stderr":"","stdout":"done-42\n"}
```

## Oracle

`.cyril-6bol/oracle.sh` is independent of the probe’s JSON parser, resolver, renderer, and login-shell invocation. It hand-transcribes the one captured request, resolves Bash with the host command lookup, direct-executes `/usr/bin/echo done-42`, and computes the expanded result with the native `/usr/bin/echo | /usr/bin/tr` pipeline. Both outputs are canonicalized with `jq -S` and compared with `diff`.

Result:

```text
AGREEMENT
```

Probe and oracle agree item-by-item on the resolved shell/family, reconstructed command, stdout, stderr, and exit status for both the genuine capture and the operator expansion.

## What I learned

The actual ship-worktree environment has `$SHELL` unset even though `/usr/bin/bash` is runnable, so Unix fallback is a production path rather than a theoretical edge; additionally, ordinary `shlex.join` would quote `|` as a literal, so supporting shell control syntax requires a deliberate operator-token exception rather than generic argv quoting.
