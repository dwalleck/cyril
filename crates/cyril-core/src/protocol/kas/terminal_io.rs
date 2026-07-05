//! KAS-5b terminal host-I/O responders (cyril-ufie): cyril answers the
//! `terminal/*` server→client requests KAS sends when cyril advertises the
//! `terminal` capability (KasEngine). KAS delegates shell execution to the host;
//! this registry makes cyril the executor — the audit/gate/transform point
//! (ADR-0003), and the one that stresses the non-blocking invariant hardest
//! (a command can run up to 60s).
//!
//! Wire shapes verified @ 2.10.0 (`.cyril-ufie/PROVE-IT.md`,
//! `.cyril-7bdu/host_callbacks_2.10.0.json`): bare ACP `terminal/{create,output,
//! wait_for_exit,release,kill}`, every call carries `sessionId`, `cwd` absolute.
//!
//! **Lifecycle ("Option B"):** `create` spawns a piped child and returns the id
//! immediately; `wait` moves the child out of the registry (dropping the `RefCell`
//! borrow) and [`wait_with_output_killable`] drains both pipes *while* waiting —
//! no pipe-buffer deadlock — while also watching the terminal's kill signal so a
//! concurrent `kill`/`release` terminates the child through the task that owns it
//! (cyril-lw67). The undrained-pipe window before `wait` is sub-ms (KAS calls
//! `wait` immediately after `create`); the chatty-command risk if KAS ever
//! delays `wait` is tracked **cyril-r3t6**.
//!
//! **Non-blocking invariant (ADR-0004): `wait`/`release`/`kill` MUST await
//! `tokio::process`, never a thread-pinning `std::process` wait** (the bridge is a
//! single-threaded `current_thread` + `LocalSet` runtime; rpc.rs:272 spawns each
//! request, but all on one thread). **Never hold a `RefCell` borrow across an
//! `.await`** — take the child out in a scoped borrow, await, re-borrow to store —
//! else a concurrent op panics `BorrowMutError`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use agent_client_protocol as acp;
use tokio::io::AsyncReadExt as _;
use tokio::process::Child;
use tokio::sync::Notify;

/// A process-lifetime registry of live terminals, one per `KiroClient`
/// (`!Send`, single bridge thread — no lock, mirroring `tool_call_inputs`).
pub(crate) struct TerminalRegistry {
    inner: RefCell<HashMap<acp::TerminalId, Entry>>,
    counter: Cell<u64>,
}

/// A tracked terminal. `Running` holds the spawned child until `wait`/`kill` takes
/// it out (`None` while a take is in flight); `Exited` caches the captured combined
/// output + status so a later `output`/`wait` is a snapshot, not a re-run.
enum Entry {
    Running {
        /// The session that created this terminal (`CreateTerminalRequest.
        /// session_id`), so `reap_session` can find a cancelled session's live
        /// children by linear scan — the terminal id stays the primary key
        /// (cyril-3lh8).
        session_id: acp::SessionId,
        /// The spawned child, `None` while an awaiting op has taken it out.
        child: Option<Child>,
        /// Kill signal for the in-flight owner. The rpc layer spawns every
        /// inbound request as its own concurrent task, so `kill`/`release` land
        /// WHILE a `wait` owns the child (KAS's command-timeout pattern does
        /// this on every kill). They cannot `start_kill` a child they don't
        /// hold, so they notify this and the owning task kills + reaps
        /// (cyril-lw67).
        kill_signal: Rc<Notify>,
    },
    Exited {
        output: String,
        status: acp::TerminalExitStatus,
    },
}

/// Outcome of taking a terminal's child out of the registry for an awaiting op.
enum Taken {
    /// The live child, removed from its `Running` slot — caller awaits + reaps
    /// it, watching the kill signal for a concurrent `kill`/`release`.
    Child(Child, Rc<Notify>),
    /// The terminal already exited; carries its cached status (for `wait`).
    AlreadyExited(acp::TerminalExitStatus),
    /// Another op already took the child and is awaiting it. Carries the kill
    /// signal so `kill`/`release` can terminate the child through the in-flight
    /// owner instead of silently falling through (cyril-lw67) — with KAS's
    /// create→wait-immediately pattern, every kill arrives in this state.
    InFlight(Rc<Notify>),
}

impl TerminalRegistry {
    pub(crate) fn new() -> Self {
        Self {
            inner: RefCell::new(HashMap::new()),
            counter: Cell::new(0),
        }
    }

    /// Answer `terminal/create`: spawn `command` (piped stdout+stderr) in the
    /// translated `cwd`, assign a process-unique `term-{n}` id, and return it
    /// **immediately** — no await on exit (the non-blocking entry point). A spawn
    /// failure (nonexistent command, missing cwd) returns `Err` (`-32603`), never
    /// panics; a non-absolute `cwd` is rejected `-32602`.
    pub(crate) fn create(
        &self,
        req: &acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        let cwd = match &req.cwd {
            // Reuse the fs host-io contract: absolute-or-`-32602`, then translate
            // (Windows `/mnt/c`→`C:\`; Linux no-op). Load-bearing: a relative cwd
            // would silently run the command in the bridge's process cwd.
            Some(p) => Some(super::host_io::to_native_checked(p)?),
            None => None,
        };
        let mut cmd = tokio::process::Command::new(&req.command);
        cmd.args(&req.args)
            // stdin MUST be null, not the inherited default: the bridge's stdin is
            // cyril's TUI terminal. A KAS command that reads stdin (`cat`, `grep`
            // with no file, a REPL) would otherwise attach to that terminal —
            // blocking `wait_for_exit` forever on the never-arriving input AND
            // stealing the user's keystrokes. ACP `terminal/*` has no stdin-input
            // method, so these terminals are provably non-interactive; null gives an
            // immediate EOF.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            // SIGKILL the child if its handle is dropped un-reaped (cyril-ba5x):
            // on Shutdown / bridge-thread death / app exit the LocalSet drops,
            // taking the registry (idle Child) and any pending wait future
            // (in-flight Child) with it — without this, those children outlive
            // cyril entirely. Normal paths (`wait`/`kill`/`release`) reap before
            // dropping, so this changes nothing for a completed command.
            .kill_on_drop(true);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        // O(env): KAS sends none @ 2.10.0; populated is pass-through.
        for e in &req.env {
            cmd.env(&e.name, &e.value);
        }
        let child = cmd.spawn().map_err(|e| spawn_err(&req.command, e))?;

        let n = self.counter.get().saturating_add(1);
        self.counter.set(n);
        let id = acp::TerminalId::new(format!("term-{n}"));
        self.inner.borrow_mut().insert(
            id.clone(),
            Entry::Running {
                session_id: req.session_id.clone(),
                child: Some(child),
                kill_signal: Rc::new(Notify::new()),
            },
        );
        Ok(acp::CreateTerminalResponse::new(id))
    }

