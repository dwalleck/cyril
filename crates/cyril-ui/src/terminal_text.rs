//! Display agent- or server-supplied text literally while neutralizing
//! terminal controls. Display policy lives here, in the UI: core passes such
//! text through unchanged and the UI escapes it when it renders.

use std::fmt::{self, Write};

struct TerminalSafe<'a>(&'a str);

/// Render printable text exactly while visibly escaping terminal controls and bidi.
pub fn terminal_safe(text: &str) -> impl fmt::Display + '_ {
    TerminalSafe(text)
}

/// Characters that would act on the terminal, reorder text, break lines, or
/// hide inside copied text (zero-width and invisible format characters) if
/// printed raw. ZWJ/ZWNJ stay literal: emoji sequences and several scripts
/// need them to render as written.
pub fn needs_escape(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200b}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
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
pub fn characters(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().flat_map(|character| {
        if needs_escape(character) {
            SafeCharacter::Escaped(character.escape_debug())
        } else {
            SafeCharacter::Literal(Some(character))
        }
    })
}

/// [`terminal_safe`] per line: the text's own line breaks stay line breaks,
/// for multi-line diagnostics such as a disconnect reason.
pub fn terminal_safe_lines(text: &str) -> impl fmt::Display + '_ {
    TerminalSafeLines(text)
}

struct TerminalSafeLines<'a>(&'a str);

impl fmt::Display for TerminalSafeLines<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, line) in self.0.split('\n').enumerate() {
            if index > 0 {
                formatter.write_char('\n')?;
            }
            write!(formatter, "{}", terminal_safe(line))?;
        }
        Ok(())
    }
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
    fn line_safe_form_keeps_line_breaks_and_escapes_everything_else() {
        assert_eq!(
            super::terminal_safe_lines("first \u{202e}\nsecond\r\u{1b}[2J").to_string(),
            "first \\u{202e}\nsecond\\r\\u{1b}[2J"
        );
    }

    #[test]
    fn joiners_stay_literal_for_emoji_and_scripts() {
        for text in ["👩\u{200d}💻", "می\u{200c}خواهم"] {
            assert_eq!(terminal_safe(text).to_string(), text);
        }
    }

    #[test]
    fn terminal_controls_and_direction_controls_are_visible_not_active() {
        assert_eq!(
            terminal_safe("a\u{1b}[2J\r\n\t\u{202e}b").to_string(),
            r"a\u{1b}[2J\r\n\t\u{202e}b"
        );
        for character in [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{200b}', '\u{2028}',
            '\u{2029}', '\u{2060}', '\u{2064}', '\u{feff}',
        ] {
            let value = character.to_string();
            assert!(!terminal_safe(&value).to_string().contains(character));
        }
    }
}
