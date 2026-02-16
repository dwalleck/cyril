use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use cyril_core::event::AppEvent;

/// Unified event type for the TUI application.
#[derive(Debug)]
pub enum Event {
    /// Terminal key press.
    Key(KeyEvent),
    /// Terminal mouse event.
    Mouse(MouseEvent),
    /// Terminal resize.
    #[allow(dead_code)]
    Resize(u16, u16),
    /// Render tick (30fps).
    Tick,
    /// ACP event from the agent.
    Acp(AppEvent),
}

impl From<CrosstermEvent> for Event {
    fn from(event: CrosstermEvent) -> Self {
        match event {
            CrosstermEvent::Key(key) => Event::Key(key),
            CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
            CrosstermEvent::Resize(w, h) => Event::Resize(w, h),
            _ => Event::Tick, // Focus events etc. treated as no-op
        }
    }
}
