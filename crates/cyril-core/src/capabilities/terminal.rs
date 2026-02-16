use std::collections::HashMap;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Identifies a managed terminal.
pub type TerminalId = String;

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
        let id = format!("term-{}", self.next_id);
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
    pub fn get_output(&mut self, id: &str) -> Result<String> {
        let term = self
            .terminals
            .get_mut(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;

        // Drain all available output
        while let Ok(chunk) = term.output_rx.try_recv() {
            term.accumulated_output.push_str(&chunk);
        }

        let output = term.accumulated_output.clone();
        term.accumulated_output.clear();
        Ok(output)
    }

    /// Wait for a terminal to exit and return its exit code.
    pub async fn wait_for_exit(&mut self, id: &str) -> Result<i32> {
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
    pub fn release(&mut self, id: &str) -> Result<()> {
        self.terminals
            .remove(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;
        Ok(())
    }

    /// Kill a terminal process.
    pub async fn kill(&mut self, id: &str) -> Result<()> {
        let term = self
            .terminals
            .get_mut(id)
            .with_context(|| format!("Unknown terminal: {id}"))?;

        term.child.kill().await.context("Failed to kill terminal")?;
        Ok(())
    }
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
                if let Ok(s) = String::from_utf8(buf[..n].to_vec()) {
                    let _ = tx.send(s);
                }
            }
            Err(_) => break,
        }
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
