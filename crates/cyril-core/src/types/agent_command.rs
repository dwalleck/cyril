//! Non-empty command line for spawning an ACP agent subprocess.

use crate::error::{Error, ErrorKind};

/// A non-empty argv for spawning an ACP agent.
///
/// Construct via [`AgentCommand::try_from_argv`] (returns `Err` on empty
/// input) or [`AgentCommand::new`] + [`AgentCommand::with_args`] (always
/// produces a non-empty value). The empty case is unrepresentable, so
/// downstream callers can rely on `program()` returning a real binary
/// name without runtime checks of their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCommand {
    program: String,
    args: Vec<String>,
}

impl AgentCommand {
    /// Build an `AgentCommand` with no arguments.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    /// Replace the args list. Builder-style.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Construct from an argv vector. Returns `Err` if empty.
    ///
    /// This is the right entry point for CLI parsing, where clap may
    /// theoretically deliver a zero-element vec depending on configuration.
    pub fn try_from_argv(argv: Vec<String>) -> crate::Result<Self> {
        let mut iter = argv.into_iter();
        let program = iter.next().ok_or_else(|| {
            Error::from_kind(ErrorKind::Transport {
                detail: "agent command is empty (need at least the program name)".into(),
            })
        })?;
        Ok(Self {
            program,
            args: iter.collect(),
        })
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn try_from_argv_empty_returns_transport_error() {
        let err = AgentCommand::try_from_argv(Vec::new()).unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::Transport { .. }));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn try_from_argv_single_program_no_args() {
        let cmd = AgentCommand::try_from_argv(vec!["kiro-cli".to_string()]).unwrap();
        assert_eq!(cmd.program(), "kiro-cli");
        assert!(cmd.args().is_empty());
    }

    #[test]
    fn try_from_argv_program_with_args() {
        let cmd =
            AgentCommand::try_from_argv(vec!["kiro-cli".to_string(), "acp".to_string()]).unwrap();
        assert_eq!(cmd.program(), "kiro-cli");
        assert_eq!(cmd.args(), &["acp".to_string()]);
    }

    #[test]
    fn new_with_args_builder() {
        let cmd = AgentCommand::new("kiro-cli").with_args(vec!["acp".to_string()]);
        assert_eq!(cmd.program(), "kiro-cli");
        assert_eq!(cmd.args(), &["acp".to_string()]);
    }
}
