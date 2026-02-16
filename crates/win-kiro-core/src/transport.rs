use anyhow::{Context, Result, bail};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub type CompatStdin = tokio_util::compat::Compat<tokio::process::ChildStdin>;
pub type CompatStdout = tokio_util::compat::Compat<tokio::process::ChildStdout>;

/// Wraps the WSL agent subprocess and its compat-wrapped pipes.
pub struct AgentProcess {
    _child: Child,
    stdin: Option<CompatStdin>,
    stdout: Option<CompatStdout>,
    stderr_rx: mpsc::UnboundedReceiver<String>,
}

impl AgentProcess {
    /// Spawn `wsl kiro-cli acp` and return compat-wrapped stdin/stdout
    /// suitable for passing to `ClientSideConnection::new`.
    pub fn spawn() -> Result<Self> {
        let mut child = Command::new("wsl")
            .args(["kiro-cli", "acp"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn `wsl kiro-cli acp`. Is WSL installed and kiro-cli available?")?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to capture agent stdin")?
            .compat_write();

        let stdout = child
            .stdout
            .take()
            .context("Failed to capture agent stdout")?
            .compat();

        // Capture stderr in background so we can surface auth/startup errors
        let stderr = child
            .stderr
            .take()
            .context("Failed to capture agent stderr")?;

        let (stderr_tx, stderr_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut stderr = stderr;
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(s) = std::string::String::from_utf8(buf[..n].to_vec()) {
                            let _ = stderr_tx.send(s);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            _child: child,
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr_rx,
        })
    }

    /// Take the stdin pipe (can only be called once).
    pub fn take_stdin(&mut self) -> Result<CompatStdin> {
        self.stdin.take().context("stdin already taken")
    }

    /// Take the stdout pipe (can only be called once).
    pub fn take_stdout(&mut self) -> Result<CompatStdout> {
        self.stdout.take().context("stdout already taken")
    }

    /// Drain any stderr output collected so far.
    pub fn drain_stderr(&mut self) -> String {
        let mut output = String::new();
        while let Ok(chunk) = self.stderr_rx.try_recv() {
            output.push_str(&chunk);
        }
        output
    }

    /// Check if the process has already exited (non-blocking).
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
        self._child.try_wait().context("Failed to check agent process status")
    }

    /// Wait briefly for the process to start, returning an error if it exits
    /// immediately (e.g. due to auth failure).
    pub async fn check_startup(&mut self) -> Result<()> {
        // Give the process a moment to fail
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        if let Some(status) = self.try_wait()? {
            let stderr = self.drain_stderr();
            if stderr.contains("not logged in") || stderr.contains("please log in") {
                bail!(
                    "kiro-cli requires authentication.\n\
                     Run `wsl kiro-cli login` first, then try again.\n\n\
                     Agent stderr: {stderr}"
                );
            }
            bail!(
                "Agent process exited immediately with {status}.\n\
                 Agent stderr: {stderr}"
            );
        }
        Ok(())
    }
}
