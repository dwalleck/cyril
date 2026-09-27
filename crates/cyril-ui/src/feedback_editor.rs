use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Maximum number of Unicode scalar values retained by one rejection reason.
const MAX_SCALAR_VALUES: usize = 4_096;
fn limit_notice() -> String {
    format!("Reason limit reached ({MAX_SCALAR_VALUES} characters).")
}

/// Opaque, request-local text editor used by the approval feedback phase.
///
/// The module is private so callers can observe a draft without taking ownership
/// of its invariants. Editing is deliberately crate-private: [`crate::state::UiState`]
/// owns the approval phase and responder while this type owns UTF-8 cursor
/// movement, normalization, filtering, and the scalar ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionFeedback {
    text: String,
    cursor: usize,
    scalar_count: usize,
    notice: Option<String>,
}

/// Result of routing one key while the feedback editor owns the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedbackAction {
    /// The draft or cursor changed, or the key was consumed without a phase
    /// transition. The caller should redraw and must not route the key to chat.
    Consumed,
    /// Enter submitted the current draft.
    Submit,
    /// Escape discarded the current draft and returned to option selection.
    Cancel,
}

impl RejectionFeedback {
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            scalar_count: 0,
            notice: None,
        }
    }

    /// The exact untrimmed draft currently shown to the operator.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn into_text(self) -> String {
        self.text
    }

    /// The UTF-8 byte offset of the cursor.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Number of Unicode scalar values in [`Self::text`].
    pub fn scalar_count(&self) -> usize {
        self.scalar_count
    }

    /// The visible notice from the most recent refused insertion, if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Whether the draft is empty or contains only Unicode whitespace.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Read the shared input ceiling without duplicating the literal in a
    /// renderer or state machine.
    pub const fn max_scalars() -> usize {
        MAX_SCALAR_VALUES
    }

    /// Route a key while the approval feedback phase owns the keyboard.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> FeedbackAction {
        match key.code {
            KeyCode::Esc if key.modifiers == KeyModifiers::NONE => FeedbackAction::Cancel,
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.insert_char('\n');
                FeedbackAction::Consumed
            }
            // Raw-mode LF is Ctrl+J on legacy terminals; unlike Shift+Enter,
            // it is distinguishable from the CR used to submit.
            KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => {
                self.insert_char('\n');
                FeedbackAction::Consumed
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => FeedbackAction::Submit,
            // Windows AltGr arrives as Ctrl+Alt, optionally with Shift.
            // Plain Ctrl/Alt shortcuts must still not become draft text.
            KeyCode::Char(character)
                if key.modifiers.is_empty()
                    || key.modifiers == KeyModifiers::SHIFT
                    || key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT)
                    || key.modifiers
                        == (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert_char(character);
                FeedbackAction::Consumed
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                self.backspace();
                FeedbackAction::Consumed
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                self.delete();
                FeedbackAction::Consumed
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                self.move_left();
                FeedbackAction::Consumed
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                self.move_right();
                FeedbackAction::Consumed
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                self.move_line_start();
                FeedbackAction::Consumed
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                self.move_line_end();
                FeedbackAction::Consumed
            }
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.move_vertical(false);
                FeedbackAction::Consumed
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.move_vertical(true);
                FeedbackAction::Consumed
            }
            _ => FeedbackAction::Consumed,
        }
    }

    /// Insert pasted text atomically after normalizing line endings and
    /// removing terminal control characters other than newline and tab.
    pub(crate) fn insert_text(&mut self, text: &str) {
        let remaining = MAX_SCALAR_VALUES.saturating_sub(self.scalar_count);
        let Some((filtered, inserted)) = normalize_and_filter(text, remaining) else {
            self.notice = Some(limit_notice());
            return;
        };
        if filtered.is_empty() {
            return;
        }

        self.text.insert_str(self.cursor, &filtered);
        self.cursor += filtered.len();
        self.scalar_count += inserted;
        self.notice = None;
    }

    fn insert_char(&mut self, character: char) {
        if character.is_control() && character != '\n' && character != '\t' {
            return;
        }
        if self.scalar_count >= MAX_SCALAR_VALUES {
            self.notice = Some(limit_notice());
            return;
        }
        self.text.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.scalar_count += 1;
        self.notice = None;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        self.text.drain(previous..self.cursor);
        self.cursor = previous;
        self.scalar_count = self.scalar_count.saturating_sub(1);
        self.notice = None;
    }

    fn delete(&mut self) {
        if self.cursor == self.text.len() {
            return;
        }
        let next = self.text[self.cursor..]
            .char_indices()
            .nth(1)
            .map_or(self.text.len(), |(index, _)| self.cursor + index);
        self.text.drain(self.cursor..next);
        self.scalar_count = self.scalar_count.saturating_sub(1);
        self.notice = None;
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(index, _)| index);
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map_or(self.text.len(), |(index, _)| self.cursor + index);
        }
    }
    fn move_line_start(&mut self) {
        self.cursor = self.text[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
    }

    fn move_line_end(&mut self) {
        self.cursor = self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |offset| self.cursor + offset);
    }

    fn move_vertical(&mut self, down: bool) {
        let line_start = self.text[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let column = self.text[line_start..self.cursor].chars().count();
        if down {
            let Some(newline_offset) = self.text[self.cursor..].find('\n') else {
                return;
            };
            let next_start = self.cursor + newline_offset + 1;
            let next_end = self.text[next_start..]
                .find('\n')
                .map_or(self.text.len(), |offset| next_start + offset);
            self.cursor = next_start
                + self.text[next_start..next_end]
                    .char_indices()
                    .nth(column)
                    .map_or(next_end - next_start, |(offset, _)| offset);
        } else {
            if line_start == 0 {
                return;
            }
            let previous_end = line_start - 1;
            let previous_start = self.text[..previous_end]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            self.cursor = previous_start
                + self.text[previous_start..previous_end]
                    .char_indices()
                    .nth(column)
                    .map_or(previous_end - previous_start, |(offset, _)| offset);
        }
    }
}

