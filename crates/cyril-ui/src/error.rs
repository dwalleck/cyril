use std::fmt;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("terminal error: {detail}")]
    Terminal { detail: String },

    #[error("render failed: {detail}")]
    Render { detail: String },
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
    use super::*;

    #[test]
    fn terminal_error_displays() {
        let err = Error::from_kind(ErrorKind::Terminal {
            detail: "raw mode failed".into(),
        });
        assert_eq!(err.to_string(), "terminal error: raw mode failed");
    }

    #[test]
    fn render_error_displays() {
        let err = Error::from_kind(ErrorKind::Render {
            detail: "layout overflow".into(),
        });
        assert_eq!(err.to_string(), "render failed: layout overflow");
    }

    #[test]
    fn error_kind_accessible() {
        let err = Error::from_kind(ErrorKind::Terminal {
            detail: "test".into(),
        });
        assert!(matches!(err.kind(), ErrorKind::Terminal { .. }));
    }

    #[test]
    fn error_with_source_chains() {
        let source = std::io::Error::other("io failed");
        let err = Error::with_source(
            ErrorKind::Terminal {
                detail: "setup".into(),
            },
            source,
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn result_alias_works() {
        fn ok_fn() -> Result<i32> {
            Ok(42)
        }
        fn err_fn() -> Result<i32> {
            Err(Error::from_kind(ErrorKind::Render {
                detail: "x".into(),
            }))
        }
        assert_eq!(ok_fn().ok(), Some(42));
        assert!(err_fn().is_err());
    }
}
