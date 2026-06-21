//! Voice input indicator (ROADMAP CN2 / V1a).
//!
//! A single status line shown just above the input while voice capture or
//! transcription is in progress. Mirrors `crew_panel`'s sizing contract:
//! `height_for()` is the single source of truth for both the layout constraint
//! in `render.rs` and the guard around `render()`.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use cyril_core::types::VoiceStatus;

use crate::palette;
use crate::traits::TuiState;

/// Number of cells in the level meter bar.
const METER_CELLS: usize = 12;

/// Height of the voice indicator: one line while active, hidden when idle.
/// Single source of truth for sizing (called by `render.rs` and `render`).
pub fn height_for(state: &dyn TuiState) -> u16 {
    match state.voice_status() {
        VoiceStatus::Idle => 0,
        _ => 1,
    }
}

/// Render the voice indicator. Draws nothing when idle.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let spans: Vec<Span> = match state.voice_status() {
        VoiceStatus::Idle => return,
        VoiceStatus::Listening => vec![
            Span::styled("🎙 listening ", Style::default().fg(palette::USER_BLUE)),
            Span::styled(meter_bar(state.voice_level()), Style::default()),
            Span::styled("  /voice to stop", Style::default().fg(palette::MUTED_GRAY)),
        ],
        VoiceStatus::Transcribing => vec![Span::styled(
            "⏳ transcribing…",
            Style::default().fg(palette::SYSTEM_MAUVE),
        )],
    };

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Build a fixed-width bar like `[█████░░░░░░░]` from a `0.0..=1.0` level.
///
/// Total for every `f32`: `clamp(0.0, 1.0)` never panics (the bounds are
/// valid), and the `as usize` cast saturates — so `NaN`/±∞/out-of-range all
/// resolve to a valid cell count (`NaN → 0`, an empty bar). The source of this
/// value, [`UiState::set_voice_level`], also normalizes `NaN` away.
fn meter_bar(level: f32) -> String {
    let filled = (level.clamp(0.0, 1.0) * METER_CELLS as f32).round() as usize;
    let filled = filled.min(METER_CELLS);
    let mut bar = String::with_capacity(METER_CELLS + 2);
    bar.push('[');
    for i in 0..METER_CELLS {
        bar.push(if i < filled { '█' } else { '░' });
    }
    bar.push(']');
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_bar_empty_and_full() {
        assert_eq!(meter_bar(0.0), "[░░░░░░░░░░░░]");
        assert_eq!(meter_bar(1.0), "[████████████]");
    }

    #[test]
    fn meter_bar_clamps_out_of_range() {
        assert_eq!(meter_bar(-1.0), "[░░░░░░░░░░░░]");
        assert_eq!(meter_bar(2.0), "[████████████]");
    }

    #[test]
    fn meter_bar_nan_renders_empty_without_panicking() {
        // Documents the totality: NaN saturates through the `as usize` cast to
        // an empty bar rather than panicking.
        assert_eq!(meter_bar(f32::NAN), "[░░░░░░░░░░░░]");
    }

    #[test]
    fn meter_bar_half() {
        // 0.5 * 12 = 6 filled cells.
        assert_eq!(meter_bar(0.5), "[██████░░░░░░]");
    }
}
