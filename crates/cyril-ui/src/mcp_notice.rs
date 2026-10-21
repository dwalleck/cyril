//! Display external MCP notices literally while neutralizing terminal controls.

use std::fmt::{self, Write};

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
}
