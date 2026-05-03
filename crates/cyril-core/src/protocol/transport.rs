use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::types::AgentCommand;

pub(crate) struct AgentProcess {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
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

        Ok(Self {
            stdin,
            stdout,
            _child: child,
        })
    }
}
