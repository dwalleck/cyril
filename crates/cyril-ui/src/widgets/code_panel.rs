use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use cyril_core::types::{CodePanelData, LspStatus};

/// Render the code intelligence panel as an input-protected overlay.
///
/// `input_top` is the absolute row of the input box's top border; placement
/// goes through [`crate::widgets::modal::place`] so the popup never covers
/// the input (cyril-a14l C7).
pub fn render(frame: &mut Frame, area: Rect, input_top: u16, data: &CodePanelData, theme: &Theme) {
    let mut lines: Vec<Line> = Vec::new();

    // Status line
    let (icon, color) = status_style(&data.status, theme);
    let mut status_spans = vec![Span::styled(
        format!("{icon} {}", status_label(&data.status)),
        Style::default().fg(color),
    )];
    if let Some(ref msg) = data.message {
        status_spans.push(Span::styled(
            format!(" — {msg}"),
            Style::default().fg(theme.subdued),
        ));
    }
    lines.push(Line::from(status_spans));

    // Warning
    if let Some(ref warning) = data.warning {
        lines.push(Line::default());
        lines.push(Line::styled(
            format!("⚠ {warning}"),
            Style::default().fg(theme.emphasis),
        ));
    }

    // Workspace info
    if data.root_path.is_some()
        || !data.detected_languages.is_empty()
        || !data.project_markers.is_empty()
    {
        lines.push(Line::default());

        if let Some(ref root) = data.root_path {
            lines.push(Line::from(vec![
                Span::styled("Workspace: ", Style::default().fg(theme.accent_quinary)),
                Span::styled(root.as_str(), Style::default().fg(theme.subdued)),
            ]));
        }
        if !data.detected_languages.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Languages: ", Style::default().fg(theme.accent_quinary)),
                Span::styled(
                    data.detected_languages.join(", "),
                    Style::default().fg(theme.subdued),
                ),
            ]));
        }
        if !data.project_markers.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Markers:   ", Style::default().fg(theme.accent_quinary)),
                Span::styled(
                    data.project_markers.join(", "),
                    Style::default().fg(theme.subdued),
                ),
            ]));
        }
    }

    // LSP servers
    if !data.lsps.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(
            "LSP Servers:",
            Style::default().fg(theme.accent_quinary),
        ));

        let max_name_len = data.lsps.iter().map(|l| l.name.len()).max().unwrap_or(8);

        for lsp in &data.lsps {
            let (lsp_icon, lsp_color) = match &lsp.status {
                Some(s) => status_style(s, theme),
                None => ("○", theme.subdued),
            };
            let label = match &lsp.status {
                Some(s) => status_label(s),
                None => "—",
            };
            let langs = format!("({})", lsp.languages.join(", "));
            let duration = lsp
                .init_duration_ms
                .map(|ms| format!(" ({ms}ms)"))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{lsp_icon} {:width$}", lsp.name, width = max_name_len),
                    Style::default().fg(lsp_color),
                ),
                Span::styled(format!("  {langs:16}"), Style::default().fg(theme.subdued)),
                Span::styled(format!("{label}{duration}"), Style::default().fg(lsp_color)),
            ]));
        }
    }

    // Config path
    if let Some(ref config) = data.config_path {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Config: ", Style::default().fg(theme.subdued)),
            Span::styled(config.as_str(), Style::default().fg(theme.accent_quinary)),
        ]));
    }

    // Footer
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("[r]", Style::default().fg(theme.accent_quinary)),
        Span::styled(" refresh  ", Style::default().fg(theme.subdued)),
        Span::styled("[Esc]", Style::default().fg(theme.accent_quinary)),
        Span::styled(" close", Style::default().fg(theme.subdued)),
    ]));

    // Size and position: input-protected placement (cyril-a14l C7).
    let content_width = lines.iter().map(|l| l.width()).max().unwrap_or(30) as u16 + 4;
    let Some(popup_area) = crate::widgets::modal::place(
        area,
        input_top,
        content_width.clamp(40, 80),
        (lines.len() as u16).saturating_add(2),
    ) else {
        return; // no rows above the input can hold the popup
    };

    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                " /code ",
                Style::default()
                    .fg(theme.accent_quinary)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );

    frame.render_widget(popup, popup_area);
}

fn status_style(status: &LspStatus, theme: &Theme) -> (&'static str, Color) {
    match status {
        LspStatus::Initialized => ("✓", theme.subdued_positive),
        LspStatus::Initializing => ("◐", theme.emphasis),
        LspStatus::Failed => ("✗", theme.subdued_negative),
        LspStatus::Unknown(_) => ("○", theme.subdued),
    }
}

fn status_label(status: &LspStatus) -> &str {
    match status {
        LspStatus::Initialized => "initialized",
        LspStatus::Initializing => "initializing",
        LspStatus::Failed => "failed",
        LspStatus::Unknown(s) => s.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::LspServerInfo;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample_panel_data() -> CodePanelData {
        CodePanelData {
            status: LspStatus::Initialized,
            message: Some("LSP servers ready".into()),
            warning: None,
            root_path: Some("/home/user/repos/cyril".into()),
            detected_languages: vec!["rust".into()],
            project_markers: vec!["Cargo.toml".into()],
            config_path: Some(".kiro/settings/lsp.json".into()),
            doc_url: None,
            lsps: vec![
                LspServerInfo {
                    name: "rust-analyzer".into(),
                    languages: vec!["rust".into()],
                    status: Some(LspStatus::Initialized),
                    init_duration_ms: Some(44),
                },
                LspServerInfo {
                    name: "pyright".into(),
                    languages: vec!["python".into()],
                    status: Some(LspStatus::Failed),
                    init_duration_ms: None,
                },
            ],
        }
    }

    #[test]
    fn code_panel_renders_without_panic() {
        let data = sample_panel_data();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    frame.area().height,
                    &data,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                );
            })
            .expect("draw");
    }

    #[test]
    fn code_panel_renders_with_warning() {
        let mut data = sample_panel_data();
        data.warning = Some("pyright not found on PATH".into());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    frame.area().height,
                    &data,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                );
            })
            .expect("draw");
    }

    #[test]
    fn code_panel_renders_empty_lsps() {
        let data = CodePanelData {
            status: LspStatus::Initializing,
            message: Some("Detecting workspace...".into()),
            warning: None,
            root_path: None,
            detected_languages: vec![],
            project_markers: vec![],
            config_path: None,
            doc_url: None,
            lsps: vec![],
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    frame.area().height,
                    &data,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                );
            })
            .expect("draw");
    }

    #[test]
    fn code_panel_renders_narrow_terminal() {
        let data = sample_panel_data();
        let backend = TestBackend::new(30, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    frame.area().height,
                    &data,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                );
            })
            .expect("draw");
    }
}
