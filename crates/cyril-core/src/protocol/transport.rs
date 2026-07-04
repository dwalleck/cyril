use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use crate::types::AgentCommand;

/// How many trailing stderr lines [`StderrTail`] retains for diagnostics.
const STDERR_TAIL_CAPACITY: usize = 50;

/// Cloneable handle to the last [`STDERR_TAIL_CAPACITY`] stderr lines drained
/// from the agent subprocess (cyril-0gke).
///
/// The drain task pushes as lines arrive; the disconnect path snapshots when
/// the agent dies so its final words (crash traceback, refusal reason) aren't
/// lost. `Arc<Mutex>` because the drain runs on a plain `tokio::spawn` (pipe
/// reads are `Send`) while readers live on the bridge's LocalSet thread.
#[derive(Clone, Default)]
pub(crate) struct StderrTail {
    lines: Arc<Mutex<VecDeque<String>>>,
}

impl StderrTail {
    /// Append a line, evicting the oldest once the buffer is full.
    fn push(&self, line: String) {
        let mut lines = self.lock();
        if lines.len() == STDERR_TAIL_CAPACITY {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// The retained tail, oldest line first.
    pub(crate) fn snapshot(&self) -> Vec<String> {
        self.lock().iter().cloned().collect()
    }

    /// Lock the buffer, recovering from poisoning — a panicked drain task must
    /// not also cost us the diagnostic tail it had already captured.
    fn lock(&self) -> MutexGuard<'_, VecDeque<String>> {
        match self.lines.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("stderr tail mutex poisoned; recovering buffered lines");
                poisoned.into_inner()
            }
        }
    }
}

/// Drain the child's stderr for its whole life so a chatty agent can never
/// block on a full pipe (Linux pipe buffer is ~64KB; KAS's node runtime and
/// tracebacks write to stderr freely). Each line is debug-logged and kept in
/// `tail` for the disconnect path.
fn spawn_stderr_drain(stderr: ChildStderr, tail: StderrTail) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            // Byte-level split (not `lines()`): a non-UTF-8 byte in a traceback
            // must not abort the drain, or the pipe-full wedge comes back.
            match reader.read_until(b'\n', &mut buf).await {
                // EOF — the child closed stderr (normally: it exited).
                Ok(0) => break,
                Ok(_) => {
                    while matches!(buf.last(), Some(b'\n' | b'\r')) {
                        buf.pop();
                    }
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    tracing::debug!(line = %line, "agent stderr");
                    tail.push(line);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "agent stderr drain failed; stopping");
                    break;
                }
            }
        }
    });
}

pub(crate) struct AgentProcess {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    /// Last stderr lines drained from the child; see [`StderrTail`].
    stderr_tail: StderrTail,
    /// Held to keep the child process alive; dropped when the bridge shuts down.
    pub _child: Child,
}

impl AgentProcess {
    /// Spawn an ACP agent subprocess described by `cmd`.
    pub async fn spawn(cmd: &AgentCommand, cwd: &Path) -> crate::Result<Self> {
        let program = cmd.program();
        let args = cmd.args();

        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::Error::with_source(
                    crate::ErrorKind::Transport {
                        detail: format!("failed to spawn {program}"),
                    },
                    e,
                )
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "failed to capture stdin".into(),
            })
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "failed to capture stdout".into(),
            })
        })?;

        let stderr = child.stderr.take().ok_or_else(|| {
            crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "failed to capture stderr".into(),
            })
        })?;

        let stderr_tail = StderrTail::default();
        spawn_stderr_drain(stderr, stderr_tail.clone());

        Ok(Self {
            stdin,
            stdout,
            stderr_tail,
            _child: child,
        })
    }

    /// Handle to the child's last stderr lines, for including in disconnect
    /// diagnostics. Cloneable, so it stays usable after `stdin`/`stdout` are
    /// moved into the ACP connection (full UI surfacing is cyril-l7tw).
    pub(crate) fn stderr_tail(&self) -> StderrTail {
        self.stderr_tail.clone()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::Duration;

    use super::*;

    /// Regression fence for the stderr-pipe wedge (cyril-0gke bug class): a
    /// child that writes far more than the 64KB Linux pipe buffer to stderr
    /// must still be able to finish. Without a drain task, its stderr writes
    /// block once the pipe fills and the process never exits.
    #[cfg(unix)]
    #[tokio::test]
    async fn chatty_stderr_does_not_wedge_agent_process() {
        let dir = tempfile::tempdir().expect("tempdir");
        // ~4000 lines x ~45 bytes = ~180KB of stderr, well past the 64KB pipe
        // buffer, then a normal stdout hand-off and clean exit.
        let script = r#"i=0; while [ $i -lt 4000 ]; do echo "stderr spam line $i padding-padding-padding" 1>&2; i=$((i+1)); done; echo done"#;
        let cmd = AgentCommand::new("sh").with_args(vec!["-c".to_string(), script.to_string()]);

        let mut process = AgentProcess::spawn(&cmd, dir.path())
            .await
            .expect("spawn sh");

        let status = tokio::time::timeout(Duration::from_secs(5), process._child.wait())
            .await
            .expect("agent process wedged: stderr pipe filled and nobody drained it")
            .expect("wait on child failed");
        assert!(status.success(), "child exited with failure: {status:?}");
    }

    /// Last-N semantics of the ring buffer: pushing past capacity evicts the
    /// oldest lines and preserves arrival order.
    #[test]
    fn stderr_tail_keeps_only_last_n_lines() {
        let tail = StderrTail::default();
        assert!(tail.snapshot().is_empty());

        for i in 0..(STDERR_TAIL_CAPACITY + 5) {
            tail.push(format!("line {i}"));
        }

        let snapshot = tail.snapshot();
        assert_eq!(snapshot.len(), STDERR_TAIL_CAPACITY);
        assert_eq!(snapshot.first().map(String::as_str), Some("line 5"));
        assert_eq!(snapshot.last().map(String::as_str), Some("line 54"));
    }

    /// The accessor hands the disconnect path a live view: after the child
    /// exits, the drained tail holds the final `STDERR_TAIL_CAPACITY` lines.
    #[cfg(unix)]
    #[tokio::test]
    async fn stderr_tail_snapshot_holds_final_lines_after_exit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = r#"i=0; while [ $i -lt 60 ]; do echo "line $i" 1>&2; i=$((i+1)); done"#;
        let cmd = AgentCommand::new("sh").with_args(vec!["-c".to_string(), script.to_string()]);

        let mut process = AgentProcess::spawn(&cmd, dir.path())
            .await
            .expect("spawn sh");
        let tail = process.stderr_tail();

        tokio::time::timeout(Duration::from_secs(5), process._child.wait())
            .await
            .expect("child did not exit")
            .expect("wait on child failed");

        // The drain task races the child's exit; poll until it catches up.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = tail.snapshot();
            if snapshot.last().is_some_and(|line| line == "line 59") {
                assert_eq!(snapshot.len(), STDERR_TAIL_CAPACITY);
                assert_eq!(snapshot.first().map(String::as_str), Some("line 10"));
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "stderr tail never caught up: {snapshot:?}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
