use std::fmt;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("protocol error: {message}")]
    Protocol { message: String },

    #[error("agent process failed: {detail}")]
    Transport { detail: String },

    #[error("agent process exited unexpectedly (code {exit_code:?})")]
    AgentExited {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("no active session")]
    NoSession,

    #[error("session {id} not found")]
    SessionNotFound { id: String },

    #[error("unknown command: {name}")]
    UnknownCommand { name: String },

    #[error("command failed: {detail}")]
    CommandFailed { detail: String },

    #[error("bridge channel closed")]
    BridgeClosed,

    #[error("permission request timed out")]
    PermissionTimeout,

    #[error("invalid configuration: {detail}")]
    InvalidConfig { detail: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

impl Error {
    pub fn from_kind(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub fn with_source(
        kind: ErrorKind,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::*;

    #[test]
    fn error_displays_kind_message() {
        let err = Error::from_kind(ErrorKind::NoSession);
        assert_eq!(err.to_string(), "no active session");
    }

    #[test]
    fn protocol_error_displays_message() {
        let err = Error::from_kind(ErrorKind::Protocol {
            message: "timeout".into(),
        });
        assert_eq!(err.to_string(), "protocol error: timeout");
    }

    #[test]
    fn error_kind_accessible() {
        let err = Error::from_kind(ErrorKind::Protocol {
            message: "timeout".into(),
        });
        assert!(matches!(
            err.kind(),
            ErrorKind::Protocol { message } if message == "timeout"
        ));
    }

    #[test]
    fn error_with_source_chains() {
        let source = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = Error::with_source(
            ErrorKind::Transport {
                detail: "connect failed".into(),
            },
            source,
        );
        assert!(StdError::source(&err).is_some());
        assert_eq!(err.to_string(), "agent process failed: connect failed");
    }

    #[test]
    fn all_error_kinds_display() {
        let cases: Vec<(ErrorKind, &str)> = vec![
            (ErrorKind::NoSession, "no active session"),
            (
                ErrorKind::SessionNotFound { id: "abc".into() },
                "session abc not found",
            ),
            (
                ErrorKind::UnknownCommand {
                    name: "/foo".into(),
                },
                "unknown command: /foo",
            ),
            (
                ErrorKind::CommandFailed {
                    detail: "oops".into(),
                },
                "command failed: oops",
            ),
            (ErrorKind::BridgeClosed, "bridge channel closed"),
            (ErrorKind::PermissionTimeout, "permission request timed out"),
            (
                ErrorKind::InvalidConfig {
                    detail: "bad toml".into(),
                },
                "invalid configuration: bad toml",
            ),
            (
                ErrorKind::AgentExited {
                    exit_code: Some(1),
                    stderr: "error".into(),
                },
                "agent process exited unexpectedly (code Some(1))",
            ),
        ];
        for (kind, expected) in cases {
            let err = Error::from_kind(kind);
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn result_alias_works() {
        fn test_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(test_fn().ok(), Some(42));
    }

    #[test]
    fn result_alias_error() {
        fn test_fn() -> Result<i32> {
            Err(Error::from_kind(ErrorKind::NoSession))
        }
        assert!(test_fn().is_err());
    }
}
