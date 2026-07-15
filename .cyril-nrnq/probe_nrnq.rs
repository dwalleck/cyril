//! cyril-nrnq probe: which style tuples do the four modal overlays emit?
//! Prints `WIDGET|fg=..|bg=..|mods=..` lines (distinct, sorted) for diffing
//! against the static-grep oracle (.cyril-nrnq/oracle-static.txt).
//! Run: cargo test -p cyril-ui --test probe_nrnq -- --nocapture

use std::collections::BTreeSet;

use cyril_core::types::{
    CodePanelData, CommandOption, HookInfo, LspServerInfo, LspStatus, PermissionOption,
    PermissionOptionId, PermissionOptionKind, ToolCall, ToolCallId, ToolCallStatus, ToolKind,
    TrustOption,
};
use cyril_ui::traits::{ApprovalPhase, ApprovalState, HooksPanelState, PickerState};
use cyril_ui::widgets::{approval, code_panel, hooks_panel, picker};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Style;

fn dump(widget: &str, draw: impl Fn(&mut ratatui::Frame)) {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| draw(f)).expect("draw");
    let buffer = terminal.backend().buffer();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for y in 0..24u16 {
        for x in 0..80u16 {
            let cell = &buffer[(x, y)];
            if cell.style() == Style::default() {
                continue;
            }
            seen.insert(format!(
                "{widget}|fg={:?}|bg={:?}|mods={:?}",
                cell.fg, cell.bg, cell.modifier
            ));
        }
    }
    for line in seen {
        println!("{line}");
    }
}

fn approval_state(phase: ApprovalPhase, trust: bool) -> ApprovalState {
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
        trust_options: if trust {
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
        phase,
        responder: tokio::sync::oneshot::channel().0,
    }
}

#[test]
fn probe_modal_styles() {
    let opt_phase = approval_state(ApprovalPhase::SelectOption, false);
    dump("approval-option", |f| {
        approval::render(f, f.area(), &opt_phase);
    });

    let trust_phase = approval_state(
        ApprovalPhase::SelectTrust {
            chosen_option_id: PermissionOptionId::new("allow"),
        },
        true,
    );
    dump("approval-trust", |f| {
        approval::render(f, f.area(), &trust_phase);
    });

    let picker_state = PickerState {
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
    };
    dump("picker", |f| picker::render(f, f.area(), &picker_state));

    let hooks = HooksPanelState {
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
    };
    dump("hooks", |f| hooks_panel::render(f, f.area(), &hooks));

    let code = CodePanelData {
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
    };
    dump("code", |f| code_panel::render(f, f.area(), &code));
}
