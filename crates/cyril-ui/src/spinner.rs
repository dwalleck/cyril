//! Spinner animation constants — the single source of truth.
//!
//! Consolidated from the legacy `palette` module and chat's private
//! duplicate during the semantic-theme contraction (cyril-6r3a).

/// Braille spinner animation frames, in display order.
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Milliseconds per spinner animation frame.
pub const SPINNER_FRAME_MS: u128 = 80;

#[cfg(test)]
mod tests {
    use super::*;

    /// cyril-6r3a C2 pin (permanent): the frozen glyph sequence and frame
    /// interval. A single-glyph transcription typo fails here.
    #[test]
    fn values_match_frozen_history() {
        assert_eq!(
            SPINNER_CHARS,
            &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
        );
        assert_eq!(SPINNER_FRAME_MS, 80);
    }
}
