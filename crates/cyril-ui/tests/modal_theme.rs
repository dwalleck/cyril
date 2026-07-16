//! cyril-nrnq fences: modal semantic-color migration.
//!
//! Ground truth is `tests/fixtures/modal-baseline.tsv` — a cell dump of the five
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
    "/tests/fixtures/modal-baseline.tsv"
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

fn scene(name: &str, theme: &Theme) -> Vec<String> {
    match name {
        "approval-option" => {
            let st = approval_state(false);
            scene_rows(name, |f| {
                approval::render(f, f.area(), f.area().height, &st, theme)
            })
        }
        "approval-trust" => {
            let st = approval_state(true);
            scene_rows(name, |f| {
                approval::render(f, f.area(), f.area().height, &st, theme)
            })
        }
        "picker" => {
            let st = picker_scene_state();
            scene_rows(name, |f| picker::render(f, f.area(), &st, theme))
        }
        "hooks" => {
            let st = hooks_scene_state();
            scene_rows(name, |f| hooks_panel::render(f, f.area(), &st, theme))
        }
        "code" => {
            let st = code_scene_state();
            scene_rows(name, |f| code_panel::render(f, f.area(), &st, theme))
        }
        other => panic!("unknown scene {other}"),
    }
}

const ALL_SCENES: [&str; 5] = [
    "approval-option",
    "approval-trust",
    "picker",
    "hooks",
    "code",
];

fn all_scene_rows() -> Vec<String> {
    let theme = truecolor_theme();
    ALL_SCENES.iter().flat_map(|s| scene(s, &theme)).collect()
}

/// ghuu-canon normalization (named ANSI -> canonical RGB), Debug-string space.
fn canonical_color(c: &str) -> String {
    match c {
        "Red" => "Rgb(128, 0, 0)".into(),
        "Green" => "Rgb(0, 128, 0)".into(),
        "Yellow" => "Rgb(128, 128, 0)".into(),
        "Cyan" => "Rgb(0, 128, 128)".into(),
        "DarkGray" => "Rgb(128, 128, 128)".into(),
        "Gray" => "Rgb(192, 192, 192)".into(),
        "White" => "Rgb(255, 255, 255)".into(),
        other => other.to_string(),
    }
}

fn normalize_row(row: &str) -> String {
    let parts: Vec<&str> = row.split('\t').collect();
    assert_eq!(parts.len(), 7, "malformed baseline row: {row}");
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        parts[0],
        parts[1],
        parts[2],
        parts[3],
        canonical_color(parts[4]),
        canonical_color(parts[5]),
        parts[6]
    )
}

fn baseline_rows(scene_name: &str) -> Vec<String> {
    let text =
        std::fs::read_to_string(BASELINE).unwrap_or_else(|e| panic!("read baseline TSV: {e}"));
    let prefix = format!("{scene_name}\t");
    text.lines()
        .filter(|l| l.starts_with(&prefix))
        .map(normalize_row)
        .collect()
}

/// C3: the migrated widget renders zero normalized-cell drift vs the frozen
/// pre-migration baseline.
fn assert_scene_equivalent(scene_name: &str) {
    let theme = truecolor_theme();
    let current = scene(scene_name, &theme);
    let baseline = baseline_rows(scene_name);
    assert_eq!(
        current.len(),
        baseline.len(),
        "{scene_name}: cell count drifted"
    );
    for (c, b) in current.iter().zip(&baseline) {
        assert_eq!(c, b, "{scene_name}: normalized cell drift");
    }
}

/// Local mirror of traits::test_support::marker_theme (cfg(test)-private to
/// the lib). Pairwise distinctness is asserted below AND fenced lib-side by
/// theme::tests::marker_theme_roles_are_pairwise_distinct.
fn marker_theme() -> Theme {
    Theme {
        syntax: None,
        canvas: ratatui::style::Color::Indexed(1),
        chrome: ratatui::style::Color::Indexed(2),
        code: ratatui::style::Color::Indexed(3),
        selection: ratatui::style::Color::Indexed(4),
        text: ratatui::style::Color::Indexed(5),
        muted: ratatui::style::Color::Indexed(6),
        border: ratatui::style::Color::Indexed(7),
        accent: ratatui::style::Color::Indexed(8),
        accent_alt: ratatui::style::Color::Indexed(9),
        user: ratatui::style::Color::Indexed(10),
        agent: ratatui::style::Color::Indexed(11),
        system: ratatui::style::Color::Indexed(12),
        info: ratatui::style::Color::Indexed(13),
        success: ratatui::style::Color::Indexed(14),
        warning: ratatui::style::Color::Indexed(15),
        danger: ratatui::style::Color::Indexed(16),
        diff_add: ratatui::style::Color::Indexed(17),
        diff_delete: ratatui::style::Color::Indexed(18),
        diff_context: ratatui::style::Color::Indexed(19),
        emphasis: ratatui::style::Color::Indexed(20),
        accent_tertiary: ratatui::style::Color::Indexed(21),
        accent_quaternary: ratatui::style::Color::Indexed(22),
        accent_quinary: ratatui::style::Color::Indexed(23),
        subdued: ratatui::style::Color::Indexed(24),
        subdued_positive: ratatui::style::Color::Indexed(25),
        subdued_negative: ratatui::style::Color::Indexed(26),
        soft_accent: ratatui::style::Color::Indexed(27),
        positive_accent: ratatui::style::Color::Indexed(28),
        inset_background: ratatui::style::Color::Indexed(29),
        text_secondary: ratatui::style::Color::Indexed(30),
        accent_violet: ratatui::style::Color::Indexed(31),
    }
}

