//! The `/review` consent form (cyril-iowg): the target, the scope and its
//! file count, what the run costs, the check command, and every permission
//! the run's sessions get. Enter starts the review; Esc backs out.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::theme::Theme;
use crate::traits::{ReviewCheck, ReviewField, ReviewForm};
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
/// The form's outer width; long lines wrap inside it.
const WIDTH: u16 = 80;

/// Rows `lines` take once wrapped to `inner` columns, so the box never clips
/// the consent text below a long line.
fn wrapped_rows(lines: &[Line<'_>], inner: u16) -> usize {
    let inner = usize::from(inner.max(1));
    lines
        .iter()
        .map(|line| line.width().div_ceil(inner).max(1))
        .sum()
}

pub fn render(frame: &mut Frame, area: Rect, input_top: u16, form: &ReviewForm, theme: &Theme) {
    let lines = lines(form, theme);
    let height = u16::try_from(wrapped_rows(&lines, WIDTH - 2) + 2).unwrap_or(u16::MAX);
    let Some(popup_area) = modal::place(area, input_top, WIDTH, height) else {
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
    let files = match (form.file_count, &form.problem) {
        (Some(1), _) => " — 1 file".to_owned(),
        (Some(count), _) => format!(" — {count} files"),
        (None, Some(_)) => " — cannot be counted".to_owned(),
        (None, None) => " — counting files…".to_owned(),
    };
    let check = match &form.check {
        ReviewCheck::Reading => "reading .cyril/config.toml…".to_owned(),
        ReviewCheck::NotConfigured => "no check configured".to_owned(),
        ReviewCheck::WillRun {
            command,
            timeout_secs,
        } => format!("will run: {command} (timeout {timeout_secs}s)"),
    };
    let marker = |field: ReviewField| {
        if !form.busy && form.focus == field {
            Span::styled("◂ ▸", Style::default().fg(theme.accent_quinary))
        } else {
            Span::raw("")
        }
    };
    let mut lines = vec![Line::from(vec![
        label("Target"),
        value(form.target.label()),
        detail(format!("  ({}) ", form.target.spec())),
        marker(ReviewField::Target),
    ])];
    if let cyril_core::review::target::ReviewTarget::Base(base) = &form.target {
        lines.push(Line::from(vec![
            label("Base"),
            value(base.clone()),
            detail(format!("  ({} branches) ", form.branches.len())),
            marker(ReviewField::Base),
        ]));
    }
    // Git's errors span lines; the form shows them as one wrapped line.
    let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(problem) = &form.problem {
        lines.push(Line::from(vec![
            label(""),
            Span::styled(one_line(problem), Style::default().fg(theme.danger)),
        ]));
    }
    if let Some(note) = &form.note {
        lines.push(Line::from(vec![
            label(""),
            Span::styled(one_line(note), Style::default().fg(theme.warning)),
        ]));
    }
    lines.extend([
        Line::from(vec![
            label("Scope"),
            value(if form.scope.is_empty() {
                "…".to_owned()
            } else {
                form.scope.join(" ")
            }),
            detail(files),
        ]),
        Line::from(vec![label("Runs"), detail(EXPECTED_RUN.to_owned())]),
        Line::from(vec![label("Check"), value(check)]),
    ]);
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
            "←/→ change · Tab next field · Enter starts the review · Esc cancels"
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
    use cyril_core::review::target::ReviewTarget;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn form() -> ReviewForm {
        ReviewForm {
            scope: vec![".".to_owned()],
            file_count: Some(12),
            check: ReviewCheck::NotConfigured,
            ..ReviewForm::opening(ReviewTarget::Auto)
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
            "Target   auto  (auto) ◂ ▸",
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

    /// The base field exists only in base-branch mode, and the focus marker
    /// follows Tab.
    #[test]
    fn the_base_field_shows_only_in_base_mode() {
        let auto = rendered(&form());
        assert!(!auto.contains("Base "), "{auto}");
        let mut base = form();
        base.target = ReviewTarget::Base("main".to_owned());
        base.branches = vec!["main".to_owned(), "release".to_owned()];
        base.focus = ReviewField::Base;
        let text = rendered(&base);
        assert!(text.contains("Target   vs main  (main...HEAD)"), "{text}");
        assert!(text.contains("Base     main  (2 branches) ◂ ▸"), "{text}");
        let mut broken = form();
        broken.target = ReviewTarget::HeadCommit;
        broken.problem = Some("HEAD has no parent commit".to_owned());
        let text = rendered(&broken);
        assert!(
            text.contains("Target   the commit at HEAD  (HEAD~1..HEAD)"),
            "{text}"
        );
        assert!(text.contains("HEAD has no parent commit"), "{text}");
    }

    /// A long problem wraps inside the box, and the consent text below it
    /// stays on screen.
    #[test]
    fn a_long_problem_wraps_without_clipping_the_consent() {
        let mut broken = form();
        broken.target = ReviewTarget::HeadCommit;
        broken.file_count = None;
        broken.problem = Some(format!(
            "git diff failed:\nfatal: {}",
            "ambiguous argument 'HEAD~1..HEAD': unknown revision ".repeat(3)
        ));
        let text = rendered(&broken);
        assert!(text.contains("cannot be counted"), "{text}");
        assert!(
            text.contains("never asked"),
            "the last permission line is visible:\n{text}"
        );
        assert!(
            text.contains("Esc cancels"),
            "the key hint is visible:\n{text}"
        );
    }
}
