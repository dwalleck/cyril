//! The `/review` consent form (cyril-iowg): the target, the scope and its
//! file count, what the run costs, the check command, and every permission
//! the run's sessions get. Enter starts the review; Esc backs out.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::theme::Theme;
use crate::traits::{ReviewCheck, ReviewForm};
use crate::widgets::modal;

/// Measured on full runs of the recipe: 50 sessions in 39.4 min and 64
/// sessions in 50.9 min (project memory, 2026-09-20 / 2026-09-30).
const EXPECTED_RUN: &str = "about 50–65 model sessions, about 40–50 minutes";

/// What the armed run's step sessions may do, in plain words; everything
/// else is denied and logged, never prompted.
const MAY_DO: [&str; 4] = [
    "read files inside this repository",
    "run this review's own crtool steps (no other shell command)",
    "write only under its run directory, .code-review/<run>/",
    "everything else is denied and logged, never asked",
];

const LABEL_WIDTH: usize = 9;

pub fn render(frame: &mut Frame, area: Rect, input_top: u16, form: &ReviewForm, theme: &Theme) {
    let lines = lines(form, theme);
    let height = u16::try_from(lines.len() + 2).unwrap_or(u16::MAX);
    let Some(popup_area) = modal::place(area, input_top, 80, height) else {
        return; // no rows above the input can hold the form
    };
    frame.render_widget(Clear, popup_area);
    let popup = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(Span::styled(
                " Review ",
                Style::default()
                    .fg(theme.accent_quinary)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );
    frame.render_widget(popup, popup_area);
}

fn lines<'a>(form: &'a ReviewForm, theme: &Theme) -> Vec<Line<'a>> {
    let label = |text: &'static str| {
        Span::styled(
            format!("{text:<LABEL_WIDTH$}"),
            Style::default().fg(theme.subdued),
        )
    };
    let value = |text: String| Span::styled(text, Style::default().fg(theme.text));
    let detail = |text: String| Span::styled(text, Style::default().fg(theme.text_secondary));
    let files = match form.file_count {
        Some(1) => " — 1 file".to_owned(),
        Some(count) => format!(" — {count} files"),
        None => " — counting files…".to_owned(),
    };
    let check = match &form.check {
        ReviewCheck::NotConfigured => "no check configured".to_owned(),
        ReviewCheck::WillRun {
            command,
            timeout_secs,
        } => format!("will run: {command} (timeout {timeout_secs}s)"),
    };
    let mut lines = vec![
        Line::from(vec![label("Target"), value(form.target.clone())]),
        Line::from(vec![
            label("Scope"),
            value(form.scope.join(" ")),
            detail(files),
        ]),
        Line::from(vec![label("Runs"), detail(EXPECTED_RUN.to_owned())]),
        Line::from(vec![label("Check"), value(check)]),
    ];
    for (index, permission) in MAY_DO.iter().enumerate() {
        let lead = if index == 0 {
            label("May do")
        } else {
            label("")
        };
        lines.push(Line::from(vec![lead, detail(format!("• {permission}"))]));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        if form.busy {
            "Starting the review…"
        } else {
            "Enter starts the review · Esc cancels"
        },
        Style::default()
            .fg(theme.subdued)
            .add_modifier(Modifier::ITALIC),
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn form() -> ReviewForm {
        ReviewForm {
            target: "auto".to_owned(),
            scope: vec![".".to_owned()],
            file_count: Some(12),
            check: ReviewCheck::NotConfigured,
            busy: false,
        }
    }

    fn rendered(form: &ReviewForm) -> String {
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    22,
                    form,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                )
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_owned())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_form_shows_everything_consent_covers() {
        let text = rendered(&form());
        for expected in [
            " Review ",
            "Target   auto",
            "Scope    . — 12 files",
            "Runs     about 50–65 model sessions, about 40–50 minutes",
            "Check    no check configured",
            "May do   • read files inside this repository",
            "• run this review's own crtool steps (no other shell command)",
            "• write only under its run directory, .code-review/<run>/",
            "• everything else is denied and logged, never asked",
            "Enter starts the review · Esc cancels",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
    }

    #[test]
    fn counting_a_configured_check_and_busy_states_render() {
        let mut busy = form();
        busy.file_count = None;
        busy.check = ReviewCheck::WillRun {
            command: "cargo check".to_owned(),
            timeout_secs: 1800,
        };
        busy.busy = true;
        let text = rendered(&busy);
        assert!(text.contains("Scope    . — counting files…"), "{text}");
        assert!(
            text.contains("Check    will run: cargo check (timeout 1800s)"),
            "{text}"
        );
        assert!(text.contains("Starting the review…"), "{text}");
        assert!(!text.contains("Enter starts the review"), "{text}");
    }
}
