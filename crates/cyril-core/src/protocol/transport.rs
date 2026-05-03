use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub(crate) struct AgentProcess {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    /// Held to keep the child process alive; dropped when the bridge shuts down.
    pub _child: Child,
}

impl AgentProcess {
    /// Spawn an ACP agent subprocess. `agent_command[0]` is the program;
    /// `agent_command[1..]` are arguments. Returns an error if the slice is empty.
    pub async fn spawn(agent_command: &[String], cwd: &Path) -> crate::Result<Self> {
        let (program, args) = agent_command.split_first().ok_or_else(|| {
            crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "agent command is empty (need at least the program name)".into(),
            })
        })?;

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