    /// Answer `terminal/wait_for_exit`: await the command's exit and return its
    /// status. Reply is **FLAT** `{exitCode, signal}` (typed
    /// `WaitForTerminalExitResponse`, `#[serde(flatten)]`) — NOT nested
    /// `{exitStatus:{…}}` (the prove-it finding, `.cyril-ufie/PROVE-IT.md`).
    /// Unknown id → `-32602`. Drains both pipes via `wait_with_output` so a chatty
    /// command can't pipe-deadlock; awaits `tokio::process`, never `std::process`.
    pub(crate) async fn wait(
        &self,
        req: &acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        // take_child drops the RefCell borrow before we await (the no-borrow-across-
        // await invariant). A sequential KAS never double-waits; InFlight is defensive.
        let (child, kill_signal) = match self.take_child(&req.terminal_id)? {
            Taken::Child(child, kill_signal) => (child, kill_signal),
            Taken::AlreadyExited(status) => {
                return Ok(acp::WaitForTerminalExitResponse::new(status));
            }
            Taken::InFlight(_) => {
                return Err(acp::Error::new(
                    -32603,
                    format!("terminal {} wait already in progress", req.terminal_id),
                ));
            }
        };
        let out = match wait_with_output_killable(child, &kill_signal).await {
            Ok(out) => out,
            // take_child left a Running(None) slot; a reap error must free it (not
            // leave the id wedged in a permanent InFlight state — a retried wait
            // would otherwise get "wait already in progress" instead of a clean
            // unknown-id) before surfacing the error to KAS.
            Err(e) => {
                self.inner.borrow_mut().remove(&req.terminal_id);
                return Err(wait_err(&req.terminal_id, e));
            }
        };
        let status = exit_status(&out.status);
        self.store_exited(&req.terminal_id, combine_output(&out), status.clone());
        Ok(acp::WaitForTerminalExitResponse::new(status))
    }

    /// Answer `terminal/output`: snapshot the terminal's current output + status
    /// **without** awaiting. Reply is `{output, truncated, exitStatus}` (nested
    /// `exit_status`). `output` is the command's **combined stdout+stderr** once it
    /// has exited; a still-`Running` terminal returns empty (Option B captures at
    /// `wait`). Unknown id → `-32602`. `truncated` is always `false` (cyril-1rpv).
    pub(crate) fn output(
        &self,
        req: &acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        let map = self.inner.borrow();
        match map.get(&req.terminal_id) {
            None => Err(unknown_terminal(&req.terminal_id)),
            Some(Entry::Exited { output, status }) => {
                Ok(acp::TerminalOutputResponse::new(output.clone(), false)
                    .exit_status(status.clone()))
            }
            Some(Entry::Running { .. }) => {
                Ok(acp::TerminalOutputResponse::new(String::new(), false))
            }
        }
    }

    /// Answer `terminal/release`: kill a still-running child, reap it (await the
    /// exit so no zombie/orphan is left), and **free the id** — subsequent ops on
    /// it become unknown-id `-32602`. Unknown id → `-32602`. Awaits `tokio::process`.
    pub(crate) async fn release(
        &self,
        req: &acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        match self.take_child(&req.terminal_id)? {
            Taken::Child(mut child, _) => {
                // SIGKILL then reap. Without the wait the child is a zombie; tokio's
                // Child does NOT kill/reap on drop. Output is discarded (id is freed).
                // Both ops are best-effort (the child may have already exited), but a
                // failure is logged, not swallowed (CLAUDE.md: no discarded Results).
                if let Err(e) = child.start_kill() {
                    tracing::debug!(terminal_id = %req.terminal_id, error = %e, "KAS terminal release: start_kill failed");
                }
                if let Err(e) = child.wait().await {
                    tracing::debug!(terminal_id = %req.terminal_id, error = %e, "KAS terminal release: reap failed (possible zombie)");
                }
            }
            // A pending wait/kill owns the child; release can't SIGKILL a child it
            // doesn't hold, so it signals the owner to start_kill + reap
            // (cyril-lw67 — the old fall-through killed nothing). The remove below
            // doubles as the released tombstone: the owner's completion path
            // (store_exited) finds the id gone and discards the output instead of
            // resurrecting the released id.
            Taken::InFlight(kill_signal) => kill_signal.notify_one(),
            Taken::AlreadyExited(_) => {}
        }
        self.inner.borrow_mut().remove(&req.terminal_id);
        Ok(acp::ReleaseTerminalResponse::new())
    }

