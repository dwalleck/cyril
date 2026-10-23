//! Literal read-only MCP content with one geometry shared by rendering and keys.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::mcp_panel::McpPanelState;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    pub(crate) popup: Rect,
    pub(crate) body: Rect,
    header: Rect,
    footer: Rect,
    bordered: bool,
}

pub(crate) fn placement(area: Rect, input_top: u16) -> Option<Placement> {
    let popup = crate::widgets::modal::place(area, input_top, 112, 30)?;
    // A very short popup spends its cells on actionable content, not a frame.
    let bordered = popup.height >= 5 && popup.width >= 4;
    let inner = if bordered {
        popup.inner(Margin::new(1, 1))
    } else {
        popup
    };
    let footer_height = u16::from(inner.height > 0);
    let header_height = u16::from(inner.height >= 3);
    let body_height = inner.height.saturating_sub(footer_height + header_height);
    Some(Placement {
        popup,
        header: Rect::new(inner.x, inner.y, inner.width, header_height),
        body: Rect::new(inner.x, inner.y + header_height, inner.width, body_height),
        footer: Rect::new(
            inner.x,
            inner.bottom().saturating_sub(footer_height),
            inner.width,
            footer_height,
        ),
        bordered,
    })
}

pub fn render(frame: &mut Frame, area: Rect, input_top: u16, state: &McpPanelState, theme: &Theme) {
    let Some(placed) = placement(area, input_top) else {
        return;
    };
    super::modal::clear(frame, placed.popup);
    frame.render_widget(
        Block::default()
            .borders(if placed.bordered {
                Borders::ALL
            } else {
                Borders::NONE
            })
            .border_style(Style::default().fg(theme.accent_quinary))
            .style(Style::default().bg(theme.canvas)),
        placed.popup,
    );
    if placed.header.height > 0 {
        frame.buffer_mut().set_stringn(
            placed.header.x,
            placed.header.y,
            &state.heading,
            usize::from(placed.header.width),
            Style::default()
                .fg(theme.accent_quinary)
                .add_modifier(Modifier::BOLD),
        );
    }
    if placed.footer.height > 0 {
        frame.buffer_mut().set_stringn(
            placed.footer.x,
            placed.footer.y,
            &state.footer,
            usize::from(placed.footer.width),
            Style::default().fg(theme.info),
        );
    }
    // Rows were escaped and physically wrapped on state/geometry changes. A
    // draw borrows just the visible strings, never clones schemas or previews.
    let window = usize::from(placed.body.height);
    let first = state
        .scroll
        .min(state.document.len().saturating_sub(window.max(1)));
    for (index, row) in state.document.iter().enumerate().skip(first).take(window) {
        let style = if index == state.focus_line {
            Style::default()
                .fg(theme.text)
                .bg(theme.selection)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };
        let y = placed.body.y + (index - first) as u16;
        let width = usize::from(placed.body.width);
        if let Some(caret) = state.caret.as_ref().filter(|caret| caret.row == index) {
            // These byte boundaries came from the editor's source offset.
            // Separate writes keep adjacent user graphemes out of the caret.
            frame.buffer_mut().set_stringn(
                placed.body.x,
                y,
                &row[..caret.bytes.start],
                caret.column.min(width),
                style,
            );
            if caret.column < width {
                let x = placed.body.x + caret.column as u16;
                frame
                    .buffer_mut()
                    .set_stringn(x, y, &row[caret.bytes.clone()], 1, style);
                frame.buffer_mut().set_stringn(
                    x + 1,
                    y,
                    &row[caret.bytes.end..],
                    width - caret.column - 1,
                    style,
                );
            }
        } else {
            frame
                .buffer_mut()
                .set_stringn(placed.body.x, y, row, width, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cyril_core::types::{McpState, SessionId};
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn geometry_uses_actual_modal_placement_and_protects_input() {
        for (width, height, input_top) in [(80, 24, 19), (40, 12, 7), (20, 8, 4)] {
            let area = Rect::new(0, 0, width, height);
            let placed = placement(area, input_top).unwrap_or_else(|| panic!("available popup"));
            assert_eq!(
                Some(placed.popup),
                crate::widgets::modal::place(area, input_top, 112, 30)
            );
            assert!(placed.popup.bottom() <= input_top);
            assert!(placed.footer.height > 0);
        }
        assert!(placement(Rect::new(0, 0, 2, 2), 1).is_none());
    }

    #[test]
    fn unreported_state_and_small_terminal_footer_are_visible()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = Terminal::new(TestBackend::new(60, 12))?;
        let mut state = McpPanelState::new(Arc::new(McpState::for_session(SessionId::new("root"))));
        let placed = placement(Rect::new(0, 0, 60, 12), 9).unwrap_or_else(|| panic!("placement"));
        state.geometry(
            usize::from(placed.body.width),
            usize::from(placed.body.height),
        );
        let theme = crate::theme::resolve_no_color(crate::theme::ThemeId::CyrilDark);
        terminal.draw(|frame| {
            let area = frame.area();
            render(frame, area, 9, &state, &theme);
        })?;
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Esc close"));
        assert!(state.document.join("\n").contains("unknown, not zero"));
        Ok(())
    }
}
