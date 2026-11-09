//! Display external MCP notices literally while neutralizing terminal controls.

use std::fmt;

use cyril_core::types::Notification;

/// Re-exported so existing MCP callers and `test_bridge` keep their path.
pub use crate::terminal_text::terminal_safe;
pub(crate) use crate::terminal_text::{characters, needs_escape};

/// One borrowed v2 progress notice, usable in main, candidate, and peer history
/// without allocating an intermediate message or changing its session scope.
pub(crate) enum V2Progress<'a> {
    Ready(&'a str),
    Failed {
        server: &'a str,
        error: Option<&'a str>,
    },
    OAuth {
        server: &'a str,
        url: &'a str,
    },
}

pub(crate) fn v2_progress(notification: &Notification) -> Option<V2Progress<'_>> {
    match notification {
        Notification::McpServerInitialized { server_name, .. } => {
            Some(V2Progress::Ready(server_name))
        }
        Notification::McpServerInitFailure {
            server_name, error, ..
        } => Some(V2Progress::Failed {
            server: server_name,
            error: error.as_deref(),
        }),
        Notification::McpOAuthRequest {
            server_name, url, ..
        } => Some(V2Progress::OAuth {
            server: server_name,
            url,
        }),
        _ => None,
    }
}

impl fmt::Display for V2Progress<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(server) => {
                write!(formatter, "MCP server '{}' ready", terminal_safe(server))
            }
            Self::Failed {
                server,
                error: Some(error),
            } => write!(
                formatter,
                "MCP server '{}' failed to initialize: {}",
                terminal_safe(server),
                terminal_safe(error),
            ),
            Self::Failed {
                server,
                error: None,
            } => write!(
                formatter,
                "MCP server '{}' failed to initialize",
                terminal_safe(server),
            ),
            Self::OAuth { server, url } => write!(
                formatter,
                "MCP server '{}' requires authentication. Open in browser: {}",
                terminal_safe(server),
                terminal_safe(url),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn v2_progress_preserves_printable_urls_and_neutralizes_controls() {
        use cyril_core::types::Notification;
        let notice = Notification::McpOAuthRequest {
            session_id: None,
            server_name: "auth\u{1b}[31m".into(),
            url: "https://example.test/a'b\"c\\日?state=exact#finish\n\u{202e}".into(),
        };
        assert_eq!(
            super::v2_progress(&notice)
                .unwrap_or_else(|| panic!("OAuth notice"))
                .to_string(),
            "MCP server 'auth\\u{1b}[31m' requires authentication. Open in browser: https://example.test/a'b\"c\\日?state=exact#finish\\n\\u{202e}",
        );
        assert!(super::v2_progress(&Notification::TurnStarted).is_none());
    }
}
