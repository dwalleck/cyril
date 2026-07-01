# KAS-5b — terminal host-callback responders (cyril-ufie)

Falsifiable design. Extends the prove-it (`.cyril-ufie/PROVE-IT.md`, probes
`.cyril-ufie/probe.rs` + `probe2.rs`, reusing capture
`.cyril-7bdu/host_callbacks_2.10.0.json`). **Scope: advertise `terminal:true` and
implement the five typed `acp::Client` terminal methods + the `_kiro/terminal/shell_type`
ext responder.** Depends on KAS-5a (cyril-7bdu) for the host-io seam.

## Purpose

KAS delegates **shell execution** to the host via server→client ACP *requests* when
the client advertises the `terminal` capability — the same delegation model KAS-5a
built for `fs`. Today `KasEngine` advertises `fs` but not `terminal`
(engine.rs:96-103), so KAS runs shell commands in-process and cyril never sees them.
KAS-5b makes cyril the executor of shell commands too — the second `cyril-stages`
interception point (ADR-0003), and the one that **stresses the non-blocking
invariant hardest** (a command can run up to 60s, vs an fs op's milliseconds).

## Architecture (grounded in the code, not aspiration)

- **Mechanism (verified, probe.rs):** the five terminal ops arrive as **typed
  `acp::Client` trait methods** — `create_terminal(CreateTerminalRequest)`,
  `terminal_output(TerminalOutputRequest)`, `wait_for_terminal_exit(WaitForTerminalExitRequest)`,
  `release_terminal(ReleaseTerminalRequest)`, `kill_terminal(KillTerminalRequest)` —
  default body `Err(method_not_found())` (acp 0.10.2 `client.rs:86,100,119,128,143`).
  `KiroClient` (`protocol/client.rs:41`) adds five `#[cfg(feature="kas")]` overrides,
  exactly where the code already earmarks them (client.rs:217-220). Bare ACP, not
  `_kiro/*` (prove-it #1).
- **`shell_type` is an ext request, not a typed method:** `_kiro/terminal/shell_type`
  rides the `ext_method` path; `handle_ext_request` (client.rs:213) matches the
  stripped string `kiro/terminal/shell_type` (acp strips the leading `_` inbound, per
  the auth constant `kiro/auth/getAccessToken`, auth.rs:20) and replies `{shellType}`.
- **Capability gate:** `KasEngine::client_capabilities()` (engine.rs:96) gains
  `.terminal(true)` (builder confirmed, schema 0.11.2 `client.rs:1514`; field
  `client.rs:1478`). `V2Engine` stays empty — v2 never receives terminal requests, so
  v2 is byte-unchanged. The existing `!caps.terminal` assertion (engine.rs:163) flips
  to `caps.terminal` — a ready-made regression fence.
- **New state — a terminal registry (terminal is STATEFUL, unlike stateless fs):**
  `KiroClient` gains `terminals: RefCell<HashMap<TerminalId, TerminalEntry>>` and a
  `Cell<u64>` id counter, mirroring the existing `tool_call_inputs: RefCell<HashMap>`
  (client.rs:19) — `!Send`, single-thread, no lock. Ids are `term-{n}` from the
  monotonic counter (process-unique). A `terminal_io` module under `protocol/kas/`
  (sibling of `host_io.rs`) owns spawn/drain/reap.
- **Lifecycle model (avoids the pipe-buffer deadlock + the borrow-across-await hazard):**
  `create_terminal` spawns the command via **`tokio::process::Command`** with piped
  stdout+stderr, inserts a `Running{child}` entry, and returns the id **immediately**
  (no await on exit). `wait_for_terminal_exit` **moves** the child out of the entry
  (dropping the `RefCell` borrow *before* awaiting), calls
  `child.wait_with_output().await` — which drains both pipes *concurrently with* the
  wait, so a >64KB writer can't deadlock — then re-borrows to store
  `Exited{output, status}` and returns the **flat** status. `terminal_output`
  snapshots the entry (combined stdout+stderr; `Exited`→status, `Running`→none).
  `release_terminal` kills a `Running` child + removes the entry; `kill_terminal`
  kills + keeps the entry valid. **Invariant: never hold a `RefCell` borrow across an
  `.await`** (take the child out, await, re-borrow) — else a concurrent op panics
  `BorrowMutError`.
- **Non-blocking (ADR-0004):** the acp connection spawns each inbound request as its
  own `spawn_local` task (rpc.rs:272-274), but the runtime is **single-threaded**
  (`current_thread`+`LocalSet`, bridge.rs:138/144). So `wait_for_terminal_exit` must
  `.await` `tokio::process` (yields to the reactor), **never** a thread-pinning
  `std::process::Child::wait()`. Same rule KAS-5a documented for `tokio::fs` vs
  `std::fs` (host_io.rs:9-18) — terminal inherits and stresses it.
- **Path boundary:** `cwd` is absolute (prove-it). It crosses `platform::path`
  translation at the responder boundary (Linux no-op; Windows `C:\`↔`/mnt/c`), reusing
  KAS-5a's `to_native`/`has_root` logic (host_io.rs:75).

## Input shapes (step 2)

`CreateTerminalRequest { session_id, command: String, args: Vec<String>,
env: Vec<EnvVariable>, cwd: Option<PathBuf>, output_byte_limit: Option<u64> }`;
`{TerminalOutput,WaitForTerminalExit,Release,Kill}Request { session_id, terminal_id }`;
`_kiro/terminal/shell_type { session_id }`. Distinct production-reachable shapes:

- **command**: valid program (observed `echo`, `ls`); **nonexistent** program (spawn
  fails) → C6.
- **args**: empty (observed `[]` via shell), single, multi (observed `["-la"]`).
- **env**: empty (observed); populated (spec-reachable) → applied via `Command.envs`.
- **cwd**: `Some(absolute)` (observed, every call) → C7; `None` → process cwd;
  `Some(relative)` (spec-violating) → rejected; `Some(/mnt/c/...)` on Windows → translated.
- **exit outcome**: code 0 (observed `echo`); code **non-zero** (probe `exit 42`);
  **signal-killed** (probe2 SIGKILL, code None) → C8; relevant to `kill`.
- **output**: empty; stdout-only; **stdout+stderr** (combined) → C9; non-UTF-8 bytes
  (lossy decode to `String`).
- **terminal_id** (output/wait/release/kill): known-`Running`; known-`Exited`;
  **unknown** (never created / already released) → C5.
- **concurrency**: 1 terminal; **2+ concurrent** terminals (distinct ids, no
  cross-talk, no starvation) → C4, C12.
- **session_id**: always present; main vs subagent session (the orphan-on-cancel edge
  needs per-session keying — out of scope, cyril-3lh8).

**Args / env coverage**: `args` (empty/single/multi) pass through `Command.args` and are
exercised by C9 (`sh -c 'echo …'` = multi-arg) and C7 (`pwd` = no args). `env` empty is
the observed default (covered by every create claim).

**Out of scope** (one line each):
- `output_byte_limit = Some(N)` truncation — **not production-reachable @ 2.10.0** (capture
  shows KAS sends none); always `truncated:false`, full output. Tracked **cyril-1rpv**.
- `env = Some(populated)` — **not observed @ 2.10.0** (capture sends none); passed through
  via `Command.envs(req.env)` when present, no dedicated fence (trivial, no branch).
- PTY/TTY allocation — cyril pipes; interactive programs needing a tty get none (KAS's
  shell tool is non-interactive).
- non-UTF-8 in `command`/`args` themselves — `String`/`Vec<String>` per acp; not reachable.

## Removed-invariant sweep (step 2b)

**Core move is subtractive underneath.** "+terminal capability" removes the invariant
**"every server→client host callback completes promptly, so no single `spawn_local`
task stays live long on the one bridge thread"** (true for KAS-5a: `tokio::fs` ops are
milliseconds). Walk the chain it guaranteed for free:

1. host callbacks are short → 2. the single bridge thread is never occupied by one
callback for long → 3. the rpc read loop, the prompt task, a mid-turn **cancel**, and a
*second* terminal's `create` are all serviced promptly.

Terminal **removes link 1**: `wait_for_terminal_exit` can be live for 60s. If it
`.await`s `tokio::process` (yielding), links 2-3 still hold. If it instead (the
regression) uses a sync `std::process` wait, **or** holds a `RefCell` borrow across the
await, links 2-3 break: a concurrent terminal/cancel is starved or the runtime panics.
→ **Claim C12** (the property that must still hold) + the borrow-across-await invariant.

A second removed invariant: **"the bridge spawns no child processes."** Now it does →
children must be reaped (no zombies) and killed on release. → **C9/C10**. Reaping a
child whose turn was *cancelled without a release* is the residual leak → out of scope,
tracked **cyril-3lh8** (needs per-session id keying).

Invariants judged **still-safe** (one-line reason each): v2 path (advertises no
`terminal` cap → receives no terminal requests → unchanged); fs responders (untouched —
terminal is additive to them); the existing `request_permission` approval overlay (KAS
gates `terminal/create` with its own `request_permission`, already handled by the
unchanged approval path, prove-it #3 — cyril parses none of the `_meta.kiro.consent`).

## Claims

1. `KasEngine::client_capabilities()` advertises `terminal = true` (keeping `fs`); `V2Engine` stays empty.
2. **[cheapest — passed]** `wait_for_terminal_exit` reply serializes **flat** `{exitCode, signal}` (typed `WaitForTerminalExitResponse`), NOT nested `{exitStatus:{…}}`.
3. `create_terminal` returns a `terminalId` immediately, without awaiting the command's exit (a long command does not delay the create reply).
4. `terminalId`s are unique across concurrently-live terminals.
5. `terminal_output`/`wait_for_terminal_exit`/`release_terminal`/`kill_terminal` on an **unknown** `terminalId` return `Err` (`-32602`), never panic the bridge.
6. `create_terminal` with a **nonexistent command** returns `Err` (`-32603`), never panics the bridge.
7. `create_terminal` runs the command in `cwd` when `Some` (absolute, translated via `platform::path`); `None` → process cwd; a **non-absolute** `cwd` is rejected `-32602`.
8. A non-zero exit reports `exitCode = code, signal = null`; a **signal-killed** command reports `exitCode = null, signal = Some`.
9. `terminal_output` reply serializes `{output, truncated:false, exitStatus}` (nested `exit_status`), with `output` = the command's **combined stdout+stderr** (non-UTF-8 lossily decoded).
10. `release_terminal` kills a still-running child, reaps it, and frees the id (subsequent ops on it → unknown-id `Err`).
11. `kill_terminal` terminates a running child but keeps the id valid (a later `terminal_output`/`wait_for_terminal_exit` still resolves).
12. A slow `wait_for_terminal_exit` (awaiting a long command) does not starve the single-threaded runtime: a concurrent second terminal's `create`+`wait` still completes promptly (responders use `tokio::process` and hold no `RefCell` borrow across `.await`).
13. `_kiro/terminal/shell_type` returns `{shellType}` via the `ext_method` path; all terminal methods compile only under `#[cfg(feature="kas")]` (a default build still answers `method_not_found`).

## Falsification

| # | Claim | Falsifier (input → result that proves it false) | Oracle (independent of SUT) | Cost | Status | Regression fence |
|---|-------|--------------------------------------------------|-----------------------------|------|--------|------------------|
| 1 | KAS advertises `terminal`; V2 empty | Call each engine's `client_capabilities()`; if KAS `terminal=false` or V2 non-empty → false | direct struct assertion (`caps.terminal`) | 5m | **passed** (builder `.terminal(true)` exists, schema 0.11.2 `client.rs:1514`) | unit `engine::kas_advertises_terminal_v2_empty` (flip existing `!caps.terminal`) |
| 2 | wait reply FLAT, not nested | Serialize typed `WaitForTerminalExitResponse`; if JSON contains key `exitStatus` (wrapper) instead of top-level `exitCode`/`signal` → false | `serde_json` of the pinned acp type (probe.rs) + the KAS-5a accepted reply (probe-…-2.10.0.py:158) | 2m | **passed** (probe.rs: `{"exitCode":42,"signal":null}`) | unit `terminal_wait_reply_is_flat` (assert no `"exitStatus"`, has `"exitCode"`) |
| 2L | KAS *honors* the flat wait reply for a non-zero exit | Agent runs `true` (really exits 0); host injects an unpredictable `exitCode=42` into cyril's exact wire (flat wait + nested output); if the agent surfaces 0/success instead of 42 → false | live KAS 2.10.0 turn (the prove-it residual) | live | **passed (live 2026-06-30)** — agent reported `EXIT_CODE=42`; since `true` exits 0, 42 could only come from KAS parsing cyril's flat shape | **manual (live)** `experiments/conductor-spike/probe-kas5b-c2l-flat-wait-2.10.0.py` — re-run on a KAS binary bump |
| 3 | create returns before exit | `create_terminal{command:"sleep", args:["5"]}`; if the call returns only after ~5s (awaited exit) instead of <1s → false | wall-clock around the call (test harness clock, not the SUT) | 30m | pending | integration `create_returns_before_command_exits` |
| 4 | ids unique across concurrent terminals | Create 2 terminals before releasing either; if the two `terminalId`s are equal → false | string compare of the two returned ids | 20m | pending | unit `terminal_ids_unique` |
| 5 | unknown id → Err, not panic | `terminal_output`/`wait`/`release`/`kill` with an id never created; if it panics or returns `Ok` → false | the test asserts `Err`, bridge task survives | 20m | **passed (registry-miss)** (probe.rs: `terms.get("term-99").is_none()` — a miss, mapped to `Err`, no panic) | unit `unknown_terminal_id_errors_not_panics` (all four methods) |
| 6 | nonexistent command → Err, not panic | `create_terminal{command:"definitely-not-real-xyz"}`; if it panics or returns `Ok(id)` → false | tokio spawn returns `Err` (probe2: `is_err=true`) | 20m | **passed** (probe2.rs `C4 spawn(nonexistent) is_err = true`) | unit `create_nonexistent_command_errors` |
| 7 | cwd honored; non-absolute rejected | `create{command:"pwd", cwd:"/tmp"}` → output must contain `/tmp`; `cwd:"rel"` → must be `-32602`, not run in process cwd | `pwd` output (coreutils) + `has_root()` (pure fn, KAS-5a oracle) | 35m | pending | unit `create_cwd_honored_relative_rejected` |
| 8 | exit code vs signal | `exit 42` → `{exitCode:42,signal:null}`; SIGKILL → `{exitCode:null,signal:Some}` | probe.rs (`exit 42`→42) + probe2.rs (`code=None signal=Some(9)`) | 5m | **passed** (both probes) | unit `exit_status_nonzero_and_signal` |
| 9 | output = combined stdout+stderr (nested wire) | `sh -c 'echo OUT; echo ERR 1>&2'`; if `output` lacks `ERR` (stdout-only) or wire lacks nested `exitStatus` → false | probe2.rs (`combined_has_both=true`) + probe.rs output wire | 10m | **passed** (probes) | unit `output_reply_combined_stdout_stderr_nested` |
| 10 | release kills+reaps+frees id | Create `sleep 30`; `release`; then `terminal_output` on that id → must be unknown-id `Err`; the child must not survive | OS process check (`kill -0 pid` from the test) + the post-release `Err` | 45m | pending | integration `release_kills_and_frees_id` |
| 11 | kill terminates but keeps id | Create `sleep 30`; `kill`; then `wait_for_terminal_exit` on the SAME id → resolves with a signal status (not unknown-id) | the post-kill `wait` returns `Ok` with `signal=Some` | 45m | pending | integration `kill_terminates_keeps_id` |
| 12 | slow wait doesn't starve runtime | On a `current_thread` runtime: start `wait` on a `sleep 5` terminal; concurrently `create`+`wait` an `echo` terminal; if the echo terminal completes only after ~5s, or the runtime panics `BorrowMutError` → false | wall-clock of the *fast* terminal (must be <1s), on a single-thread harness | 1h | pending | integration `slow_wait_does_not_starve_runtime` |
| 13 | shell_type via ext; methods cfg-gated | Send `_kiro/terminal/shell_type`; if it returns `method_not_found` (unrouted) → false. Build without `kas`; if terminal overrides compile in → false | the ext reply `{shellType}` + a `cargo build` without `--features kas` | 30m | pending | unit `shell_type_ext_reply` + CI default-build (no-kas) |

Cheapest (C2) is **passed** — design may proceed to planning per the gate. C1/C5/C6/C8/C9 also have passing probe evidence.

### Non-vacuity (named buggy impls)
- C1: a `KasEngine` that forgets `.terminal(true)` (or copies V2's empty caps) → `caps.terminal=false`.
- C2: returning a hand-built `json!({"exitStatus":{...}})` (copying the KAS-5a probe) instead of the typed `WaitForTerminalExitResponse` → JSON has the `exitStatus` wrapper.
- C2L: KAS parsing per ACP spec (flat) → cyril's nested copy would read exit as undefined; cyril's flat reply is what this confirms.
- C3: `create` that does `child.wait_with_output().await` *before* returning the id → blocks ~5s.
- C4: an id scheme using a constant/`cwd`-hash instead of the monotonic counter → collisions.
- C5: `self.terminals.borrow().get(id).unwrap()` → panics on a miss (the `unwrap_used` lint should catch it, but the test fences the runtime behavior).
- C6: `Command::new(cmd).spawn().expect(...)` / `.unwrap()` → panics the bridge on a bad command instead of `Err`.
- C7: `Command::new(cmd)` with no `.current_dir()` → runs in the process cwd; or no `has_root()` guard → a relative cwd silently runs in process cwd.
- C8: mapping a signaled exit to `exit_code(0)` (ignoring `ExitStatusExt::signal()`) → reports `exitCode:0` for a killed process.
- C9: capturing `child.stdout` only (dropping stderr), or building `TerminalOutputResponse` with a flattened/None `exit_status` → `output` missing `ERR` / wire missing nested `exitStatus`.
- C10: `release` that removes the entry but never calls `start_kill` → orphaned `sleep 30`; or that leaves the id present → post-release op succeeds.
- C11: `kill` implemented as `release` (removes the entry) → post-kill `wait` is unknown-id instead of resolving.
- C12: `wait_for_terminal_exit` using `std::process::Child::wait()` (pins the thread) **or** `self.terminals.borrow_mut().get_mut(id)…await` (borrow held across await → `BorrowMutError`) → fast terminal blocked / runtime panics.
- C13: matching the un-stripped `_kiro/terminal/shell_type` (acp strips the `_`) → never matches → `method_not_found`; or terminal overrides outside `#[cfg(feature="kas")]` → default build links KAS code.

### Per-claim distinctness
Each claim has its own named oracle/test; a failure localizes to one claim. C8 and C9
both touch exit status but assert different outputs (status fields vs the `output`
string + nested-wire shape). C2 (cyril emits flat) and C2L (KAS honors flat) are split
so a failure says *which side* drifted.

## Negative space (what KAS-5b deliberately does NOT do)

1. **`output_byte_limit` truncation** — always `truncated:false` + full output; not reachable @ 2.10.0. Tracked **cyril-1rpv**.
2. **Reaping a session's terminals on cancel/turn-end** (orphan-on-cancel) — release/kill reap on the happy path; a cancelled-without-release `sleep` lingers until it exits. Tracked **cyril-3lh8**.
3. **`_kiro/fs/*` / `_kiro/terminal/*` supersets beyond `shell_type`** — KAS routes directory-list and delete through the *shell* (`terminal/create` `ls`/`rm`), never these methods (prove-it #1); not implemented.
4. **PTY/TTY allocation** — cyril pipes stdout+stderr; programs that demand a tty get none. KAS's shell tool is non-interactive.
5. **Stages gate/transform of terminal host-io** — like KAS-5a, the loop-mediation seam is deferred to its first consumer **cyril-g9vt**; terminal resolves directly in the `#[cfg(feature="kas")]` `KiroClient` override.
6. **No cyril-imposed command policy/sandboxing** — cyril mirrors KAS's flow (`terminal/create` is permission-gated by KAS's own `request_permission`, handled by the unchanged approval overlay).
7. **No live Windows/WSL validation** — cwd translation is unit-tested both directions (reusing KAS-5a's `to_native`), but a live Windows KAS run is not in this slice (Linux dev box). The WSL-internal-path edge for a Windows host is **cyril-8tq6**.

## Tracker references
- **cyril-ufie** (this issue) — KAS-5b terminal host-callbacks; `blocks` edge from cyril-7bdu (KAS-5a seam lands first).
- **cyril-1rpv** — `output_byte_limit` truncation follow-up (filed from this design; `discovered-from` cyril-ufie).
- **cyril-3lh8** — orphan-on-cancel terminal reaping (filed from this design; `discovered-from` cyril-ufie).
- **cyril-g9vt** — loop host-io mediation seam (shared with KAS-5a; terminal resolves directly until then).
- **cyril-8tq6** — WSL-internal path translation for a Windows host (applies to terminal `cwd`).
