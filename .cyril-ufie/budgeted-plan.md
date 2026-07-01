# KAS-5b — terminal host-callback responders: budgeted plan (cyril-ufie)

From the approved design (`.cyril-ufie/falsifiable-design.md`, cheapest falsifier C2
**passed**). 6 slices, built bottom-up: `TerminalRegistry` (create → read → terminate),
wire into `KiroClient`, prove non-blocking, then flip the capability **live last** so
KAS never routes a terminal callback to a missing responder.

**Lifecycle model (design-committed, "Option B"):** `create` spawns a piped child and
returns the id; `wait`/`kill` move the child out of the registry (dropping the `RefCell`
borrow) and `child.wait_with_output().await` (drains both pipes *while* waiting — no
pipe-buffer deadlock); `release` kills + reaps. The undrained-pipe window before `wait`
is sub-ms (capture shows KAS calls `wait` immediately after `create`); the chatty-command
risk if KAS ever delays `wait` is tracked **cyril-r3t6**.

**Shared invariants (all slices):**
- **Never hold a `RefCell` borrow across `.await`** — take the child out in a scoped
  borrow, await, re-borrow to store. Violation → `BorrowMutError` panic under concurrency.
- **Diagnostics via `tracing` only** (→ log file/stderr), like `host_io::io_err`. The
  command's own stdout/stderr is **data** captured into the ACP `output` field (returned
  over the wire), never written to cyril's fds. No `println!`.
- **`#[cfg(feature = "kas")]`** on every terminal item (default build links no KAS code).

---

## Slice 1: `TerminalRegistry` + `create_terminal` resolver

**Claim:** C3 (create returns id immediately, no exit await), C4 (ids unique), C6
(nonexistent command → Err, not panic), C7 (cwd honored / non-absolute rejected).
**Oracle:** wall-clock around create (C3); string compare of two ids (C4); the test
asserts `Err` + the task survives (C6); `has_root()` pure fn + tokio spawn ENOENT on a
missing cwd dir (C7) — all independent of the registry.
**Stress fixture (expected output written first):**
- A — two `create` calls before any release → ids `term-1` ≠ `term-2`. *(bug: id from a
  constant/cwd-hash → collision.)*
- B — `create{command:"definitely-not-a-real-binary-xyz"}` → `Err(-32603)`, no panic.
  *(bug: `.spawn().expect()`/`.unwrap()` → bridge task panics.)*
- C — `create{command:"echo", cwd:"relative/x"}` → `Err(-32602)` "must be absolute".
  *(bug: no `has_root()` guard → relative cwd silently runs in process cwd.)*
- D — `create{command:"echo", cwd:"/nonexistent-xyz-dir"}` → `Err(-32603)` (spawn ENOENT).
  *(bug: cwd ignored → `echo` spawns fine in process cwd → `Ok` → fixture fails, proving
  `current_dir` is wired without needing to read output.)*
