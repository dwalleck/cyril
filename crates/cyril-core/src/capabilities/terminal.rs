use std::collections::HashMap;
use std::fmt;
use std::process::Stdio;
use std::str::FromStr;

use anyhow::{Context, Result};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Identifies a managed terminal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalId(String);

impl TerminalId {
    fn new(id: u64) -> Self {
        Self(format!("term-{id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for TerminalId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// A running terminal process on Windows.
#[derive(Debug)]
struct TerminalProcess {
    child: Child,
    output_rx: mpsc::UnboundedReceiver<String>,
    accumulated_output: String,
}

/// Which shell to use for terminal execution.
#[derive(Debug, Clone)]
pub enum Shell {
    Pwsh,
    PowerShell,
    Cmd,
}

impl Shell {
    fn program(&self) -> &str {
        match self {
            Shell::Pwsh => "pwsh",
            Shell::PowerShell => "powershell",
            Shell::Cmd => "cmd.exe",
        }
    }

    fn command_flag(&self) -> &str {
        match self {
            Shell::Pwsh | Shell::PowerShell => "-Command",
            Shell::Cmd => "/C",
        }
    }
}

/// Maximum accumulated output buffer size (1MB). Older output is truncated.
const MAX_ACCUMULATED_OUTPUT: usize = 1024 * 1024;

/// Manages terminal processes spawned on Windows.
#[derive(Debug)]
pub struct TerminalManager {
    shell: Shell,
    terminals: HashMap<TerminalId, TerminalProcess>,
    next_id: u64,
}

impl TerminalManager {
    /// Create a new TerminalManager, auto-detecting the best available shell.
    pub fn new() -> Self {
        let shell = detect_shell();
        tracing::info!("Using shell: {:?}", shell);
        Self {
            shell,
            terminals: HashMap::new(),
            next_id: 0,
        }
    }

    /// Create a terminal and start running a command.
    pub fn create_terminal(&mut self, command: &str) -> Result<TerminalId> {
        let id = TerminalId::new(self.next_id);
        self.next_id += 1;

        let mut child = Command::new(self.shell.program())
            .arg(self.shell.command_flag())
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("Failed to spawn terminal command: {command}"))?;

        let (output_tx, output_rx) = mpsc::unbounded_channel();

        // Spawn tasks to read stdout and stderr into the channel.
        // These use tokio::spawn (Send) because they only do I/O, no ACP types.
        if let Some(stdout) = child.stdout.take() {
            let tx = output_tx.clone();
            tokio::spawn(async move {
                read_stream_to_channel(stdout, tx).await;
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let tx = output_tx;
            tokio::spawn(async move {
                read_stream_to_channel(stderr, tx).await;
            });
        }

        self.terminals.insert(
            id.clone(),
            TerminalProcess {
                child,
                output_rx,
                accumulated_output: String::new(),
            },
        );

        Ok(id)
    }

    /// Get new output from a terminal since the last call.
    pub fn get_output(&mut self, id: &TerminalId) -> Result<String> {
        let term = self
            .terminals
            .get_mut(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;

        // Drain all available output
        while let Ok(chunk) = term.output_rx.try_recv() {
            term.accumulated_output.push_str(&chunk);
        }

        cap_output(&mut term.accumulated_output, MAX_ACCUMULATED_OUTPUT);

        let output = term.accumulated_output.clone();
        term.accumulated_output.clear();
        Ok(output)
    }

    /// Wait for a terminal to exit and return its exit code.
    pub async fn wait_for_exit(&mut self, id: &TerminalId) -> Result<i32> {
        let term = self
            .terminals
            .get_mut(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;

        let status = term
            .child
            .wait()
            .await
            .context("Failed to wait for terminal")?;

        Ok(status.code().unwrap_or(-1))
    }

    /// Release (remove) a terminal.
    pub fn release(&mut self, id: &TerminalId) -> Result<()> {
        self.terminals
            .remove(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;
        Ok(())
    }

    /// Kill a terminal process.
    pub async fn kill(&mut self, id: &TerminalId) -> Result<()> {
        let term = self
            .terminals
            .get_mut(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;

        term.child.kill().await.context("Failed to kill terminal")?;
        Ok(())
    }
}

/// Cap a buffer to `max_len` bytes, keeping the most recent content.
/// Prepends a truncation marker when content is trimmed.
fn cap_output(buf: &mut String, max_len: usize) {
    const PREFIX: &str = "[output truncated]\n";
    if buf.len() <= max_len {
        return;
    }
    let max_tail = max_len.saturating_sub(PREFIX.len());
    let start = buf.len() - max_tail;
    let boundary = buf.ceil_char_boundary(start);
    let truncated = format!("{PREFIX}{}", &buf[boundary..]);
    *buf = truncated;
}

async fn read_stream_to_channel(
    mut stream: impl tokio::io::AsyncRead + Unpin,
    tx: mpsc::UnboundedSender<String>,
) {
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                if tx.send(s).is_err() {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!("Terminal stream read error: {e}");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_id_display_format() {
        let id = TerminalId::new(42);
        assert_eq!(id.as_str(), "term-42");
        assert_eq!(id.to_string(), "term-42");
    }

    #[test]
    fn terminal_id_from_str_roundtrip() {
        let id: TerminalId = "term-7".parse().expect("infallible");
        assert_eq!(id.as_str(), "term-7");
    }

    #[tokio::test]
    async fn create_terminal_and_get_output() {
        let mut manager = TerminalManager {
            shell: Shell::Cmd,
            terminals: HashMap::new(),
            next_id: 0,
        };

        // Use a simple cross-platform command
        let cmd = if cfg!(target_os = "windows") {
            "echo hello"
        } else {
            // On Linux, cmd.exe doesn't exist, so skip the actual spawn test
            return;
        };

        let id = manager.create_terminal(cmd).expect("failed to create terminal");
        assert_eq!(id.as_str(), "term-0");

        // Wait for process to finish producing output
        let _exit = manager.wait_for_exit(&id).await.expect("wait failed");

        let output = manager.get_output(&id).expect("get_output failed");
        assert!(output.contains("hello"), "expected 'hello' in output: {output}");

        manager.release(&id).expect("release failed");
    }

    #[test]
    fn release_unknown_terminal_returns_error() {
        let mut manager = TerminalManager {
            shell: Shell::Cmd,
            terminals: HashMap::new(),
            next_id: 0,
        };

        let unknown = TerminalId::new(999);
        let result = manager.release(&unknown);
        assert!(result.is_err());
    }

    #[test]
    fn get_output_unknown_terminal_returns_error() {
        let mut manager = TerminalManager {
            shell: Shell::Cmd,
            terminals: HashMap::new(),
            next_id: 0,
        };

        let unknown = TerminalId::new(999);
        let result = manager.get_output(&unknown);
        assert!(result.is_err());
    }

    #[test]
    fn cap_output_no_op_when_under_limit() {
        let mut buf = "hello".to_string();
        cap_output(&mut buf, 100);
        assert_eq!(buf, "hello");
    }

    #[test]
    fn cap_output_truncates_and_adds_prefix() {
        let mut buf = "a".repeat(200);
        cap_output(&mut buf, 100);
        assert!(buf.starts_with("[output truncated]\n"));
        assert!(buf.len() <= 100);
    }

    #[test]
    fn cap_output_respects_multibyte_char_boundaries() {
        // Each emoji is 4 bytes; fill buffer to force truncation mid-character
        let mut buf = "\u{1F600}".repeat(100); // 400 bytes of emoji
        cap_output(&mut buf, 50);
        assert!(buf.starts_with("[output truncated]\n"));
        // Result must be valid UTF-8 (no panic on the assertion itself proves this)
        assert!(buf.len() <= 50 + 4); // allow up to one extra char from boundary rounding
    }

    #[test]
    fn cap_output_keeps_most_recent_content() {
        let mut buf = format!("{}{}", "x".repeat(500), "TAIL");
        cap_output(&mut buf, 100);
        assert!(buf.ends_with("TAIL"));
    }
}

/// Auto-detect the best available shell on Windows.
fn detect_shell() -> Shell {
    // Try pwsh (PowerShell 7+) first
    if std::process::Command::new("pwsh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        return Shell::Pwsh;
    }

    // Fall back to powershell (5.1, always present on Windows 10+)
    if std::process::Command::new("powershell")
        .arg("-Command")
        .arg("$PSVersionTable")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        return Shell::PowerShell;
    }

    // Last resort
    Shell::Cmd
}
