# agent-shims

PATH shims that exploit Kiro's shell side channels (`$AGENT_DISPLAY_OUT` /
`$AGENT_CONTEXT_OUT`, shipped in kiro-cli 2.3.0) to make agent-driven commands
context-economical: the full output goes to the **user's display**, the model
gets a **distilled summary**, and a pointer to the full log rides the
`agent_notes` field of the tool result.

## How interception works

The agent types a plain `cargo test ...`. Kiro spawns the command through a
shell with this directory prepended to `PATH`, so resolution lands on the shim
before the real binary. The agent neither chooses nor sees the shim. Kiro
exports the two FIFO env vars automatically for every `execute_bash` —
per-invocation pipes named `agent-{display,context}-out-<pid>-<nonce>-<toolCallId>.fifo`.

```
agent: "cargo test"  →  shell PATH lookup  →  agent-shims/cargo
                                                ├─ full output → $AGENT_DISPLAY_OUT  (streams to tool_call_update content → cyril renders live)
                                                ├─ summary     → stdout              (the only inline text in the model's tool result)
                                                └─ log pointer → $AGENT_CONTEXT_OUT  (arrives as rawOutput.Json.agent_notes)
```

## Deployment

Prepend this directory to the `PATH` of the process that spawns `kiro-cli`:

```sh
PATH="$PWD/experiments/agent-shims:$PATH" kiro-cli acp   # or however the bridge launches
```

Cyril could do this in the bridge spawn (a future proxy-stage/skill concern).

## Shims

- `cargo` — wraps `test` / `clippy` / `build` / `check`. All other subcommands
  and human-driven invocations (env vars unset) `exec` the real cargo
  untouched, so the shim is safe to have on PATH permanently.
- `git` — wraps `status` / `commit` / `push` / `switch` / `checkout`. Different
  goal from `cargo`: not token economy but **labeled side-band facts with
  byte-exact stdout**. Real git output passes through untouched; repo-state facts
  (branch, main-vs-linked worktree, `core.hooksPath`, dirty counts, upstream
  ahead/behind, branches held by sibling worktrees) are delivered to the model as
  `agent_notes` instead of being spliced into command output. Aimed at the
  branch-hitchhiking hazard in CLAUDE.md ("Parallel sessions — one worktree per
  session"), which has bitten this repo three times: the model gets the state it
  needs to notice it is about to commit on the wrong branch.

### Channel semantics — measured, and NOT what you'd assume

Verified on the wire at kiro-cli 2.14.2
(`experiments/conductor-spike/probe-git-shim-2.14.2.py`):

| Channel | Model | Human |
|---|---|---|
| `stdout` | yes — tool result `stdout` | yes — streamed content |
| `$AGENT_DISPLAY_OUT` | **no** | yes — streamed content |
| `$AGENT_CONTEXT_OUT` | yes — `rawOutput.Json.agent_notes` | **yes** — also streamed as content |

**Only the display channel is one-audience.** `$AGENT_CONTEXT_OUT` is *not*
hidden from the user: Kiro streams both FIFOs into `tool_call_update.content`
interleaved with stdout in arrival order, so a cyril user sees the notes live.
Its real value is that the model receives the text in a separate labeled field
and that stdout stays byte-exact — not invisibility.

### Why the facts are declarative

The `git` shim emits state (`worktree-kind: main`), never instructions ("do not
commit here"). The 2.5.1 side-channel probe found the model classifies injected
*instructions* on `$AGENT_CONTEXT_OUT` as a prompt-injection attempt and
deliberately refuses them — so facts-to-reason-from is the only reliable payload.

Main-vs-linked worktree is detected portably by comparing
`git rev-parse --absolute-git-dir` with `--git-common-dir` (equal ⇒ main), not by
hardcoding a checkout path.

## Caveats

- **Kiro-only, below ACP.** Other ACP agents won't set these vars; the `[ -n ]`
  guard makes the shim degrade to a transparent passthrough.
- **Not a security boundary.** An absolute path (`~/.cargo/bin/cargo`) bypasses
  the shim.
- **FIFO writes block without a reader.** Only possible if Kiro exported the
  vars but its reader died mid-command.
- **Unix only.** The backend feature lives in `execute_cmd/unix.rs`; the
  Windows implementation is unverified.

## Verification

- Standalone (hand-made FIFOs): see this README's history / probe logs.
- End-to-end through a live `kiro-cli-chat 2.5.1 acp` session:
  `experiments/conductor-spike/probe-cargo-shim-2.5.1.py` — asserts
  interception, context economy, display streaming, and `agent_notes`
  delivery on the actual ACP wire.

Wire-level background: `docs/`/memory notes on the side-channel FIFOs, and the
original behavior probe `experiments/conductor-spike/probe-side-channels-2.5.1.py`.