    /// Answer `terminal/kill`: terminate a running child but **keep the id valid** —
    /// a later `terminal_output`/`wait_for_terminal_exit` still resolves (KAS's
    /// command-timeout pattern: kill, then read the partial output). Reaps via
    /// `wait_with_output`, caching the captured output + signal status. Unknown id →
    /// `-32602`.
    pub(crate) async fn kill(
        &self,
        req: &acp::KillTerminalRequest,
    ) -> acp::Result<acp::KillTerminalResponse> {
        match self.take_child(&req.terminal_id)? {
            Taken::Child(mut child, _) => {
                if let Err(e) = child.start_kill() {
                    tracing::debug!(terminal_id = %req.terminal_id, error = %e, "KAS terminal kill: start_kill failed");
                }
                let out = match child.wait_with_output().await {
                    Ok(out) => out,
                    // take_child left a Running(None) slot; a reap error must free it
                    // (not leave the id wedged in a permanent InFlight state) before
                    // surfacing the error to KAS.
                    Err(e) => {
                        self.inner.borrow_mut().remove(&req.terminal_id);
                        return Err(wait_err(&req.terminal_id, e));
                    }
                };
                self.store_exited(
                    &req.terminal_id,
                    combine_output(&out),
                    exit_status(&out.status),
                );
            }
            // With KAS's create→wait-immediately pattern, EVERY kill lands here: a
            // pending wait owns the child. Signal it to start_kill from the task
            // that holds the Child (cyril-lw67 — the old fall-through replied Ok
            // having killed nothing, hanging the turn); the pending wait resolves
            // with the killed status and caches it, keeping the id valid per the
            // kill contract.
            Taken::InFlight(kill_signal) => kill_signal.notify_one(),
            Taken::AlreadyExited(_) => {}
        }
        Ok(acp::KillTerminalResponse::new())
    }

    /// Kill + reap every terminal a session still has **running** (cyril-3lh8,
    /// KAS-5b follow-up). A CANCELLED turn may end without KAS ever sending
    /// `terminal/release`, so its live children would run to natural exit as
    /// orphans (a 60s sleep = 60s orphan; a child wedged writing to a full pipe
    /// never exits at all — only a kill covers it). The bridge loop triggers
    /// this from its CancelRequest arm ONLY — reaping at turn-end could race an
    /// in-flight `release`.
    ///
    /// Applies the [`Self::kill`] contract, NOT release: ids stay valid with
    /// partial output cached, so KAS's late `terminal/output`/`terminal/release`
    /// for those ids still resolve instead of erroring `-32602`. The linear scan
    /// collects matching ids in one scoped borrow (the terminal id stays the
    /// primary key), then delegates each to `kill` — take-then-await, and
    /// in-flight children are terminated through their owning `wait` via the
    /// kill signal (cyril-lw67).
    pub(crate) async fn reap_session(&self, session_id: &acp::SessionId) {
        let running: Vec<acp::TerminalId> = self
            .inner
            .borrow()
            .iter()
            .filter_map(|(id, entry)| match entry {
                Entry::Running {
                    session_id: sid, ..
                } if sid == session_id => Some(id.clone()),
                _ => None,
            })
            .collect();
        for id in running {
            tracing::debug!(terminal_id = %id, session_id = %session_id, "reaping cancelled session's live terminal");
            if let Err(e) = self
                .kill(&acp::KillTerminalRequest::new(
                    session_id.clone(),
                    id.clone(),
                ))
                .await
            {
                // Best-effort per terminal: the id can vanish legitimately (a
                // concurrent release raced the scan) — log and keep reaping.
                tracing::debug!(terminal_id = %id, error = %e, "KAS terminal reap: kill failed");
            }
        }
    }

    /// Take a terminal's live child out of the registry in a scoped `RefCell`
    /// borrow so the caller can `.await` its exit **without holding the borrow**
    /// (the no-borrow-across-await invariant). `Running` leaves a `None` slot;
    /// `Exited` returns the cached status; an unknown id is `-32602`.
    fn take_child(&self, id: &acp::TerminalId) -> acp::Result<Taken> {
        let mut map = self.inner.borrow_mut();
        match map.get_mut(id) {
            None => Err(unknown_terminal(id)),
            Some(Entry::Exited { status, .. }) => Ok(Taken::AlreadyExited(status.clone())),
            Some(Entry::Running {
                child, kill_signal, ..
            }) => Ok(match child.take() {
                Some(child) => Taken::Child(child, Rc::clone(kill_signal)),
                None => Taken::InFlight(Rc::clone(kill_signal)),
            }),
        }
    }

    /// Cache a reaped terminal's captured output + status — unless the id was
    /// released while the reap was in flight. `release` removes the id as its
    /// tombstone; blindly `insert`ing here would resurrect a released id
    /// (violating the released-id → `-32602` contract) and leak the entry +
    /// captured output for the life of the bridge (cyril-lw67). While an op owns
    /// the child the only reachable states are `Running(None)` (overwrite with
    /// the snapshot) and absent (released — discard).
    fn store_exited(&self, id: &acp::TerminalId, output: String, status: acp::TerminalExitStatus) {
        match self.inner.borrow_mut().get_mut(id) {
            Some(entry) => *entry = Entry::Exited { output, status },
            None => {
                tracing::debug!(terminal_id = %id, "KAS terminal released during pending wait; discarding captured output");
            }
        }
    }
}

/// Await a taken child's exit while draining both pipes (the `wait_with_output`
/// contract — a chatty command can't pipe-buffer-deadlock) AND watching the
/// terminal's kill signal. `kill`/`release` can land while a `wait` owns the
/// child (the rpc layer spawns each inbound request as its own task); they
/// can't `start_kill` a child they don't hold, so they notify the signal and
/// THIS task — the owner — kills, then lets the wait resolve with the killed
/// status (cyril-lw67). `wait_with_output` itself consumes the child, so it
/// can't be combined with a kill hook; this reimplements its drain-while-wait
/// shape with a `select!` on the exit. `tokio::process::Child::wait` is
/// cancel-safe, so selecting over it is sound. A `notify_one` sent before this
/// task polls `notified()` is not lost — `Notify` stores the permit.
async fn wait_with_output_killable(
    mut child: Child,
    kill_signal: &Notify,
) -> std::io::Result<std::process::Output> {
    async fn drain(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut buf).await?;
        }
        Ok(buf)
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let exit = async {
        tokio::select! {
            res = child.wait() => return res,
            _ = kill_signal.notified() => {}
        }
        // Signaled by a concurrent kill/release: SIGKILL from the task that owns
        // the Child, then reap. start_kill on a child that already exited (but
        // is not yet reaped) is best-effort — logged, never fatal; the wait
        // below reaps either way.
        if let Err(e) = child.start_kill() {
            tracing::debug!(error = %e, "KAS terminal kill-signal: start_kill failed");
        }
        child.wait().await
    };
    let (status, stdout, stderr) = tokio::join!(exit, drain(stdout), drain(stderr));
    Ok(std::process::Output {
        status: status?,
        stdout: stdout?,
        stderr: stderr?,
    })
}