fn normalize_and_filter(input: &str, remaining: usize) -> Option<(String, usize)> {
    let mut chars = input.chars().peekable();
    let mut output = String::new();
    let mut accepted = 0usize;

    while let Some(character) = chars.next() {
        let normalized = if character == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            '\n'
        } else {
            character
        };

        if normalized.is_control() && normalized != '\n' && normalized != '\t' {
            continue;
        }
        accepted += 1;
        if accepted > remaining {
            return None;
        }
        output.push(normalized);
    }

    Some((output, accepted))
}

#[cfg(test)]
mod tests {
    // Independent approved fixture: mutation of the production ceiling in
    // either direction must fail this exact boundary assertion.
    const APPROVED_REASON_SCALARS: usize = 4_096;
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn rejection_feedback_accepts_unicode_and_preserves_exact_text() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text(" leading 解释\ntrailing ");

        assert_eq!(editor.text(), " leading 解释\ntrailing ");
        assert_eq!(editor.scalar_count(), editor.text().chars().count());
        assert_eq!(editor.cursor(), editor.text().len());
        assert_eq!(editor.notice(), None);
    }

    #[test]
    fn rejection_feedback_accepts_altgr_text_without_accepting_shortcuts() {
        let mut editor = RejectionFeedback::new();
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        for (character, modifiers) in [
            ('@', altgr),
            ('€', altgr | KeyModifiers::SHIFT),
            ('j', altgr),
        ] {
            assert_eq!(
                editor.handle_key(KeyEvent::new(KeyCode::Char(character), modifiers)),
                FeedbackAction::Consumed
            );
        }
        assert_eq!(editor.text(), "@€j");
        assert_eq!(editor.cursor(), "@€j".len());
        assert_eq!(editor.scalar_count(), 3);

        for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            editor.handle_key(KeyEvent::new(KeyCode::Char('x'), modifiers));
        }
        assert_eq!(editor.text(), "@€j", "shortcuts must not become draft text");
    }

    #[test]
    fn rejection_feedback_normalizes_crlf_and_filters_controls() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text("a\r\nb\rc\u{0000}d\u{001b}e\t f");

        assert_eq!(editor.text(), "a\nb\ncde\t f");
        assert_eq!(editor.scalar_count(), 10);
    }

    #[test]
    fn rejection_feedback_shift_enter_inserts_newline_and_enter_submits() {
        let mut editor = RejectionFeedback::new();
        assert_eq!(
            editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
            FeedbackAction::Consumed
        );
        assert_eq!(editor.text(), "\n");
        assert_eq!(
            editor.handle_key(key(KeyCode::Enter)),
            FeedbackAction::Submit
        );
    }

    #[test]
    fn rejection_feedback_ctrl_j_inserts_newline_without_submitting() {
        let mut editor = RejectionFeedback::new();
        assert_eq!(
            editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL)),
            FeedbackAction::Consumed
        );
        assert_eq!(editor.text(), "\n");
        assert_eq!(
            editor.handle_key(key(KeyCode::Enter)),
            FeedbackAction::Submit
        );
    }

    #[test]
    fn rejection_feedback_editor_keys_move_and_edit_scalar_boundaries() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text("a界b");
        editor.handle_key(key(KeyCode::Left));
        editor.handle_key(key(KeyCode::Backspace));
        assert_eq!(editor.text(), "ab");
        editor.handle_key(key(KeyCode::Home));
        editor.handle_key(key(KeyCode::Delete));
        assert_eq!(editor.text(), "b");
        editor.handle_key(key(KeyCode::End));
        editor.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::SHIFT));
        assert_eq!(editor.text(), "b!");
    }

    #[test]
    fn rejection_feedback_overflow_is_atomic_and_notice_is_visible() {
        assert_eq!(
            RejectionFeedback::max_scalars(),
            APPROVED_REASON_SCALARS,
            "the production bound must match the approved fixture"
        );
        let mut editor = RejectionFeedback::new();
        editor.insert_text(&"界".repeat(APPROVED_REASON_SCALARS));
        let before = editor.clone();

        editor.insert_text("x");

        assert_eq!(editor.text(), before.text());
        assert_eq!(editor.cursor(), before.cursor());
        assert_eq!(editor.scalar_count(), APPROVED_REASON_SCALARS);
        assert!(editor.notice().is_some());
    }

    #[test]
    fn rejection_feedback_typed_insertions_obey_scalar_limit() {
        for key in [
            KeyEvent::new(KeyCode::Char('界'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        ] {
            let mut editor = RejectionFeedback::new();
            editor.insert_text(&"界".repeat(APPROVED_REASON_SCALARS - 1));
            assert_eq!(editor.handle_key(key), FeedbackAction::Consumed);
            assert_eq!(editor.scalar_count(), APPROVED_REASON_SCALARS);
            assert_eq!(editor.notice(), None);
            let before = editor.clone();
            assert_eq!(editor.handle_key(key), FeedbackAction::Consumed);
            assert_eq!(editor.text(), before.text(), "refused key: {key:?}");
            assert_eq!(editor.cursor(), before.cursor(), "refused key: {key:?}");
            assert!(editor.notice().is_some(), "missing refusal notice: {key:?}");
        }
    }

    #[test]
    fn rejection_feedback_oversized_paste_is_atomic_after_filtering() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text("prefix");
        let before = editor.clone();
        let oversized = format!("{}{}", "\u{0000}".repeat(10_000), "x".repeat(4_100));

        editor.insert_text(&oversized);

        assert_eq!(editor.text(), before.text());
        assert_eq!(editor.cursor(), before.cursor());
        assert!(editor.notice().is_some());
    }

    #[test]
    fn rejection_feedback_blank_detection_keeps_whitespace() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text(" \n\t");
        assert!(editor.is_blank());
        assert_eq!(editor.text(), " \n\t");
    }

    #[test]
    fn rejection_feedback_escape_discards_at_phase_boundary() {
        let mut editor = RejectionFeedback::new();
        editor.insert_text("draft");
        assert_eq!(editor.handle_key(key(KeyCode::Esc)), FeedbackAction::Cancel);
        assert_eq!(editor.text(), "draft");
    }
}
