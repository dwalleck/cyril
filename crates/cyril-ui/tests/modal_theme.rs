//! cyril-nrnq fences: modal semantic-color migration.
//!
//! Ground truth is `.cyril-nrnq/modal-baseline.tsv` — a cell dump of the five
//! modal scenes rendered by the PRE-migration widgets (frozen before any
//! migration commit; regenerate only via the `#[ignore]`d `generate_baseline`
//! test, and never after the migration slices land).

use std::path::Path;

use cyril_core::types::{
    CodePanelData, CommandOption, HookInfo, LspServerInfo, LspStatus, PermissionOption,
    PermissionOptionId, PermissionOptionKind, ToolCall, ToolCallId, ToolCallStatus, ToolKind,
    TrustOption,
};
use cyril_ui::theme::{ColorMode, Theme, ThemeId, resolve};
use cyril_ui::traits::{ApprovalPhase, ApprovalState, HooksPanelState, PickerState};
use cyril_ui::widgets::{approval, code_panel, hooks_panel, picker};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Style;

const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.cyril-nrnq/modal-baseline.tsv"
);
const SCENE_W: u16 = 80;
const SCENE_H: u16 = 24;

// ── Scene fixtures (identical to .cyril-nrnq/probe_nrnq.rs) ────────────────

fn approval_state(trust_phase: bool) -> ApprovalState {
    ApprovalState {
        tool_call: ToolCall::new(
            ToolCallId::new("tc_1"),
            "echo hello".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            None,
        ),
        message: "Allow execution?".into(),
        options: vec![
            PermissionOption {
                id: PermissionOptionId::new("allow"),
                label: "Allow Once".into(),
                kind: PermissionOptionKind::AllowOnce,
                is_destructive: false,
            },
            PermissionOption {
                id: PermissionOptionId::new("reject"),
                label: "Reject".into(),
                kind: PermissionOptionKind::RejectOnce,
                is_destructive: true,
            },
        ],
        trust_options: if trust_phase {
            vec![
                TrustOption {
                    label: "Session".into(),
                    display: "this session only".into(),
                    setting_key: "s".into(),
                    patterns: vec!["*".into()],
                },
                TrustOption {
                    label: "Always".into(),
                    display: "persist to agent config".into(),
                    setting_key: "a".into(),
                    patterns: vec!["*".into()],
                },
            ]
        } else {
            vec![]
        },
        selected: 0,
        phase: if trust_phase {
            ApprovalPhase::SelectTrust {
                chosen_option_id: PermissionOptionId::new("allow"),
            }
        } else {
            ApprovalPhase::SelectOption
        },
        responder: tokio::sync::oneshot::channel().0,
    }
}

fn picker_scene_state() -> PickerState {
    PickerState {
        title: "Probe".into(),
        options: (0..3)
            .map(|i| CommandOption {
                label: format!("opt-{i}"),
                value: format!("v{i}"),
                description: Some(format!("desc-{i}")),
                group: Some("tier".into()),
                is_current: i == 0,
            })
            .collect(),
        filter: "ab".into(),
        filtered_indices: vec![0, 1, 2],
        selected: 1,
    }
}

fn hooks_scene_state() -> HooksPanelState {
    HooksPanelState {
        hooks: vec![
            HookInfo {
                trigger: "agentSpawn".into(),
                command: "echo spawn".into(),
                matcher: None,
            },
            HookInfo {
                trigger: "userPromptSubmit".into(),
                command: "lint".into(),
                matcher: Some("*.rs".into()),
            },
        ],
        scroll_offset: 0,
    }
}

fn code_scene_state() -> CodePanelData {
    CodePanelData {
        status: LspStatus::Initialized,
        message: Some("ready".into()),
        warning: Some("partial index".into()),
        root_path: Some("/repo".into()),
        detected_languages: vec!["rust".into()],
        project_markers: vec!["Cargo.toml".into()],
        config_path: Some("/repo/.lsp.json".into()),
        doc_url: None,
        lsps: vec![
            LspServerInfo {
                name: "ra".into(),
                languages: vec!["rust".into()],
                status: Some(LspStatus::Initialized),
                init_duration_ms: Some(1200),
            },
            LspServerInfo {
                name: "ts".into(),
                languages: vec!["ts".into()],
                status: Some(LspStatus::Initializing),
                init_duration_ms: None,
            },
            LspServerInfo {
                name: "py".into(),
                languages: vec!["py".into()],
                status: Some(LspStatus::Failed),
                init_duration_ms: None,
            },
            LspServerInfo {
                name: "go".into(),
                languages: vec!["go".into()],
                status: None,
                init_duration_ms: None,
            },
        ],
    }
}

