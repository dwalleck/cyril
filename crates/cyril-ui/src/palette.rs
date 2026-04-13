//! Named color constants for consistent cross-terminal rendering.
//!
//! All colors are true-color RGB values so they render identically
//! regardless of the user's terminal theme (unlike named colors like
//! `Color::Cyan` which map to the terminal's 16-color palette).

use ratatui::style::Color;

// --- Message labels ---
pub const USER_BLUE: Color = Color::Rgb(138, 180, 248);
pub const AGENT_GREEN: Color = Color::Rgb(129, 199, 132);
pub const SYSTEM_MAUVE: Color = Color::Rgb(180, 142, 173);
pub const MUTED_GRAY: Color = Color::Rgb(140, 140, 140);

// --- Code blocks ---
pub const CODE_BLOCK_BG: Color = Color::Rgb(40, 44, 52);

// --- Layout ---

/// Maximum width for code block borders and horizontal rules.
/// Prevents absurdly wide borders on ultra-wide terminals.
pub const MAX_BORDER_WIDTH: usize = 120;

// --- Spinner ---
pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
pub const SPINNER_FRAME_MS: u128 = 80;
