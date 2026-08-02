//! cyril-imjx regression fence: the picker marks the ACTIVE option.
//!
//! kiro-cli 2.14.2 never sends `current` on `/model` options, and only encodes
//! it into the `/effort` label (`"High  [active]"`) after an in-session change.
//! Cyril already knows both values — they drive the toolbar — so `show_picker`
//! joins the option list against its own state. These fences render the real
//! pipeline (`UiState::show_picker` → `picker::render`) over live-shaped
//! fixtures and assert the drawn marker, not just the state bit.
//!
//! Option shapes are transcribed from
//! `experiments/conductor-spike/trace-2.4.1-tui-recorder.jsonl`.

use cyril_core::types::CommandOption;
use cyril_ui::state::UiState;
use cyril_ui::traits::{PickerState, TuiState};
use cyril_ui::widgets::picker;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// The marker `picker::render` draws next to the active option.
const MARK: char = '✓';

/// A live `/model` option: `{value, label, description, group}` — no `current`.
fn model_option(id: &str) -> CommandOption {
    CommandOption {
        label: id.to_string(),
        value: id.to_string(),
        description: Some(format!("The {id} model")),
        group: Some("1.30x credits".into()),
        is_current: false,
    }
}

/// A live `/effort` option: `{value, label}` only.
fn effort_option(value: &str, label: &str) -> CommandOption {
    CommandOption {
        label: label.to_string(),
        value: value.to_string(),
        description: None,
        group: None,
        is_current: false,
    }
}

fn render_text(state: &PickerState) -> String {
    let (w, h) = (80u16, 24u16);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            let theme = cyril_ui::theme::resolve(
                cyril_ui::theme::ThemeId::CyrilDark,
                cyril_ui::theme::ColorMode::TrueColor,
            );
            picker::render(frame, frame.area(), frame.area().height, state, &theme);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buffer[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_open_picker(ui: &UiState) -> String {
    let Some(state) = ui.picker() else {
        panic!("show_picker did not open a picker");
    };
    render_text(state)
}

/// The row carrying the marker, with the popup border, the marker and the
/// selection caret stripped so the assertion is about WHICH option, not about
/// styling or placement.
fn marked_row(text: &str) -> String {
    let Some(line) = text.lines().find(|l| l.contains(MARK)) else {
        panic!("no row carries the {MARK} marker\n{text}");
    };
    line.replace(['│', MARK, '▸'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn picker_marks_active_row_exactly_once_for_model() {
    let mut ui = UiState::new(500);
    ui.set_current_model(Some("claude-opus-4.7".into()));
    ui.show_picker(
        "model".into(),
        vec![
            model_option("auto"),
            model_option("claude-opus-4.7"),
            model_option("claude-haiku-4.5"),
            model_option("glm-5"),
        ],
    );

    let text = render_open_picker(&ui);
    assert_eq!(
        text.matches(MARK).count(),
        1,
        "expected exactly one marked row\n{text}"
    );
    let row = marked_row(&text);
    assert!(
        row.starts_with("claude-opus-4.7"),
        "marker is on the wrong row: {row:?}\n{text}"
    );
}

#[test]
fn picker_marks_active_row_exactly_once_for_effort() {
    let mut ui = UiState::new(500);
    // Effort reaches cyril the same way the toolbar gets it: a metadata frame.
    ui.apply_notification(&cyril_core::types::Notification::MetadataUpdated {
        refusal: None,
        context_usage: None,
        metering: None,
        tokens: None,
        duration_ms: None,
        effort: cyril_core::types::EffortUpdate::Set(cyril_core::types::EffortLevel::High),
        session_id: None,
    });
    ui.show_picker(
        "effort".into(),
        vec![
            effort_option("low", "Low"),
            effort_option("medium", "Medium"),
            effort_option("high", "High"),
            effort_option("max", "Max"),
        ],
    );

    let text = render_open_picker(&ui);
    assert_eq!(
        text.matches(MARK).count(),
        1,
        "expected exactly one marked row\n{text}"
    );
    assert_eq!(marked_row(&text), "High", "\n{text}");
}

/// An agent that DOES send `current: true` still marks correctly when cyril
/// has no value of its own to join against.
#[test]
fn picker_marks_active_row_from_wire_current_fallback() {
    let mut ui = UiState::new(500);
    let mut options = vec![model_option("auto"), model_option("glm-5")];
    options[1].is_current = true;
    ui.show_picker("model".into(), options);

    let text = render_open_picker(&ui);
    assert_eq!(
        text.matches(MARK).count(),
        1,
        "expected exactly one marked row\n{text}"
    );
    let row = marked_row(&text);
    assert!(
        row.starts_with("glm-5"),
        "marker is on the wrong row: {row:?}\n{text}"
    );
}