/// Render one scene and dump every cell with a non-default style or a
/// non-space symbol as `scene\tx\ty\tsymbol\tfg\tbg\tmods` rows.
fn scene_rows(scene: &str, draw: impl Fn(&mut ratatui::Frame)) -> Vec<String> {
    let backend = TestBackend::new(SCENE_W, SCENE_H);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| draw(f)).expect("draw");
    let buffer = terminal.backend().buffer();
    let mut rows = Vec::new();
    for y in 0..SCENE_H {
        for x in 0..SCENE_W {
            let cell = &buffer[(x, y)];
            if cell.style() == Style::default() && cell.symbol() == " " {
                continue;
            }
            rows.push(format!(
                "{scene}\t{x}\t{y}\t{}\t{:?}\t{:?}\t{:?}",
                cell.symbol(),
                cell.fg,
                cell.bg,
                cell.modifier
            ));
        }
    }
    rows
}

fn truecolor_theme() -> Theme {
    resolve(ThemeId::CyrilDark, ColorMode::TrueColor)
}

fn all_scene_rows() -> Vec<String> {
    let theme = truecolor_theme();
    let mut rows = Vec::new();
    let opt = approval_state(false);
    rows.extend(scene_rows("approval-option", |f| {
        approval::render(f, f.area(), &opt, &theme);
    }));
    let trust = approval_state(true);
    rows.extend(scene_rows("approval-trust", |f| {
        approval::render(f, f.area(), &trust, &theme);
    }));
    let pick = picker_scene_state();
    rows.extend(scene_rows("picker", |f| {
        picker::render(f, f.area(), &pick, &theme)
    }));
    let hooks = hooks_scene_state();
    rows.extend(scene_rows("hooks", |f| {
        hooks_panel::render(f, f.area(), &hooks, &theme);
    }));
    let code = code_scene_state();
    rows.extend(scene_rows("code", |f| {
        code_panel::render(f, f.area(), &code, &theme);
    }));
    rows
}

fn distinct_styled_tuples(rows: &[String]) -> std::collections::BTreeSet<String> {
    rows.iter()
        .filter_map(|r| {
            let mut parts = r.split('\t');
            let scene = parts.next()?;
            let (_x, _y, _sym) = (parts.next()?, parts.next()?, parts.next()?);
            let (fg, bg, mods) = (parts.next()?, parts.next()?, parts.next()?);
            if fg == "Reset" && bg == "Reset" && mods == "NONE" {
                return None;
            }
            Some(format!("{scene}|{fg}|{bg}|{mods}"))
        })
        .collect()
}

/// One-shot baseline generator. Run explicitly BEFORE the migration slices:
/// `cargo test -p cyril-ui --test modal_theme -- --ignored generate_baseline`
#[test]
#[ignore = "one-shot pre-migration baseline generator; do not run after migration"]
fn generate_baseline() {
    let rows = all_scene_rows();
    let tuples = distinct_styled_tuples(&rows);
    assert_eq!(
        tuples.len(),
        30,
        "scene set must exercise all 30 frozen legacy tuples, got {}",
        tuples.len()
    );
    std::fs::write(BASELINE, rows.join("\n") + "\n")
        .unwrap_or_else(|e| panic!("write baseline TSV: {e}"));
}

/// C11 fence: the committed baseline exercises the full 30-tuple inventory.
#[test]
fn baseline_covers_inventory() {
    let path = Path::new(BASELINE);
    assert!(
        path.exists(),
        "baseline TSV missing — run generate_baseline"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read baseline TSV: {e}"));
    let rows: Vec<String> = text.lines().map(str::to_owned).collect();
    assert_eq!(
        distinct_styled_tuples(&rows).len(),
        30,
        "committed baseline no longer covers the 30-tuple legacy inventory"
    );
}
