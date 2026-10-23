//! Prompt strings are opaque values, not approval feedback or chat input.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub(super) struct ArgumentValue {
    pub(super) present: bool,
    pub(super) text: String,
    pub(super) cursor: usize,
}

impl ArgumentValue {
    pub(super) fn insert(&mut self, text: &str) {
        // Preserve every byte, including whitespace and CRLF. Rendering escapes
        // terminal controls separately; editing never changes the sent value.
        self.present = true;
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    pub(super) fn key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.present = false;
                self.text.clear();
                self.cursor = 0;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.present = true;
                self.text.clear();
                self.cursor = 0;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('j')) | (KeyModifiers::SHIFT, KeyCode::Enter) => {
                self.insert("\n")
            }
            (KeyModifiers::NONE, KeyCode::Backspace) if self.cursor > 0 => {
                let previous = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
                self.text.drain(previous..self.cursor);
                self.cursor = previous;
            }
            (KeyModifiers::NONE, KeyCode::Delete) if self.cursor < self.text.len() => {
                let next = self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(self.cursor, |character| self.cursor + character.len_utf8());
                self.text.drain(self.cursor..next);
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.cursor = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                if let Some(character) = self.text[self.cursor..].chars().next() {
                    self.cursor += character.len_utf8();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => self.cursor = 0,
            (KeyModifiers::NONE, KeyCode::End) => self.cursor = self.text.len(),
            (modifiers, KeyCode::Char(character))
                if modifiers.is_empty()
                    || modifiers == KeyModifiers::SHIFT
                    || modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT)
                    || modifiers
                        == (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert(character.encode_utf8(&mut [0; 4]));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_absence_and_explicit_empty_are_different() {
        let mut value = ArgumentValue::default();
        assert!(!value.present);
        value.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(value.present);
        assert!(value.text.is_empty());
        value.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(!value.present);
    }

    #[test]
    fn strings_preserve_whitespace_controls_and_more_than_feedback_limit() {
        let text = format!("  \t\r\n{}\u{1b}  ", "日".repeat(5_000));
        let mut value = ArgumentValue::default();
        value.insert(&text);
        assert_eq!(value.text, text);
        assert_eq!(value.cursor, text.len());
    }

    #[test]
    fn editing_keeps_unicode_boundaries_and_present_empty() {
        let mut value = ArgumentValue::default();
        value.insert("日x");
        value.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        value.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(value.text, "x");
        value.key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        assert!(value.text.is_empty());
        assert!(value.present);
        assert_eq!(value.cursor, 0);
    }
}
