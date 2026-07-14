use cyril_core::types::{AgentMessage, Notification};
use cyril_ui::state::UiState;
use cyril_ui::theme::{resolve, ColorMode, ThemeId};
use cyril_ui::widgets::chat;
use ratatui::{backend::TestBackend, buffer::Buffer, style::Color, Terminal};
use std::{error::Error, io};

fn locate(buffer: &Buffer, needle: &str) -> Result<(u16, u16, Color), Box<dyn Error>> {
    for y in 0..24 {
        let mut row = String::new();
        for x in 0..80 {
            let cell = buffer
                .cell((x, y))
                .ok_or_else(|| io::Error::other("missing cell"))?;
            row.push_str(cell.symbol());
        }
        if let Some(byte_x) = row.find(needle) {
            let x = row[..byte_x].chars().count() as u16;
            let cell = buffer
                .cell((x, y))
                .ok_or_else(|| io::Error::other("missing marker"))?;
            return Ok((x, y, cell.fg));
        }
    }
    Err(io::Error::other(format!("marker not visible: {needle}")).into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut state = UiState::new(20);
    state.add_user_message("Q9DX-USER");
    state.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "Q9DX-AGENT".into(),
        is_streaming: false,
    }));
    state.add_system_message("Q9DX-SYSTEM".into());

    let mut theme = resolve(ThemeId::CyrilDark, ColorMode::Ansi16);
    theme.user = Color::LightBlue;
    theme.agent = Color::LightGreen;
    theme.system = Color::LightMagenta;

    let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
    terminal.draw(|frame| chat::render(frame, frame.area(), &state, &theme))?;
    let buffer = terminal.backend().buffer();
    println!("role\tmarker\tx\ty\tforeground");
    for (role, marker) in [
        ("user", "You:"),
        ("agent", "Kiro:"),
        ("system", "Q9DX-SYSTEM"),
    ] {
        let (x, y, foreground) = locate(buffer, marker)?;
        println!("{role}\t{marker}\t{x}\t{y}\t{foreground:?}");
    }
    Ok(())
}
