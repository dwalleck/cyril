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
//! borrow) and `wait_with_output().await` drains both pipes *while* waiting — no
//! pipe-buffer deadlock. The undrained-pipe window before `wait` is sub-ms (KAS
//! calls `wait` immediately after `create`); the chatty-command risk if KAS ever
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

use agent_client_protocol as acp;
use tokio::process::Child;

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
    Running(Option<Child>),
    Exited {
        output: String,
        status: acp::TerminalExitStatus,
    },
}

/// Outcome of taking a terminal's child out of the registry for an awaiting op.
enum Taken {
    /// The live child, removed from its `Running` slot — caller awaits + reaps it.
    Child(Child),
    /// The terminal already exited; carries its cached status (for `wait`).
    AlreadyExited(acp::TerminalExitStatus),
    /// Another op already took the child and is awaiting it (defensive — KAS is
    /// sequential, so this is not reached in practice).
    InFlight,
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
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
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
        self.inner
            .borrow_mut()
            .insert(id.clone(), Entry::Running(Some(child)));
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
        let child = match self.take_child(&req.terminal_id)? {
            Taken::Child(child) => child,
            Taken::AlreadyExited(status) => {
                return Ok(acp::WaitForTerminalExitResponse::new(status));
            }
            Taken::InFlight => {
                return Err(acp::Error::new(
                    -32603,
                    format!("terminal {} wait already in progress", req.terminal_id),
                ));
            }
        };
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| wait_err(&req.terminal_id, e))?;
        let status = exit_status(&out.status);
        self.inner.borrow_mut().insert(
            req.terminal_id.clone(),
            Entry::Exited {
                output: combine_output(&out),
                status: status.clone(),
            },
        );
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
            Some(Entry::Running(_)) => Ok(acp::TerminalOutputResponse::new(String::new(), false)),
        }
    }

    /// Answer `terminal/release`: kill a still-running child, reap it (await the
    /// exit so no zombie/orphan is left), and **free the id** — subsequent ops on
    /// it become unknown-id `-32602`. Unknown id → `-32602`. Awaits `tokio::process`.
    pub(crate) async fn release(
        &self,
        req: &acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        if let Taken::Child(mut child) = self.take_child(&req.terminal_id)? {
            // SIGKILL then reap. Without the wait the child is a zombie; tokio's
            // Child does NOT kill/reap on drop. Output is discarded (id is freed).
            let _ = child.start_kill();
            let _ = child.wait().await;
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
        if let Taken::Child(mut child) = self.take_child(&req.terminal_id)? {
            let _ = child.start_kill();
            let out = child
                .wait_with_output()
                .await
                .map_err(|e| wait_err(&req.terminal_id, e))?;
            self.inner.borrow_mut().insert(
                req.terminal_id.clone(),
                Entry::Exited {
                    output: combine_output(&out),
                    status: exit_status(&out.status),
                },
            );
        }
        Ok(acp::KillTerminalResponse::new())
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
            Some(Entry::Running(slot)) => Ok(slot.take().map_or(Taken::InFlight, Taken::Child)),
        }
    }
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

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
        assert_eq!(resp.exit_status.exit_code, None, "killed => no exit code");
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
}