/// Distinct non-Reset fg / bg sets of a scene rendered under the marker
/// theme — the observable footprint of which roles a widget consumes.
fn marker_footprint(scene_name: &str) -> (Vec<String>, Vec<String>) {
    let marker = marker_theme();
    let rows = scene(scene_name, &marker);
    let mut fgs = std::collections::BTreeSet::new();
    let mut bgs = std::collections::BTreeSet::new();
    for row in rows {
        let parts: Vec<&str> = row.split('\t').collect();
        if parts[4] != "Reset" {
            fgs.insert(parts[4].to_string());
        }
        if parts[5] != "Reset" {
            bgs.insert(parts[5].to_string());
        }
    }
    (fgs.into_iter().collect(), bgs.into_iter().collect())
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

#[test]
fn baseline_equivalence_approval_option() {
    assert_scene_equivalent("approval-option");
}

#[test]
fn baseline_equivalence_approval_trust() {
    assert_scene_equivalent("approval-trust");
}

/// C4: approval consumes exactly its mapped roles — option phase: emphasis
/// (20) + text (5) + text_secondary (30) on selection bg (4); trust phase
/// adds accent_quinary (23) and subdued (24).
#[test]
fn marker_wiring_approval() {
    let (fgs, bgs) = marker_footprint("approval-option");
    assert_eq!(
        fgs,
        vec!["Indexed(20)", "Indexed(30)", "Indexed(5)"],
        "approval-option fg roles"
    );
    assert_eq!(bgs, vec!["Indexed(4)"], "approval-option bg roles");

    let (fgs, bgs) = marker_footprint("approval-trust");
    assert_eq!(
        fgs,
        vec!["Indexed(23)", "Indexed(24)", "Indexed(30)", "Indexed(5)"],
        "approval-trust fg roles"
    );
    assert_eq!(bgs, vec!["Indexed(4)"], "approval-trust bg roles");
}

#[test]
fn baseline_equivalence_picker() {
    assert_scene_equivalent("picker");
}

/// C4: picker consumes exactly subdued (24), text (5), text_secondary (30),
/// accent_quinary (23) on selection bg (4) — and, unlike approval, the
/// selected row carries NO BOLD (existing asymmetry preserved; the
/// description's ITALIC survives the swap).
#[test]
fn marker_wiring_picker() {
    let (fgs, bgs) = marker_footprint("picker");
    assert_eq!(
        fgs,
        vec!["Indexed(23)", "Indexed(24)", "Indexed(30)", "Indexed(5)"],
        "picker fg roles"
    );
    assert_eq!(bgs, vec!["Indexed(4)"], "picker bg roles");
    let marker = marker_theme();
    let rows = scene("picker", &marker);
    assert!(
        rows.iter()
            .any(|r| r.ends_with("ITALIC") && r.contains("Indexed(24)")),
        "no subdued+ITALIC description cell — modifier lost in the swap"
    );
}

#[test]
fn baseline_equivalence_hooks() {
    assert_scene_equivalent("hooks");
}

/// C4: hooks consumes accent_quinary (23: title/border/matcher-present),
/// subdued (24: header/matcher-absent), accent_violet (31: trigger — the
/// new role's FIRST consumer), text_secondary (30: command). No backgrounds.
#[test]
fn marker_wiring_hooks() {
    let (fgs, bgs) = marker_footprint("hooks");
    assert_eq!(
        fgs,
        vec!["Indexed(23)", "Indexed(24)", "Indexed(30)", "Indexed(31)"],
        "hooks fg roles"
    );
    assert!(bgs.is_empty(), "hooks should paint no backgrounds: {bgs:?}");
}

/// C10: the empty hooks list renders themed without panicking, consuming
/// only the frame roles (accent_quinary title/border + subdued notice).
#[test]
fn hooks_empty_list_renders_themed() {
    let marker = marker_theme();
    let empty = HooksPanelState {
        hooks: vec![],
        scroll_offset: 0,
    };
    let rows = scene_rows("hooks-empty", |f| {
        hooks_panel::render(f, f.area(), &empty, &marker)
    });
    let fgs: std::collections::BTreeSet<String> = rows
        .iter()
        .map(|r| {
            let (_, _, _, _, fg, _, _) = row_parts(r);
            fg
        })
        .filter(|c| c != "Reset")
        .collect();
    assert_eq!(
        fgs.into_iter().collect::<Vec<_>>(),
        vec!["Indexed(23)", "Indexed(24)"],
        "empty hooks role set"
    );
}

#[test]
fn baseline_equivalence_code() {
    assert_scene_equivalent("code");
}

/// C4: code panel consumes accent_quinary (23: labels/keys/title/border),
/// subdued (24: values, absent-status ○), emphasis (20: warning + ◐),
/// subdued_positive (25: ✓), subdued_negative (26: ✗). No backgrounds.
#[test]
fn marker_wiring_code() {
    let (fgs, bgs) = marker_footprint("code");
    assert_eq!(
        fgs,
        vec![
            "Indexed(20)",
            "Indexed(23)",
            "Indexed(24)",
            "Indexed(25)",
            "Indexed(26)"
        ],
        "code panel fg roles"
    );
    assert!(bgs.is_empty(), "code panel paints no backgrounds: {bgs:?}");
}

/// C10: Unknown status + all-None optionals + empty lsps renders themed
/// without panicking.
#[test]
fn code_edge_shapes_render_themed() {
    let marker = marker_theme();
    let edge = CodePanelData {
        status: LspStatus::Unknown("weird-state".into()),
        message: None,
        warning: None,
        root_path: None,
        detected_languages: vec![],
        project_markers: vec![],
        config_path: None,
        doc_url: None,
        lsps: vec![],
    };
    let rows = scene_rows("code-edge", |f| {
        code_panel::render(f, f.area(), &edge, &marker)
    });
    assert!(!rows.is_empty(), "edge scene rendered nothing");
    let allowed = ["Indexed(20)", "Indexed(23)", "Indexed(24)", "Reset"];
    for row in &rows {
        let fg = row.split('\t').nth(4).unwrap_or("");
        assert!(
            allowed.contains(&fg),
            "unexpected role {fg} in edge scene: {row}"
        );
    }
}

fn row_parts(row: &str) -> (String, u16, u16, String, String, String, String) {
    let p: Vec<&str> = row.split('\t').collect();
    assert_eq!(p.len(), 7, "malformed cell row: {row}");
    let coord = |s: &str| -> u16 {
        s.parse()
            .unwrap_or_else(|e| panic!("malformed coordinate {s:?} in row {row:?}: {e}"))
    };
    (
        p[0].to_string(),
        coord(p[1]),
        coord(p[2]),
        p[3].to_string(),
        p[4].to_string(),
        p[5].to_string(),
        p[6].to_string(),
    )
}

/// C7: under the NoColor projection every modal cell is color-free, and the
/// visible symbols are identical to the truecolor render.
#[test]
fn no_color_scenes_reset() {
    let truecolor = truecolor_theme();
    let no_color = resolve(ThemeId::CyrilDark, ColorMode::None);
    for name in ALL_SCENES {
        let nc_rows = scene(name, &no_color);
        for row in &nc_rows {
            let (_, x, y, _, fg, bg, _) = row_parts(row);
            assert_eq!(fg, "Reset", "{name}: colored fg at ({x},{y})");
            assert_eq!(bg, "Reset", "{name}: colored bg at ({x},{y})");
        }
        let symbols = |rows: &[String]| -> std::collections::BTreeSet<(u16, u16, String)> {
            rows.iter()
                .map(|r| row_parts(r))
                .filter(|(_, _, _, sym, ..)| sym != " ")
                .map(|(_, x, y, sym, ..)| (x, y, sym))
                .collect()
        };
        let tc_rows = scene(name, &truecolor);
        assert_eq!(
            symbols(&nc_rows),
            symbols(&tc_rows),
            "{name}: symbols drifted between color modes"
        );
    }
}

/// C8 (AC2): with every color stripped, the selected row is still
/// identifiable — exactly one ▸ marker, whose row carries the selected
/// option's label (and BOLD in approval).
#[test]
fn no_color_selection_distinguishable() {
    let no_color = resolve(ThemeId::CyrilDark, ColorMode::None);
    for (name, label, expect_bold) in [
        ("approval-option", "Allow Once", true),
        ("picker", "opt-1", false),
    ] {
        let rows = scene(name, &no_color);
        let markers: Vec<_> = rows
            .iter()
            .map(|r| row_parts(r))
            .filter(|(_, _, _, sym, ..)| sym == "▸")
            .collect();
        assert_eq!(markers.len(), 1, "{name}: expected exactly one ▸");
        let marker_y = markers[0].2;
        let mut row_text: Vec<(u16, String)> = rows
            .iter()
            .map(|r| row_parts(r))
            .filter(|(_, _, y, ..)| *y == marker_y)
            .map(|(_, x, _, sym, ..)| (x, sym))
            .collect();
        row_text.sort_by_key(|(x, _)| *x);
        let text: String = row_text.into_iter().map(|(_, s)| s).collect();
        assert!(
            text.contains(label),
            "{name}: selected label not on the ▸ row: {text}"
        );
        if expect_bold {
            assert!(
                rows.iter()
                    .map(|r| row_parts(r))
                    .filter(|(_, _, y, ..)| *y == marker_y)
                    .any(|(.., mods)| mods.contains("BOLD")),
                "{name}: selected row lost its BOLD signal"
            );
        }
    }
}
