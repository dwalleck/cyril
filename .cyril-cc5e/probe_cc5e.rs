//! cyril-cc5e probe: which picker option rows does the render actually draw?
//!
//! Emits machine-comparable `SCENARIO` lines for the oracle diff:
//!   SCENARIO <name> w=<w> h=<h> n=<n> sel=<k> marker=<bool> drawn=<labels>
//! Run: cargo test -p cyril-ui --test probe_cc5e -- --nocapture

use cyril_core::types::CommandOption;
use cyril_ui::state::UiState;
use cyril_ui::traits::{PickerState, TuiState};
use cyril_ui::widgets::picker;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn opts(n: usize) -> Vec<CommandOption> {
    (0..n)
        .map(|i| CommandOption {
            label: format!("opt-{i:02}"),
            value: format!("v{i:02}"),
            description: Some(format!("desc-{i:02}")),
            group: None,
            is_current: i == 0,
        })
        .collect()
}

fn picker_state(n: usize, selected: usize) -> PickerState {
    PickerState {
        title: "Probe".into(),
        options: opts(n),
        filter: String::new(),
        filtered_indices: (0..n).collect(),
        selected,
    }
}

fn buffer_text(terminal: &Terminal<TestBackend>, w: u16, h: u16) -> String {
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

fn run_scenario(name: &str, w: u16, h: u16, n: usize, selected: usize) {
    let state = picker_state(n, selected);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| picker::render(frame, frame.area(), &state))
        .expect("draw");
    let text = buffer_text(&terminal, w, h);

    let mut drawn: Vec<String> = Vec::new();
    for i in 0..n {
        let label = format!("opt-{i:02}");
        if text.contains(&label) {
            drawn.push(label);
        }
    }
    let marker = text.contains('▸');
    println!(
        "SCENARIO {name} w={w} h={h} n={n} sel={selected} marker={marker} drawn={}",
        drawn.join(",")
    );
}

#[test]
fn probe_picker_visibility() {
    // Q1: which rows are drawn, and is the selection marker on screen?
    run_scenario("A-control-80x24", 80, 24, 30, 5);
    run_scenario("B-deep-sel-80x24", 80, 24, 30, 20);
    run_scenario("C-floor-60x16", 60, 16, 15, 14);
    run_scenario("D-floor-top-60x16", 60, 16, 15, 0);

    // Reachability: `selected` really can leave the drawn window via plain
    // key navigation (the state machine bounds it by filtered len, not by
    // what the render shows).
    let mut ui = UiState::new(1000);
    ui.show_picker("Probe".into(), opts(30));
    for _ in 0..20 {
        ui.picker_select_next();
    }
    let sel = ui.picker().expect("picker active").selected;
    println!("STATE reachable-selected={sel} (20 presses, 30 options)");
    assert_eq!(sel, 20, "selection must be reachable past the drawn window");
}
