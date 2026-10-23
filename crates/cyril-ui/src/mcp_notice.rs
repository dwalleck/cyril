//! Display external MCP notices literally while neutralizing terminal controls.

use std::fmt::{self, Write};

use cyril_core::types::Notification;

pub(crate) struct TerminalSafe<'a>(&'a str);

/// Render printable text exactly while visibly escaping terminal controls and bidi.
pub fn terminal_safe(text: &str) -> impl fmt::Display + '_ {
    TerminalSafe(text)
}

pub(crate) fn needs_escape(character: char) -> bool {
    character.is_control()
        || matches!(character, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

enum SafeCharacter {
    Literal(Option<char>),
    Escaped(std::char::EscapeDebug),
}

impl Iterator for SafeCharacter {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self {
            Self::Literal(character) => character.take(),
            Self::Escaped(characters) => characters.next(),
        }
    }
}

/// The same projection for a cached panel document, without a temporary string.
pub(crate) fn characters(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().flat_map(|character| {
        if needs_escape(character) {
            SafeCharacter::Escaped(character.escape_debug())
        } else {
            SafeCharacter::Literal(Some(character))
        }
    })
}

impl fmt::Display for TerminalSafe<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in characters(self.0) {
            formatter.write_char(character)?;
        }
        Ok(())
    }
}

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
    use super::terminal_safe;

    #[test]
    fn printable_urls_and_messages_are_byte_exact() {
        let text = "https://example.com/a'b\"c\\日?state=original#fragment";
        assert_eq!(terminal_safe(text).to_string(), text);
    }

    #[test]
    fn terminal_controls_and_direction_controls_are_visible_not_active() {
        assert_eq!(
            terminal_safe("a\u{1b}[2J\r\n\t\u{202e}b").to_string(),
            r"a\u{1b}[2J\r\n\t\u{202e}b"
        );
        for character in [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ] {
            let value = character.to_string();
            assert!(!terminal_safe(&value).to_string().contains(character));
        }
    }

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