**Loop budget:** no new cyril loop. Id alloc `O(1)` (`Cell<u64>` increment); `HashMap`
insert `O(1)` amortized. Concurrent live terminals `N`: KAS runs them effectively
sequentially (capture), bound `N ≲ 10`; registry is `O(N)` space, no scan.
**Wall budget:** n/a (create does not await the command).
**Files:** NEW `crates/cyril-core/src/protocol/kas/terminal_io.rs`;
`crates/cyril-core/src/protocol/kas/mod.rs` (+1 line `pub(crate) mod terminal_io;`).
**Doc-comment contracts:** cwd-absoluteness is **load-bearing for correctness** (a
relative cwd silently runs the wrong directory) → **runtime check** `has_root()` →
`-32602` (reuse `host_io::to_native_checked`'s exact rationale, host_io.rs:75), NOT
`debug_assert!`.

**Code (advisory):**
```rust
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use agent_client_protocol as acp;
use tokio::process::Child;

pub(crate) struct TerminalRegistry {
    inner: RefCell<HashMap<acp::TerminalId, Entry>>,
    counter: Cell<u64>,
}
enum Entry { Running(Option<Child>), Exited { output: String, status: acp::TerminalExitStatus } }

impl TerminalRegistry {
    pub(crate) fn new() -> Self { Self { inner: RefCell::new(HashMap::new()), counter: Cell::new(0) } }

    pub(crate) fn create(&self, req: &acp::CreateTerminalRequest)
        -> acp::Result<acp::CreateTerminalResponse> {
        let cwd = match &req.cwd {
            Some(p) => Some(host_io_abs_check(p)?),     // has_root() -> -32602, then to_native
            None => None,
        };
        let mut cmd = tokio::process::Command::new(&req.command);
        cmd.args(&req.args)
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());
        if let Some(d) = cwd { cmd.current_dir(d); }
        for e in &req.env { cmd.env(&e.name, &e.value); }
        let child = cmd.spawn().map_err(|e| spawn_err(&req.command, e))?;  // -32603, no panic
        let n = self.counter.get() + 1; self.counter.set(n);
        let id = acp::TerminalId::new(format!("term-{n}"));
        self.inner.borrow_mut().insert(id.clone(), Entry::Running(Some(child)));
        Ok(acp::CreateTerminalResponse::new(id))
    }
}
```
**Verification:**
- [ ] Unit tests (fixtures A–D) pass
- [ ] Stress fixtures A–D produce the expected `Err`/id outcomes
- [ ] prove-it oracle: `probe.rs`/`probe2.rs` still agree (request round-trip, spawn-err)
- [ ] Loop/wall budgets hold (O(1) create; no loop)

---

## Slice 2: `wait_for_terminal_exit` + `terminal_output` resolvers

**Claim:** C2 (wait reply FLAT `{exitCode,signal}`), C8 (non-zero exit & signal status),
C9 (output = combined stdout+stderr, nested `exitStatus` wire), C5-partial (unknown id on
wait/output → `Err`, not panic).
**Oracle:** `serde_json` of the typed acp response (C2, already passed in `probe.rs`);
`sh -c 'exit 42'` exit code vs the wire (C8); `sh -c 'echo OUT; echo ERR 1>&2'` →
`output` contains both (C9, passed in `probe2.rs`); a never-created id → `Err` (C5).
**Stress fixture (expected first):**
- E — build the wait reply for exit 42; serialize → `{"exitCode":42,"signal":null}`, asserts
  **no** substring `"exitStatus"`. *(bug: hand-built `json!({"exitStatus":{…}})` → contains
  the wrapper; the prove-it trap.)*
- F — `create{sh -c 'exit 42'}`, `wait` → `exitCode=Some(42), signal=None`. *(bug:
  `exit_code(0)` default → reports 0.)*
- G — `create{pwd, cwd:<tmp>}`, `wait`+`output` → `output.trim()` == `<tmp>` realpath.
  *(completes C7 end-to-end; bug: cwd ignored → process cwd.)*
- H — `create{sh -c 'echo OUT; echo ERR 1>&2'}`, `wait`+`output` → `output` contains
  `OUT` **and** `ERR`; the serialized output reply contains nested `"exitStatus"`. *(bug:
  stdout-only capture drops `ERR`; or flattened/None exit_status.)*
- I — `wait`/`output` on `term-999` (never created) → `Err(-32602)`, no panic. *(bug:
  `borrow().get(id).unwrap()` → panic.)*
**Loop budget:** no new cyril loop. `output` builds `format!("{stdout}{stderr}")` =
`O(output bytes)`, single concat over bytes cyril already holds from `wait_with_output`.
Production scale: a shell command's one-turn output — typically KB, occasionally MB
(build logs); unbounded-in-principle memory is the **same** edge `output_byte_limit`
addresses, deferred **cyril-1rpv**. No per-byte syscall; well under 10^6 ops for KB-MB.
**Wall budget:** `wait_for_terminal_exit` awaits the command's runtime, bounded by KAS's
≤60s tool timeout; cyril adds no polling/busy-wait (pure `tokio` await).
**Files:** `crates/cyril-core/src/protocol/kas/terminal_io.rs` (extend).
**Doc-comment contracts:** unknown-id is **load-bearing** (a wrong/default terminal =
silently wrong) → **runtime check** returning `-32602`, not `debug_assert!`.

**Code (advisory):**
```rust
pub(crate) async fn wait(&self, req: &acp::WaitForTerminalExitRequest)
    -> acp::Result<acp::WaitForTerminalExitResponse> {
    let child = self.take_running_child(&req.terminal_id)?;     // scoped borrow, dropped here
    let status = match child {
        Some(c) => { let out = c.wait_with_output().await.map_err(wait_err)?;   // await: no borrow held
                     self.store_exited(&req.terminal_id, &out); to_exit_status(&out.status) }
        None => self.exited_status(&req.terminal_id)?,           // already Exited: return stored
    };
    Ok(acp::WaitForTerminalExitResponse::new(status))           // FLAT via serde(flatten)
}
```
**Verification:**
- [ ] Unit tests (E–I) pass
- [ ] Stress fixtures E–I produce expected outcomes (esp. E: no `exitStatus` substring)
- [ ] prove-it oracle: `probe.rs` flat-wire + `probe2.rs` signal/combined still agree
- [ ] Loop/wall budgets hold (output O(bytes); wait awaits, no busy-loop)

---

## Slice 3: `release_terminal` + `kill_terminal` resolvers

**Claim:** C10 (release kills + reaps + frees id), C11 (kill terminates but keeps id),
C5-complete (release/kill on unknown → Err).
**Oracle:** OS process liveness via the child's pid (`Child::id()`) checked from the test
(`kill -0` / waitpid), independent of the registry (C10); the post-kill `wait` resolving
with `signal=Some` (C11); a never-created id → `Err` (C5).
**Stress fixture (expected first):**
- J — `create{sleep 30}`, record pid; `release`; then `output` on that id → `Err(-32602)`
  (freed) AND the pid is no longer alive. *(bug: `release` drops the entry without
  `start_kill` → `sleep` orphaned and survives; tokio Child does **not** kill on drop.)*
- K — `create{sleep 30}`, `kill`; then `wait` on the **same** id → `Ok` with `signal=Some`
  (SIGKILL), NOT `-32602`. *(bug: `kill` implemented as `release` → removes entry → `wait`
  is unknown-id.)*
- L — `release`/`kill` on `term-999` → `Err(-32602)`, no panic.
**Loop budget:** no new cyril loop; `start_kill` + one `wait()`/`wait_with_output()` await
per call, `O(1)` registry ops.
**Wall budget:** `release`/`kill` await only the post-SIGKILL reap (≈ immediate); no poll.
**Files:** `crates/cyril-core/src/protocol/kas/terminal_io.rs` (extend).
**Doc-comment contracts:** "after `release`, the id is invalid" is **load-bearing** (a
post-release op on a freed id must not resurrect/panic) → enforced by removal + the
unknown-id `-32602` runtime check (C5).

**Code (advisory):**
```rust
pub(crate) async fn release(&self, req: &acp::ReleaseTerminalRequest)
    -> acp::Result<acp::ReleaseTerminalResponse> {
    match self.take_running_child(&req.terminal_id)? {          // -32602 if unknown
        Some(mut c) => { let _ = c.start_kill(); let _ = c.wait().await; } // reap, discard output
        None => {}                                              // already exited
    }
    self.inner.borrow_mut().remove(&req.terminal_id);
    Ok(acp::ReleaseTerminalResponse::new())
}
// kill: start_kill then wait_with_output, store Exited{output, signal-status}, KEEP entry.
```
**Verification:**
- [ ] Unit/integration tests (J–L) pass
- [ ] Stress fixtures J–L produce expected outcomes (J: pid dead + id freed)
- [ ] prove-it oracle: release/kill replies are `{}` (probe.rs); still agree
- [ ] Loop/wall budgets hold (O(1), reap-await only)

---

## Slice 4: wire 5 typed overrides + `terminals` field + `shell_type` ext arm into `KiroClient`

**Claim:** C13 (shell_type via ext path + cfg-gating), and integration reachability of
C3–C11 through the `acp::Client` trait (override reached, not `method_not_found`).
**Oracle:** construct `KiroClient(KasEngine)`, call `create_terminal` via the trait →
`Ok(id)` not `method_not_found` (mirrors KAS-5a's `read_text_file_override` test); ext
reply has a `shellType` field; a no-`kas` `cargo build` (CI matrix) for cfg-gating.
**Stress fixture (expected first):**
- M — `KiroClient(KasEngine).create_terminal(req)` via the trait → `Ok`, NOT
  `Err(method_not_found)`. *(bug: override missing / miswired → default fires.)*
- N — `ext_method{method:"kiro/terminal/shell_type"}` → `Ok` with a `shellType` value.
  *(bug: matching the un-stripped `_kiro/terminal/shell_type` — acp strips the `_` inbound,
  auth.rs:20 — → no match → `default_ext_response` null.)*
- O — `cargo build -p cyril-core` (no `--features kas`) compiles; terminal overrides
  absent → default `method_not_found`. *(bug: an override outside `#[cfg(feature="kas")]`
  links KAS code into the default build / fails to compile.)*
**Loop budget:** no loop; 5 thin delegations + one string match.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs` (field + 5 overrides + ext arm).
**Doc-comment contracts:** none new (delegates to slice 1–3 resolvers, which own the
contracts). `shellType` value derivation (`$SHELL` basename on Unix; `"powershell"` on
Windows) is a **sanity-level** default — a wrong shell string degrades a hint, not
correctness → no runtime guard needed.
**Output stream:** the overrides return values over the ACP wire (data); no fd writes.

**Code (advisory):**
```rust
// field on KiroClient: terminals: crate::protocol::kas::terminal_io::TerminalRegistry  (#[cfg(kas)])
#[cfg(feature = "kas")]
async fn create_terminal(&self, args: acp::CreateTerminalRequest)
    -> acp::Result<acp::CreateTerminalResponse> { self.terminals.create(&args) }
// terminal_output/wait_for_terminal_exit/release_terminal/kill_terminal: same one-line delegation.
// handle_ext_request: add
//   if args.method.as_ref() == "kiro/terminal/shell_type" { return respond_shell_type(); }
```
**Verification:**
- [ ] Unit/integration tests (M–O) pass; default-build (no-kas) compiles
- [ ] Stress fixtures M–O produce expected outcomes
- [ ] prove-it oracle: end-to-end create→wait→output→release via the trait matches probe
- [ ] Loop/wall budgets hold (no loop)

---

## Slice 5: non-blocking concurrency proof (C12)

**Claim:** C12 (a slow `wait_for_terminal_exit` does not starve the single-threaded
runtime; no `RefCell` borrow held across `.await`).
**Oracle:** wall-clock of the *fast* terminal on a `current_thread` + `LocalSet` harness,
independent of the registry internals — the fast op must finish while the slow one is
still running.
**Stress fixture (expected first):**
- P — on `#[tokio::test(flavor="current_thread")]` (mirrors the bridge runtime): start
  `wait` on a `create{sleep 2}` terminal as a `spawn_local` task; concurrently `create`+
  `wait` an `echo` terminal; assert the **echo** wait resolves in `< 500ms` (while `sleep`
  still runs) and **no** `BorrowMutError` panic occurs. *(bug 1: a `std::process` blocking
  wait pins the one thread → echo blocked ~2s → fixture fails. bug 2: `borrow_mut().get_mut(id)…
  .await` holds the borrow across await → second task's re-borrow panics.)*
**Loop budget:** test spawns 2 terminals → `O(1)`; no production loop introduced.
**Wall budget:** the test itself bounded < 2s (the `sleep 2`); the assertion margin
(500ms vs 2s) is generous → deterministic, not timing-flaky.
**Files:** `crates/cyril-core/src/protocol/kas/terminal_io.rs` tests **or**
`crates/cyril-core/tests/kas_terminal_nonblocking.rs` (integration).
**Doc-comment contracts:** none new — this slice *verifies* the borrow-across-await
invariant documented in slices 1–3, it adds no precondition.

**Verification:**
- [ ] Concurrency test P passes (fast < 500ms, no panic)
- [ ] Stress fixture P fails against a deliberately-blocking impl (TDD-inversion sanity)
- [ ] prove-it oracle: unaffected (offline; the same resolvers)
- [ ] Loop/wall budgets hold (test < 2s, generous margin)

---

## Slice 6: flip the capability live (go-live) + KAS non-zero-exit oracle (C1, C2L)

**Claim:** C1 (`KasEngine` advertises `terminal=true`, V2 stays empty), C2L (KAS *honors*
the flat wait reply for a non-zero exit).
**Oracle:** direct `client_capabilities()` struct assertion (C1); a **live KAS 2.10.0**
turn running a command that exits non-zero, asserting KAS surfaces the real code, not 0
(C2L — the prove-it residual).
**Stress fixture (expected first):**
- Q — `KasEngine.client_capabilities().terminal == true`; `V2Engine.client_capabilities()`
  stays `== ClientCapabilities::new()` (empty). *(bug: copy-paste the KAS caps body into
  V2 → V2 non-empty → parity break.)* This is the existing engine.rs:147 test with the
  `!caps.terminal` assertion **flipped** to `caps.terminal`.
- R (live, manual) — a KAS turn whose tool runs e.g. `sh -c 'exit 7'`; cyril replies the
  flat typed `WaitForTerminalExitResponse`; assert KAS reports the command failed with
  code 7 (a turn that treats it as success/0 falsifies the flat-is-honored claim).
**Loop budget:** n/a (one-line cap change).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/engine.rs` (`.terminal(true)` + flip test).
**Doc-comment contracts:** none.
**Ordering:** **must be last** — flipping the cap before slices 1–4 land would route live
KAS terminal callbacks to `method_not_found` and break KAS shell execution.

**Verification:**
- [ ] Unit test Q passes (KAS terminal true; V2 empty)
- [ ] Live oracle R run (or **explicit user risk-acceptance** to merge on offline evidence
      — acp `#[serde(flatten)]` + covenant §5 bare-ACP both say flat); record result
- [ ] prove-it oracle: a full live KAS turn drives create→wait→output→release end-to-end
- [ ] Loop/wall budgets hold (n/a)

---

## Plan Self-Review (step 7 — five lists, no gaps)

**1. Every loop — complexity + production scale:**
- Slice 1 `create`: no loop; id alloc + insert `O(1)`. ✓
- Slice 2 `output` concat: `O(output bytes)`, single concat, KB–MB, < 10^6 ops; unbounded-memory edge deferred **cyril-1rpv**. ✓
- Slices 3/4/5/6: no new loop (await + `O(1)` map ops + one string match). ✓
- *No loop is annotated `O(?)`; none exceeds 10^6 ops / 10^3 syscalls.*

**2. Every fixture — bug class it fails under:**
- A id-collision · B spawn-panic · C relative-cwd-silent-process-cwd · D cwd-ignored ·
  E nested-wire-trap · F exit-code-defaulted-0 · G cwd-ignored-e2e · H stderr-dropped ·
  I unknown-id-panic · J release-no-kill-orphan · K kill-equals-release · L unknown-id ·
  M override-miswired-method_not_found · N un-stripped-prefix-no-match · O cfg-leak ·
  P thread-pin / borrow-across-await · Q caps-copy-paste-parity-break · R KAS-mis-parses-flat.
  *Every fixture targets a named bug, not a happy-path exercise.* ✓

**3. Every doc-comment precondition — classification + enforcement:**
- cwd-absoluteness (slice 1): **load-bearing** → runtime `has_root()` → `-32602`. ✓
- unknown-id (slices 2/3): **load-bearing** → runtime `-32602`. ✓
- post-release id-invalid (slice 3): **load-bearing** → removal + `-32602`. ✓
- `shellType` value (slice 4): **sanity-hint** → no guard (degrades a hint, not correctness). ✓
- borrow-not-across-await (slices 1–3): structural invariant → verified by slice 5, not a caller precondition. ✓
  *No documented precondition ships without matching enforcement.*

**4. Every write target — data vs diagnostic:**
- Resolver return values → ACP wire = **data**. ✓
- `tracing::{debug,warn}` on errors → log file/stderr = **diagnostic**. ✓
- Command's own stdout/stderr → captured into ACP `output` field = **data** (not cyril fds). ✓
  *No `println!`; no data-to-stderr or diagnostic-to-stdout.*

**5. Every tracker reference — resolves to a covering issue:**
- **cyril-1rpv** (output_byte_limit / unbounded output) — verified, covers slice 2 note. ✓
- **cyril-3lh8** (orphan-on-cancel reaping) — verified, design negative space. ✓
- **cyril-r3t6** (drain-at-create if KAS delays wait) — verified, covers the Option-B window. ✓
- **cyril-g9vt** (loop-mediation seam) / **cyril-8tq6** (WSL cwd path) — verified, design. ✓
  *Every deferral cites an existing issue whose content covers the deferred work.*

## Claim coverage (matches design's 13 + live)
C1→S6 · C2→S2 · C2L→S6 · C3→S1 · C4→S1 · C5→S2+S3 · C6→S1 · C7→S1+S2 · C8→S2 ·
C9→S2 · C10→S3 · C11→S3 · C12→S5 · C13→S4. **All covered.**
