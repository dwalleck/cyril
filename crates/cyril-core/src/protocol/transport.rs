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
    pub async fn spawn(agent_name: &str, cwd: &Path) -> crate::Result<Self> {
        let (program, args) = if cfg!(target_os = "windows") {
            (
                "wsl".to_string(),
                vec![agent_name.to_string(), "acp".to_string()],
            )
        } else {
            (agent_name.to_string(), vec!["acp".to_string()])
        };

        let mut child = Command::new(&program)
            .args(&args)
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