/// The (acp-stripped) method name for KAS's `_kiro/terminal/shell_type` host
/// callback. The acp crate strips the leading `_` inbound, so cyril matches the
/// `kiro/...` form — same convention as [`super::auth::GET_ACCESS_TOKEN_METHOD`].
pub(crate) const SHELL_TYPE_METHOD: &str = "kiro/terminal/shell_type";

/// Answer `_kiro/terminal/shell_type`: report the host shell so KAS parses and
/// formats commands for it. Returns `bash` — the KAS host is Unix/WSL and
/// bash-family semantics are near-universal there; this is the value the 2.10.0
/// prove-it turn replied and KAS accepted. The precise per-user shell is a hint,
/// not load-bearing (KAS sends commands pre-split into `{command, args}` that cyril
/// runs directly), so it is a constant, not env-sniffed.
pub(crate) fn respond_shell_type() -> acp::Result<acp::ExtResponse> {
    let body = serde_json::json!({ "shellType": "bash" });
    let raw = serde_json::value::RawValue::from_string(body.to_string())
        .map_err(|e| acp::Error::new(-32603, format!("serialize shell_type reply: {e}")))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

/// Combine a finished command's stdout and stderr into one terminal stream,
/// lossily decoding non-UTF-8 bytes (ACP `output` is a `String`). A real terminal
/// interleaves both; capturing stdout-only would drop a command's error output.
fn combine_output(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Map an OS `ExitStatus` to ACP's `TerminalExitStatus`: a normal exit sets
/// `exitCode`; a signal-terminated process (Unix) sets `signal` and leaves
/// `exitCode` `None` — never reports `exitCode:0` for a killed process.
fn exit_status(status: &std::process::ExitStatus) -> acp::TerminalExitStatus {
    let es = acp::TerminalExitStatus::new();
    let es = match status.code() {
        Some(code) => es.exit_code(code as u32),
        None => es,
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return es.signal(sig.to_string());
        }
    }
    es
}

/// `-32602` for an op on a terminal id that was never created (or already
/// released). Load-bearing: a wrong/default terminal would be silently wrong, so a
/// runtime error, not a panic or a fabricated empty result.
fn unknown_terminal(id: &acp::TerminalId) -> acp::Error {
    tracing::debug!(terminal_id = %id, "KAS terminal op on unknown id");
    acp::Error::new(-32602, format!("unknown terminal: {id}"))
}

/// Build a `-32603` error for a failed `terminal/create` spawn, logging the io
/// error (NotFound vs PermissionDenied) so wire/exec drift is diagnosable —
/// surface, don't swallow (CLAUDE.md). Distinct shape from `host_io::io_err`
/// (a command string, not a path), so not a duplicate.
fn spawn_err(command: &str, e: std::io::Error) -> acp::Error {
    tracing::debug!(command = %command, error = %e, "KAS terminal spawn failed");
    acp::Error::new(-32603, format!("spawn {command}: {e}"))
}

/// `-32603` for a failure while awaiting a terminal's exit (rare io error).
fn wait_err(id: &acp::TerminalId, e: std::io::Error) -> acp::Error {
    tracing::debug!(terminal_id = %id, error = %e, "KAS terminal wait failed");
    acp::Error::new(-32603, format!("wait terminal {id}: {e}"))
}

/// Liveness probes for the orphan/reap fences, shared with the bridge's
/// cancel-reap fence (cyril-3lh8). Probes via `ps` — portable across Linux AND
/// macOS; a `/proc/<pid>` read misreports on macOS (no procfs), failing these
/// fences before the code under test is even exercised.
#[cfg(all(test, unix))]
pub(crate) mod test_probe {
    #![allow(clippy::expect_used)]

    /// `true` once the process is gone OR is a zombie (SIGKILLed, awaiting
    /// reap — tokio reaps dropped children asynchronously, so a brief zombie
    /// is "dead" for the leaked-child criterion; a leaked `sleep 60` stays
    /// alive in state `S`).
    pub(crate) fn dead_or_zombie(pid: u32) -> bool {
        let out = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("spawn ps for the liveness probe");
        if !out.status.success() {
            return true; // ps knows no such pid: fully reaped
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stat = stdout.trim();
        stat.is_empty() || stat.starts_with('Z')
    }

    /// Poll [`dead_or_zombie`] up to 5s; panic if the child outlives the
    /// kill/reap/drop under test.
    pub(crate) async fn assert_process_dies(pid: u32) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if dead_or_zombie(pid) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("terminal child {pid} survived the kill/reap/drop under test (orphan leak)");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    #[cfg(unix)]
    use super::test_probe::{assert_process_dies, dead_or_zombie};
    use super::*;

    fn create_req(command: &str) -> acp::CreateTerminalRequest {
        acp::CreateTerminalRequest::new(acp::SessionId::new("s"), command)
    }
    fn sh(script: &str) -> acp::CreateTerminalRequest {
        create_req("sh").args(vec!["-c".to_string(), script.to_string()])
    }
    fn wait_req(id: &acp::TerminalId) -> acp::WaitForTerminalExitRequest {
        acp::WaitForTerminalExitRequest::new(acp::SessionId::new("s"), id.clone())
    }
    fn out_req(id: &acp::TerminalId) -> acp::TerminalOutputRequest {
        acp::TerminalOutputRequest::new(acp::SessionId::new("s"), id.clone())
    }

    #[tokio::test]
    async fn create_assigns_unique_ids() {
        // Fixture A: two creates before any release must get DISTINCT ids.
        // Fails if ids derive from a constant/cwd-hash instead of the counter.
        let reg = TerminalRegistry::new();
        let id1 = reg.create(&create_req("true")).unwrap().terminal_id;
        let id2 = reg.create(&create_req("true")).unwrap().terminal_id;
        assert_ne!(id1, id2, "concurrent terminals must get unique ids");
        assert_eq!(id1.to_string(), "term-1");
        assert_eq!(id2.to_string(), "term-2");
    }

    #[tokio::test]
    async fn create_returns_before_command_exits() {
        // Fixture (C3): create must return the id IMMEDIATELY, without awaiting the
        // command's exit. Creating a `sleep 5` and returning in <500ms proves it;
        // a refactor that made create await wait_with_output would take ~5s -> fail.
        let reg = TerminalRegistry::new();
        let t0 = std::time::Instant::now();
        let id = reg.create(&sh("sleep 5")).unwrap().terminal_id;
        let elapsed = t0.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "create must not await the command's exit (took {elapsed:?})"
        );
        // Reap the sleeper so the test leaves no orphan.
        reg.release(&release_req(&id)).await.unwrap();
    }

    #[tokio::test]
    async fn create_closes_stdin_so_stdin_readers_dont_hang() {
        // Fixture (stdin): a command that reads stdin (`cat` with no file arg) must
        // get an immediate EOF, not attach to the bridge's inherited terminal stdin.
        // With `.stdin(null())` cat exits promptly; a regression that dropped the
        // null (inherit) or used `piped()` without a writer would block forever, so
        // the 5s timeout guard fails. Guards the non-interactive-terminal invariant.
        let reg = TerminalRegistry::new();
        let id = reg.create(&create_req("cat")).unwrap().terminal_id;
        let resp =
            tokio::time::timeout(std::time::Duration::from_secs(5), reg.wait(&wait_req(&id)))
                .await
                .expect(
                    "cat with closed stdin must exit promptly (stdin must not be inherited/piped)",
                )
                .unwrap();
        assert_eq!(
            resp.exit_status.exit_code,
            Some(0),
            "cat reads stdin to EOF and exits 0 when stdin is null"
        );
    }

    #[tokio::test]
    async fn create_nonexistent_command_errors_not_panics() {
        // Fixture B: a command that does not exist must return Err (spawn failure),
        // never panic. Fails under `.spawn().unwrap()/.expect()`.
        let reg = TerminalRegistry::new();
        let err = reg
            .create(&create_req("definitely-not-a-real-binary-xyz"))
            .expect_err("nonexistent command must error");
        assert!(err.message.contains("spawn"), "spawn failure, got {err:?}");
    }

    #[tokio::test]
    async fn create_relative_cwd_rejected_absolute_error() {
        // Fixture C: a non-absolute cwd is rejected with the DISTINCT "must be
        // absolute" error — never silently run in the process cwd.
        let reg = TerminalRegistry::new();
        let req = create_req("echo").cwd(std::path::PathBuf::from("relative/x"));
        let err = reg.create(&req).expect_err("relative cwd must be rejected");
        assert!(
            err.message.contains("must be absolute"),
            "relative cwd rejected as non-absolute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn create_honors_cwd_missing_dir_errors() {
        // Fixture D: spawn `echo` with an absolute-but-nonexistent cwd. current_dir
        // makes spawn fail (ENOENT). If cwd were IGNORED, `echo` would spawn fine in
        // the process cwd -> Ok -> this catches the bug. Distinct from C: a "spawn"
        // failure, not a "must be absolute" rejection.
        let reg = TerminalRegistry::new();
        let req = create_req("echo").cwd(std::path::PathBuf::from("/nonexistent-xyz-dir-9k2"));
        let err = reg.create(&req).expect_err("missing cwd must fail spawn");
        assert!(
            err.message.contains("spawn") && !err.message.contains("must be absolute"),
            "missing cwd is a spawn failure (cwd was applied), got {err:?}"
        );
    }

    #[tokio::test]
    async fn wait_reply_is_flat_not_nested() {
        // Fixture E (the prove-it trap): the wait reply must serialize FLAT
        // {exitCode, signal}, NOT nested {exitStatus:{…}}. Fails if a resolver
        // hand-builds the nested shape the KAS-5a probe used.
        let reg = TerminalRegistry::new();
        let id = reg.create(&sh("exit 42")).unwrap().terminal_id;
        let resp = reg.wait(&wait_req(&id)).await.unwrap();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("exitStatus"),
            "wait reply must be flat, no exitStatus wrapper: {json}"
        );
        assert!(
            json.contains("exitCode"),
            "wait reply must carry exitCode: {json}"
        );
    }

    #[tokio::test]
    async fn wait_reports_nonzero_exit_code() {
        // Fixture F: a command exiting 42 reports exitCode=Some(42), signal=None.
        // Fails under an exit_code(0) default.
        let reg = TerminalRegistry::new();
        let id = reg.create(&sh("exit 42")).unwrap().terminal_id;
        let resp = reg.wait(&wait_req(&id)).await.unwrap();
        assert_eq!(resp.exit_status.exit_code, Some(42));
        assert_eq!(resp.exit_status.signal, None);
    }

    // Unix-only: `kill -9 $$` and the signal/no-exit-code shape are POSIX-specific.
    // Windows has no signals — a terminated process reports an exit code, not a
    // signal — so `exit_status` never populates `signal` there (its arm is
    // `#[cfg(unix)]`). This assertion set is meaningful only on Unix.
    #[cfg(unix)]
    #[tokio::test]
    async fn wait_reports_signal_not_exit_zero() {
        // Fixture F2: a self-SIGKILLed command reports exitCode=None, signal=Some —
        // never exitCode:0 for a killed process. Exercises exit_status's signal arm
        // directly via a self-SIGKILL, independent of the kill resolver.
        let reg = TerminalRegistry::new();
        let id = reg.create(&sh("kill -9 $$")).unwrap().terminal_id;
        let resp = reg.wait(&wait_req(&id)).await.unwrap();
        assert_eq!(resp.exit_status.exit_code, None, "signaled => no exit code");
        assert_eq!(resp.exit_status.signal.as_deref(), Some("9"), "SIGKILL=9");
    }

    #[tokio::test]
    async fn output_honors_cwd_and_combines_stdout_stderr() {
        // Fixture G+H: run in a tmp cwd (proves the command EXECUTES there, not just
        // that current_dir was set) and write to BOTH stdout and stderr. output must
        // contain both, and its wire reply must carry nested exitStatus.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "CWD-OK").unwrap();
        let reg = TerminalRegistry::new();
        // `cat marker.txt` (relative) only finds the file if cwd is honored; then
        // echo to stderr proves stderr is captured too.
        let req = sh("cat marker.txt; echo ERRLINE 1>&2").cwd(dir.path().to_path_buf());
        let id = reg.create(&req).unwrap().terminal_id;
        reg.wait(&wait_req(&id)).await.unwrap();
        let resp = reg.output(&out_req(&id)).unwrap();
        assert!(
            resp.output.contains("CWD-OK"),
            "cwd honored: {:?}",
            resp.output
        );
        assert!(
            resp.output.contains("ERRLINE"),
            "stderr combined: {:?}",
            resp.output
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            json.contains("exitStatus"),
            "output reply nests exitStatus: {json}"
        );
    }

    #[tokio::test]
    async fn unknown_id_errors_not_panics() {
        // Fixture I: wait/output on a never-created id must Err (-32602), not panic.
        // Fails under `borrow().get(id).unwrap()`.
        let reg = TerminalRegistry::new();
        let ghost = acp::TerminalId::new("term-999");
        let we = reg
            .wait(&wait_req(&ghost))
            .await
            .expect_err("unknown wait errs");
        assert!(we.message.contains("unknown terminal"), "got {we:?}");
        let oe = reg
            .output(&out_req(&ghost))
            .expect_err("unknown output errs");
        assert!(oe.message.contains("unknown terminal"), "got {oe:?}");
    }

    #[test]
    fn shell_type_reply_carries_shelltype() {
        // Fixture N (value half): the shell_type reply is `{shellType: "bash"}` —
        // the value the prove-it turn used and KAS accepted.
        let resp = respond_shell_type().unwrap();
        let json = resp.0.get();
        assert!(
            json.contains("shellType"),
            "reply must carry shellType: {json}"
        );
        assert!(json.contains("bash"), "shellType value is bash: {json}");
    }

    fn release_req(id: &acp::TerminalId) -> acp::ReleaseTerminalRequest {
        acp::ReleaseTerminalRequest::new(acp::SessionId::new("s"), id.clone())
    }
    fn kill_req(id: &acp::TerminalId) -> acp::KillTerminalRequest {
        acp::KillTerminalRequest::new(acp::SessionId::new("s"), id.clone())
    }

    #[tokio::test]
    async fn release_kills_child_and_frees_id() {
        // Fixture J: release must KILL a running child (not orphan it) AND free the
        // id. `sh -c 'sleep 1; touch marker'` writes the marker only if it runs to
        // completion; releasing kills `sh` before `touch`, so the marker stays
        // ABSENT. A buggy release that drops the entry WITHOUT start_kill leaves sh
        // running -> marker appears -> this fails. Also asserts the id is freed.
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker.txt");
        let req = sh("sleep 1; touch marker.txt").cwd(dir.path().to_path_buf());
        let reg = TerminalRegistry::new();
        let id = reg.create(&req).unwrap().terminal_id;
        reg.release(&release_req(&id)).await.unwrap();
        // Wait past the would-be touch time (sleep 1); if sh survived, it touches now.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        assert!(
            !marker.exists(),
            "release must kill sh before it touches the marker"
        );
        let e = reg
            .output(&out_req(&id))
            .expect_err("released id must be freed");
        assert!(
            e.message.contains("unknown terminal"),
            "id freed, got {e:?}"
        );
    }

    #[tokio::test]
    async fn kill_terminates_but_keeps_id() {
        // Fixture K: kill must terminate a running child but KEEP the id valid —
        // a later wait resolves with a signal status (fast, not a 30s natural wait)
        // and output still succeeds. A buggy kill==release would free the id ->
        // wait/output -> -32602.
        let reg = TerminalRegistry::new();
        let id = reg
            .create(&create_req("sleep").args(vec!["30".into()]))
            .unwrap()
            .terminal_id;
        reg.kill(&kill_req(&id)).await.unwrap();
        let resp = reg
            .wait(&wait_req(&id))
            .await
            .expect("killed id still waits");
        // The kill terminated `sleep` before its natural exit, so it must NOT report
        // a clean exit — this holds on every platform. The precise shape differs:
        // Unix reports the signal with no exit code; Windows (TerminateProcess)
        // reports a nonzero exit code with no signal. Assert the cross-platform
        // invariant unconditionally, and the SIGKILL signal only on Unix.
        assert_ne!(
            resp.exit_status.exit_code,
            Some(0),
            "killed => not a clean exit"
        );
        #[cfg(unix)]
        assert_eq!(resp.exit_status.signal.as_deref(), Some("9"), "SIGKILL=9");
        reg.output(&out_req(&id))
            .expect("killed id keeps a valid output");
    }

    #[tokio::test]
    async fn release_kill_unknown_id_errors() {
        // Fixture L: release/kill on a never-created id -> -32602, no panic.
        let reg = TerminalRegistry::new();
        let ghost = acp::TerminalId::new("term-999");
        let re = reg
            .release(&release_req(&ghost))
            .await
            .expect_err("unknown release errs");
        assert!(re.message.contains("unknown terminal"), "got {re:?}");
        let ke = reg
            .kill(&kill_req(&ghost))
            .await
            .expect_err("unknown kill errs");
        assert!(ke.message.contains("unknown terminal"), "got {ke:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn kill_during_pending_wait_terminates_child() {
        // Fixture (cyril-lw67, kill half): the acp rpc layer spawns every inbound
        // request as its own concurrent task, so `terminal/kill` arrives WHILE a
        // `wait_for_exit` owns the child — KAS's command-timeout pattern hits this
        // EVERY time (create → wait → timeout → kill). A kill whose InFlight case
        // falls through replies Ok having killed nothing: `sleep 30` keeps running
        // and the pending wait never resolves — the 5s timeout catches that hang.
        // The fix must actually terminate the child and let the pending wait
        // resolve with the killed status, keeping the id valid for `output`.
        let reg = TerminalRegistry::new();
        let id = reg
            .create(&create_req("sleep").args(vec!["30".into()]))
            .unwrap()
            .terminal_id;
        let wr = wait_req(&id);
        let wait_fut = reg.wait(&wr);
        let kill_fut = async {
            // Let the wait future take the child first — InFlight is only
            // reachable while the wait owns it.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            reg.kill(&kill_req(&id)).await
        };
        let (wait_res, kill_res) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::join!(wait_fut, kill_fut)
            })
            .await
            .expect("kill during a pending wait must terminate the child (not fall through and leave the wait hanging)");
        kill_res.expect("kill during a pending wait replies Ok");
        let resp = wait_res.expect("pending wait resolves after the kill");
        assert_ne!(
            resp.exit_status.exit_code,
            Some(0),
            "killed => not a clean exit"
        );
        #[cfg(unix)]
        assert_eq!(resp.exit_status.signal.as_deref(), Some("9"), "SIGKILL=9");
        // kill keeps the id valid: output still resolves with the cached status.
        reg.output(&out_req(&id))
            .expect("killed id keeps a valid output");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn release_during_pending_wait_frees_id() {
        // Fixture (cyril-lw67, release half): release arriving while a wait owns
        // the child must (1) still kill the child — the InFlight fall-through
        // killed nothing, so `sleep 30` stays alive and the pending wait hangs
        // (5s timeout catches it) — and (2) keep the id UNKNOWN (-32602) after
        // the pending wait completes: the old completion path unconditionally
        // re-inserted an Exited entry under the released id, resurrecting it and
        // leaking the entry + captured output for the life of the bridge.
        let reg = TerminalRegistry::new();
        let id = reg
            .create(&create_req("sleep").args(vec!["30".into()]))
            .unwrap()
            .terminal_id;
        let wr = wait_req(&id);
        let wait_fut = reg.wait(&wr);
        let release_fut = async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            reg.release(&release_req(&id)).await
        };
        let (wait_res, release_res) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::join!(wait_fut, release_fut)
            })
            .await
            .expect("release during a pending wait must kill the child (not fall through and leave the wait hanging)");
        release_res.expect("release during a pending wait replies Ok");
        let resp = wait_res.expect("pending wait resolves after the release kills the child");
        assert_ne!(
            resp.exit_status.exit_code,
            Some(0),
            "killed => not a clean exit"
        );
        // No resurrection: the released id stays unknown even after the pending
        // wait completed (the window where the old code re-inserted the entry).
        let oe = reg
            .output(&out_req(&id))
            .expect_err("released id must stay unknown after the pending wait completes");
        assert!(oe.message.contains("unknown terminal"), "got {oe:?}");
        let we = reg
            .wait(&wait_req(&id))
            .await
            .expect_err("released id must stay unknown to a later wait too");
        assert!(we.message.contains("unknown terminal"), "got {we:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn slow_wait_does_not_starve_runtime() {
        // Fixture P (C12 — the non-blocking invariant): on a SINGLE-THREADED runtime
        // (mirrors the bridge), a slow `wait` must not starve a concurrent fast one.
        // Start the clock BEFORE join! so a thread-pinning std::process wait shows up
        // as the fast terminal taking ~2s. A RefCell borrow held across .await would
        // instead panic BorrowMutError when the second wait re-borrows — also caught.
        let reg = TerminalRegistry::new();
        let slow = reg.create(&sh("sleep 2")).unwrap().terminal_id;
        let fast = reg.create(&create_req("true")).unwrap().terminal_id;
        let (slow_wr, fast_wr) = (wait_req(&slow), wait_req(&fast));
        let t0 = std::time::Instant::now();
        let fast_fut = async {
            reg.wait(&fast_wr).await.unwrap();
            t0.elapsed()
        };
        let slow_fut = reg.wait(&slow_wr);
        let (slow_res, fast_elapsed) = tokio::join!(slow_fut, fast_fut);
        slow_res.unwrap();
        assert!(
            fast_elapsed < std::time::Duration::from_millis(500),
            "fast terminal blocked by the slow one ({fast_elapsed:?}) — runtime starved"
        );
    }

    /// Read a still-`Running` terminal's OS pid straight out of the registry
    /// (tests live in the module, so `inner` is reachable). Must be called
    /// before any `wait` takes the child out.
    #[cfg(unix)]
    fn pid_of(reg: &TerminalRegistry, id: &acp::TerminalId) -> u32 {
        match reg.inner.borrow().get(id) {
            Some(Entry::Running {
                child: Some(child), ..
            }) => child.id().expect("running child has an OS pid"),
            _ => panic!("terminal is not in the Running(Some) state"),
        }
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn dropped_registry_kills_running_terminal() {
        // Fixture (cyril-ba5x, idle half): on Shutdown / bridge-thread death /
        // app exit, the LocalSet drops -> KiroClient drops -> registry drops. A
        // child spawned WITHOUT kill_on_drop survives that (tokio moves it to
        // the orphan queue but never signals it), so a live `sleep 60` outlives
        // cyril entirely. Dropping the registry while it holds the Child must
        // kill the process.
        let reg = TerminalRegistry::new();
        let id = reg
            .create(&create_req("sleep").args(vec!["60".into()]))
            .unwrap()
            .terminal_id;
        let pid = pid_of(&reg, &id);
        assert!(
            !dead_or_zombie(pid),
            "sleep 60 must be alive before the drop"
        );
        drop(reg);
        assert_process_dies(pid).await;
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn dropped_inflight_wait_kills_terminal() {
        // Fixture (cyril-ba5x, in-flight half): with KAS's create->wait pattern
        // the Child usually lives inside a pending wait future, not the
        // registry. When the bridge dies mid-command, the LocalSet drop cancels
        // that task — dropping the future and the Child it owns. That drop must
        // kill the process too, or the in-flight command leaks past exit.
        let reg = Rc::new(TerminalRegistry::new());
        let id = reg
            .create(&create_req("sleep").args(vec!["60".into()]))
            .unwrap()
            .terminal_id;
        let pid = pid_of(&reg, &id);
        let local = tokio::task::LocalSet::new();
        let task_reg = Rc::clone(&reg);
        let wr = wait_req(&id);
        let _wait_task = local.spawn_local(async move {
            // This task is cancelled mid-await by the LocalSet drop below; a
            // wait that RESOLVES here is itself a failure (sleep 60 must not
            // exit inside the test window).
            if task_reg.wait(&wr).await.is_ok() {
                panic!("sleep 60 must not exit cleanly during the drop test");
            }
        });
        // Drive the LocalSet long enough for the wait task to take the child.
        local
            .run_until(tokio::time::sleep(std::time::Duration::from_millis(100)))
            .await;
        match reg.inner.borrow().get(&id) {
            Some(Entry::Running { child, .. }) => assert!(
                child.is_none(),
                "the spawned wait must have taken the child (in-flight state)"
            ),
            _ => panic!("terminal must still be Running while the wait is in flight"),
        }
        assert!(
            !dead_or_zombie(pid),
            "sleep 60 must be alive before the drop"
        );
        drop(local); // cancels the pending wait -> drops the future -> drops the Child
        drop(reg);
        assert_process_dies(pid).await;
    }

    /// A `sleep 60` create request under an arbitrary session (the reap fences
    /// need two distinct sessions; the other helpers hardcode `"s"`).
    #[cfg(unix)]
    fn sleep_req(session: &str) -> acp::CreateTerminalRequest {
        acp::CreateTerminalRequest::new(acp::SessionId::new(session), "sleep")
            .args(vec!["60".to_string()])
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn reap_session_kills_only_that_sessions_children_and_keeps_ids() {
        // Fixture (cyril-3lh8, idle half): reap_session must kill session A's
        // still-registered child — alive-asserted FIRST so the fence can't pass
        // as a silent no-op — while SPARING session B's (the linear scan filters
        // by the entry's session_id). KILL semantics, not release: the reaped id
        // stays valid — a later wait resolves with the signal status and output
        // still succeeds instead of erroring -32602 (KAS sends those late).
        let reg = TerminalRegistry::new();
        let a = reg.create(&sleep_req("sess-a")).unwrap().terminal_id;
        let b = reg.create(&sleep_req("sess-b")).unwrap().terminal_id;
        let a_pid = pid_of(&reg, &a);
        let b_pid = pid_of(&reg, &b);
        assert!(!dead_or_zombie(a_pid), "A must be alive before the reap");
        assert!(!dead_or_zombie(b_pid), "B must be alive before the reap");
        reg.reap_session(&acp::SessionId::new("sess-a")).await;
        assert_process_dies(a_pid).await;
        assert!(
            !dead_or_zombie(b_pid),
            "reap must not touch another session's child"
        );
        let resp = reg
            .wait(&wait_req(&a))
            .await
            .expect("reaped id still waits (kill semantics, not release)");
        assert_ne!(
            resp.exit_status.exit_code,
            Some(0),
            "reaped => not a clean exit"
        );
        assert_eq!(resp.exit_status.signal.as_deref(), Some("9"), "SIGKILL=9");
        reg.output(&out_req(&a))
            .expect("reaped id keeps a valid output");
        // Reap B so the test leaves no orphan.
        reg.release(&release_req(&b)).await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn reap_session_kills_child_owned_by_pending_wait() {
        // Fixture (cyril-3lh8, in-flight half): with KAS's create->wait-
        // immediately pattern the child is usually owned by a pending `wait`,
        // not the registry. reap_session can't start_kill a child it doesn't
        // hold; it must terminate it through the owner via the kill signal
        // (the cyril-lw67 seam) so the pending wait resolves with the killed
        // status instead of hanging out the full sleep (5s timeout catches it).
        let reg = TerminalRegistry::new();
        let id = reg.create(&sleep_req("sess-a")).unwrap().terminal_id;
        let pid = pid_of(&reg, &id);
        assert!(!dead_or_zombie(pid), "child must be alive before the reap");
        let wr = wait_req(&id);
        let wait_fut = reg.wait(&wr);
        let reap_fut = async {
            // Let the wait future take the child first — the in-flight state is
            // only reachable while the wait owns it.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            reg.reap_session(&acp::SessionId::new("sess-a")).await;
        };
        let (wait_res, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(wait_fut, reap_fut)
        })
        .await
        .expect("reap must terminate the in-flight child (pending wait must not hang)");
        let resp = wait_res.expect("pending wait resolves after the reap");
        assert_ne!(
            resp.exit_status.exit_code,
            Some(0),
            "reaped => not a clean exit"
        );
        assert_eq!(resp.exit_status.signal.as_deref(), Some("9"), "SIGKILL=9");
        assert_process_dies(pid).await;
    }
}
