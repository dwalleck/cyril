#!/usr/bin/env python3
"""Throwaway runtime probe for cyril-qaq0's existing public seams."""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TEST = ROOT / "crates/cyril-ui/tests/qaq0_runtime_probe.rs"
SOURCE = r'''
use cyril_core::types::{CommandOption, config::Config};
use cyril_ui::state::UiState;
use cyril_ui::theme::{ColorMode, ThemeId, resolve};
use cyril_ui::traits::TuiState;

fn option(label: &str, value: &str) -> CommandOption {
    CommandOption { label: label.into(), value: value.into(), description: None, group: None, is_current: false }
}

#[test]
fn observe_existing_qaq0_seams() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[ui]\nmax_messages = 321\nmouse_capture = false\ntheme = \"unknown\"\ncolor_mode = \"bogus\"\n")?;
    let config = Config::load_from_path(&path);
    println!("QAQ0 config_preserved=max_messages:{} mouse_capture:{}", config.ui.max_messages, config.ui.mouse_capture);

    let mut state = UiState::new(10);
    let before = state.theme();
    state.show_picker("theme".into(), vec![option("Dark", "cyril-dark"), option("Light", "cyril-light")]);
    state.picker_select_next();
    println!("QAQ0 picker_move=selected:{} theme_changed:{}", state.picker().map_or(99, |p| p.selected), state.theme() != before);
    state.picker_cancel();
    println!("QAQ0 picker_cancel=open:{} theme_changed:{}", state.picker().is_some(), state.theme() != before);
    state.show_picker("theme".into(), vec![option("Dark", "cyril-dark"), option("Light", "cyril-light")]);
    state.picker_select_next();
    println!("QAQ0 picker_confirm={:?} theme_changed:{}", state.picker_confirm(), state.theme() != before);

    for mode in [ColorMode::TrueColor, ColorMode::Ansi256, ColorMode::Ansi16, ColorMode::None] {
        let theme = resolve(ThemeId::CyrilDark, mode);
        println!("QAQ0 mode={mode:?} user={:?} syntax={:?}", theme.user, theme.syntax);
    }
    Ok(())
}
'''

with TEST.open("w", encoding="utf-8", newline="\n") as handle:
    handle.write(SOURCE)
try:
    command = ["cargo", "test", "-p", "cyril-ui", "--test", "qaq0_runtime_probe", "--", "--nocapture"]
    run = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    output = [
        line
        for line in (run.stdout + "\n" + run.stderr).splitlines()
        if "QAQ0" in line or line.startswith("test result:")
    ]
    sys.stdout.write("\n".join(output) + "\n")
    if run.returncode:
        sys.stderr.write("\n".join(run.stderr.splitlines()[-20:]) + "\n")
    raise SystemExit(run.returncode)
finally:
    TEST.unlink(missing_ok=True)
