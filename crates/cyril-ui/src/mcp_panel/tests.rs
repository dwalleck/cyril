use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cyril_core::types::*;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend, TestBackend},
    buffer::{Buffer, Cell},
    layout::Rect,
};

use crate::state::UiState;
use crate::traits::{ChatMessageKind, Overlay, TuiState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn catalog() -> Arc<McpCatalog> {
    Arc::new(McpCatalog {
        tools: vec![McpTool {
            name: "tool exact / 日".into(),
            description: Some("read-only tool metadata".into()),
            disabled: true,
            input_schema: "{schema data}".into(),
        }],
        prompts: vec![McpPrompt {
            name: "prompt / exact 日".into(),
            title: Some("Prompt title".into()),
            description: Some("External prompt".into()),
            arguments: vec![
                McpPromptArgument {
                    name: "required key".into(),
                    description: None,
                    required: true,
                },
                McpPromptArgument {
                    name: "optional key".into(),
                    description: None,
                    required: false,
                },
            ],
        }],
        resources: vec![McpResource {
            uri: "canary://exact/%20?x=a&b=日".into(),
            name: "Resource exact".into(),
            title: None,
            description: None,
            mime_type: Some("text/plain".into()),
        }],
        resource_templates: vec![McpResourceTemplate {
            uri_template: "canary://{name}".into(),
            name: "Template exact".into(),
            title: None,
            description: None,
            mime_type: Some("application/octet-stream".into()),
        }],
    })
}

fn server(name: &str) -> McpServer {
    McpServer {
        name: name.into(),
        auth_type: None,
        status: McpServerStatus::Connected { catalog: catalog() },
        provenance: None,
    }
}

fn state(servers: Vec<McpServer>) -> Arc<McpState> {
    let mut state = McpState::for_session(SessionId::new("root"));
    state.apply(&McpUpdate::Snapshot(Arc::new(McpSnapshot {
        session_id: SessionId::new("root"),
        servers,
        access_mode: None,
        registry_servers: None,
        access_mode_filtered_servers: None,
        unresolved_registry_servers: None,
        provenance: None,
    })));
    Arc::new(state)
}

fn document(ui: &UiState) -> String {
    ui.mcp_panel()
        .unwrap_or_else(|| panic!("open MCP panel"))
        .document
        .join("\n")
}

fn open() -> UiState {
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(vec![server("server exact / 日"), server("other")]));
    ui
}

fn prompt_form(ui: &mut UiState) {
    ui.handle_mcp_key(key(KeyCode::Tab));
    ui.handle_mcp_key(key(KeyCode::Tab));
    ui.handle_mcp_key(key(KeyCode::Enter));
}

fn render(ui: &mut UiState, width: u16, height: u16) -> Result<String, Box<dyn std::error::Error>> {
    ui.set_terminal_size(width, height);
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| {
        crate::render::draw(frame, ui);
    })?;
    let buffer = terminal.backend().buffer();
    Ok((0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

const PREPEND_ARGUMENT: &str = "\u{0600}\u{0600}Z";

fn diff_prompt_form() -> UiState {
    let mut ui = UiState::new(100);
    ui.set_terminal_size(80, 24);
    ui.show_mcp_panel(state(vec![server("exact")]));
    prompt_form(&mut ui);
    ui
}

fn assert_prompt_argument_bytes(panel: &super::McpPanelState, value: &str, cursor: usize) {
    let super::Page::Arguments(form) = &panel.page else {
        panic!("original argument form");
    };
    assert!(form.fields[0].value.present);
    assert_eq!(form.fields[0].value.text.as_bytes(), value.as_bytes());
    assert_eq!(form.fields[0].value.cursor, cursor);
    assert!(!form.fields[1].value.present);
    let mut submitted = panel.clone();
    let Some(super::PanelAction::Request(request)) = submitted.key(key(KeyCode::Enter)) else {
        panic!("cloned form must dispatch the real prompt request");
    };
    assert_eq!(request.session_id, SessionId::new("root"));
    assert_eq!(request.server_name, "exact");
    let McpOperation::GetPrompt { name, arguments } = request.operation else {
        panic!("getPrompt request");
    };
    assert_eq!(name, "prompt / exact 日");
    assert_eq!(arguments["required key"].as_bytes(), value.as_bytes());
    assert!(!arguments.contains_key("optional key"));
}

fn mcp_ui_producer_snapshot(ui: &UiState) -> Result<Buffer, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
    let completed = terminal.draw(|frame| crate::render::draw(frame, ui))?;
    // TestBackend only records diff assignments; it cannot model a continuation
    // SPACE destroying the terminal's wide glyph. Keep the actual producer.
    Ok(completed.buffer.clone())
}

fn mcp_panel_producer_snapshot(
    panel: &mut super::McpPanelState,
    area: Rect,
    input_top: u16,
    theme: &crate::theme::Theme,
) -> Result<Buffer, Box<dyn std::error::Error>> {
    let placed = crate::widgets::mcp_panel::placement(area, input_top)
        .unwrap_or_else(|| panic!("available argument form geometry"));
    panel.geometry(
        usize::from(placed.body.width),
        usize::from(placed.body.height),
    );
    // Placement moves inside one fixed canvas; Buffer::diff sees equal areas.
    let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
    let completed = terminal.draw(|frame| {
        crate::widgets::mcp_panel::render(frame, area, input_top, panel, theme);
    })?;
    Ok(completed.buffer.clone())
}

struct McpConsumerDiff<'a> {
    updates: Vec<(u16, u16, &'a Cell)>,
    ansi: String,
}

fn mcp_consumer_diff<'a>(
    previous: &Buffer,
    next: &'a Buffer,
) -> Result<McpConsumerDiff<'a>, Box<dyn std::error::Error>> {
    assert_eq!(previous.area, next.area);
    let updates = previous.diff(next);
    let mut bytes = Vec::new();
    {
        let mut backend = CrosstermBackend::new(&mut bytes);
        // Serialize every real diff update before making any rendering verdict.
        backend.draw(updates.iter().copied())?;
    }
    Ok(McpConsumerDiff {
        updates,
        ansi: String::from_utf8(bytes)?,
    })
}

fn painted_symbol(buffer: &Buffer, symbol: &str) -> (u16, u16) {
    for y in buffer.area.y..buffer.area.bottom() {
        for x in buffer.area.x..buffer.area.right() {
            if buffer[(x, y)].symbol() == symbol {
                return (x, y);
            }
        }
    }
    panic!("actual producer must contain the complete symbol {symbol:?}");
}

fn assert_diff_cell(diff: &McpConsumerDiff<'_>, next: &Buffer, position: (u16, u16)) {
    let (_, _, cell) = diff
        .updates
        .iter()
        .find(|(x, y, _)| (*x, *y) == position)
        .unwrap_or_else(|| panic!("real diff must repaint {position:?}"));
    assert_eq!(*cell, &next[position], "replacement content and style");
}

fn assert_current_wide_diff(
    previous: &Buffer,
    next: &Buffer,
    diff: &McpConsumerDiff<'_>,
    symbol: &str,
    width: u16,
) -> (u16, u16) {
    let (x, y) = painted_symbol(next, symbol);
    if previous[(x, y)] != next[(x, y)] {
        assert_diff_cell(diff, next, (x, y));
        assert!(
            diff.ansi.contains(symbol),
            "backend must print the full lead"
        );
    }
    for continuation in x + 1..x + width {
        assert!(
            !diff
                .updates
                .iter()
                .any(|(column, row, _)| (*column, *row) == (continuation, y)),
            "no operation may overwrite a CURRENT wide continuation at ({continuation}, {y})"
        );
        // Position-specific: other panel rows legitimately print spaces. A
        // cursor move here is destructive even with SGR commands before SPACE.
        assert!(
            !diff
                .ansi
                .contains(&format!("\u{1b}[{};{}H", y + 1, continuation + 1)),
            "backend must not address a current continuation column"
        );
    }
    (x, y)
}

fn assert_caret_diff(previous: &Buffer, next: &Buffer, diff: &McpConsumerDiff<'_>) -> (u16, u16) {
    let position = painted_symbol(next, "▏");
    if previous[position] != next[position] {
        assert_diff_cell(diff, next, position);
        assert!(
            diff.ansi.contains('▏'),
            "backend must print the independent caret"
        );
    }
    position
}

fn assert_former_columns_repainted(next: &Buffer, diff: &McpConsumerDiff<'_>, former: Rect) {
    use unicode_width::UnicodeWidthStr;

    let mut uncovered = 0;
    for y in former.y..former.bottom() {
        for x in former.x..former.right() {
            // Inspect ownership in the actual NEXT producer, not the old span:
            // a moved wide lead can legitimately cover a formerly used column.
            let covered = (next.area.x..x)
                .any(|lead| usize::from(lead) + next[(lead, y)].symbol().width() > usize::from(x));
            if !covered {
                uncovered += 1;
                assert_diff_cell(diff, next, (x, y));
            }
        }
    }
    assert!(uncovered > 0, "exercise genuinely uncovered former columns");
}

#[test]
fn pushes_never_open_and_unknown_is_not_known_empty() {
    let mut ui = UiState::new(100);
    assert!(!ui.refresh_mcp_panel(state(Vec::new())));
    assert!(ui.mcp_panel().is_none());
    ui.show_mcp_panel(Arc::new(McpState::for_session(SessionId::new("root"))));
    assert!(document(&ui).contains("not reported yet"));
    assert!(ui.refresh_mcp_panel(state(Vec::new())));
    assert!(document(&ui).contains("Known empty inventory"));
    assert!(!document(&ui).contains("Access mode: nonRegistry"));
    ui.hide_mcp_panel();
    assert!(ui.mcp_panel().is_none());
}

#[test]
fn governance_optional_registry_and_provenance_remain_distinct() {
    let mut data = (*state(Vec::new())).clone();
    data.policy = Some(McpPolicy::AdminDisabled);
    data.diagnostic = Some("malformed status".into());
    let snapshot = Arc::make_mut(data.snapshot.as_mut().unwrap_or_else(|| panic!("snapshot")));
    snapshot.access_mode = Some(McpAccessMode::Registry);
    snapshot.access_mode_filtered_servers = Some(vec!["filtered exact".into()]);
    snapshot.unresolved_registry_servers = Some(vec!["unresolved exact".into()]);
    snapshot.registry_servers = Some(vec![McpRegistryServer {
        name: "catalog exact".into(),
        version: Some("1.2".into()),
        description: Some("Registry description".into()),
        enabled: false,
    }]);
    snapshot.provenance = Some(McpProvenance {
        resource_type: Some("mcp".into()),
        origin: Some("organization".into()),
        krn: Some("krn:exact".into()),
        layer: Some("policy".into()),
        details: "provenance future data".into(),
    });
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::new(data));
    let text = document(&ui);
    for expected in [
        "disabled by your administrator",
        "malformed status",
        "may be stale",
        "filtered exact",
        "excluded by policy",
        "unresolved exact",
        "catalog exact",
        "not enabled in pool",
        "Registry description",
        "organization",
        "krn:exact",
        "provenance future data",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
}

#[test]
fn all_statuses_and_exact_catalog_are_visible() {
    let failed = McpServer {
        name: "auth".into(),
        auth_type: Some("oauth".into()),
        status: McpServerStatus::Failed {
            failed_authorization: true,
            authorization_url: Some("https://auth.invalid/exact?state=a%2Fb".into()),
            error_message: "Sign in needed".into(),
            error_code: Some("AUTH".into()),
        },
        provenance: None,
    };
    let mut servers = vec![failed];
    servers.extend(
        [
            McpServerStatus::Connecting,
            McpServerStatus::Disabled,
            McpServerStatus::Unknown {
                status: "future-state".into(),
            },
            McpServerStatus::Failed {
                failed_authorization: false,
                authorization_url: None,
                error_message: "transport".into(),
                error_code: None,
            },
        ]
        .into_iter()
        .enumerate()
        .map(|(index, status)| McpServer {
            name: format!("server-{index}"),
            status,
            auth_type: None,
            provenance: None,
        }),
    );
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(servers));
    let text = document(&ui);
    for expected in [
        "authentication required",
        "connecting",
        "disabled",
        "future-state",
        "transport failed",
        "https://auth.invalid/exact?state=a%2Fb",
        "Sign in needed",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    let mut ui = open();
    for expected in [
        "tool exact / 日",
        "prompt / exact 日",
        "canary://exact/%20?x=a&b=日",
        "canary://{name}",
    ] {
        ui.handle_mcp_key(key(KeyCode::Tab));
        assert!(document(&ui).contains(expected));
    }
    assert!(ui.messages().is_empty());
}

#[test]
fn removed_selection_clamps_without_dispatch_and_editor_keeps_original_target() {
    let mut ui = open();
    prompt_form(&mut ui);
    assert!(ui.mcp_argument_paste(" original value "));
    ui.refresh_mcp_panel(state(vec![server("other")]));
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("pinned request"));
    assert_eq!(request.session_id, SessionId::new("root"));
    assert_eq!(request.server_name, "server exact / 日");
    ui.handle_mcp_key(key(KeyCode::Esc));
    ui.handle_mcp_key(key(KeyCode::Esc));
    ui.show_mcp_panel(state(vec![server("server exact / 日"), server("other")]));
    ui.refresh_mcp_panel(state(vec![server("other")]));
    assert!(document(&ui).contains("> other"));
    assert!(ui.messages().is_empty());
}

#[test]
fn prompt_strings_preserve_presence_empty_whitespace_and_large_pastes() {
    let mut ui = open();
    prompt_form(&mut ui);
    assert!(ui.handle_mcp_key(key(KeyCode::Enter)).is_none());
    assert!(document(&ui).contains("Required argument"));
    ui.handle_mcp_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    ui.handle_mcp_key(key(KeyCode::Tab));
    let value = format!(" \t\r\n{}  ", "日".repeat(5_000));
    assert!(ui.mcp_argument_paste(&value));
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("read request"));
    let McpOperation::GetPrompt { name, arguments } = request.operation else {
        panic!("getPrompt only")
    };
    assert_eq!(name, "prompt / exact 日");
    assert_eq!(arguments.get("required key"), Some(&String::new()));
    assert_eq!(arguments.get("optional key"), Some(&value));
    assert!(ui.input_text().is_empty());
    assert!(ui.messages().is_empty());

    let mut ui = open();
    prompt_form(&mut ui);
    assert!(ui.mcp_argument_paste("   "));
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("whitespace is present"));
    let McpOperation::GetPrompt { arguments, .. } = request.operation else {
        panic!("prompt")
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments["required key"], "   ");
    assert!(!arguments.contains_key("optional key"));
}

#[test]
fn reset_and_authenticate_are_distinct_and_ack_does_not_connect_or_clear_url() {
    let failed = McpServer {
        name: "auth".into(),
        auth_type: None,
        status: McpServerStatus::Failed {
            failed_authorization: true,
            authorization_url: Some("https://auth.invalid/manual".into()),
            error_message: "needs auth".into(),
            error_code: None,
        },
        provenance: None,
    };
    let initial = state(vec![failed]);
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::clone(&initial));
    let reset = ui
        .handle_mcp_key(key(KeyCode::Char('r')))
        .unwrap_or_else(|| panic!("reset"));
    assert_eq!(reset.operation, McpOperation::Reset { start_oauth: false });
    assert!(ui.handle_mcp_key(key(KeyCode::Char('a'))).is_none());
    let mut data = (*initial).clone();
    data.apply(&McpUpdate::Pending(McpPending {
        id: 1,
        request: reset.clone(),
    }));
    ui.refresh_mcp_panel(Arc::new(data.clone()));
    ui.hide_mcp_panel();
    ui.show_mcp_panel(Arc::new(data.clone()));
    assert!(document(&ui).contains("PENDING"));
    assert!(ui.handle_mcp_key(key(KeyCode::Char('r'))).is_none());
    data.apply(&McpUpdate::Outcome(McpOutcome {
        id: 1,
        request: reset,
        result: Ok(McpOperationResult::Acknowledged),
    }));
    ui.refresh_mcp_panel(Arc::new(data));
    assert!(document(&ui).contains("https://auth.invalid/manual"));
    let auth = ui
        .handle_mcp_key(key(KeyCode::Char('a')))
        .unwrap_or_else(|| panic!("authenticate"));
    assert_eq!(auth.operation, McpOperation::Reset { start_oauth: true });
    ui.refresh_mcp_panel(state(vec![server("auth")]));
    assert!(!document(&ui).contains("https://auth.invalid/manual"));
    ui.refresh_mcp_panel(state(Vec::new()));
    assert!(!document(&ui).contains("https://auth.invalid/manual"));
}

#[test]
fn read_preview_is_full_scrollable_literal_external_data_with_nontext_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let initial = state(vec![server("exact")]);
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::clone(&initial));
    for _ in 0..3 {
        ui.handle_mcp_key(key(KeyCode::Tab));
    }
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("resource read"));
    assert_eq!(
        request.operation,
        McpOperation::GetResource {
            uri: "canary://exact/%20?x=a&b=日".into()
        }
    );
    assert!(!ui.mcp_argument_paste("must not paste into preview"));
    let mut data = (*initial).clone();
    data.apply(&McpUpdate::Pending(McpPending {
        id: 9,
        request: request.clone(),
    }));
    data.apply(&McpUpdate::Outcome(McpOutcome {
        id: 9,
        request,
        result: Ok(McpOperationResult::Preview(Arc::new(
            McpPreview::Resource {
                contents: vec![
                    McpContent::Text {
                        text: format!("FIRST\n{}\nLAST\u{1b}[2J", "external text\n".repeat(200)),
                        mime_type: Some("text/plain".into()),
                        uri: Some("canary://returned".into()),
                    },
                    McpContent::NonText {
                        kind: "image".into(),
                        mime_type: Some("image/png".into()),
                        uri: Some("canary://image".into()),
                        name: Some("image exact".into()),
                        description: Some("not executed".into()),
                    },
                ],
            },
        ))),
    }));
    ui.refresh_mcp_panel(Arc::new(data));
    let text = document(&ui);
    for expected in [
        "READ-ONLY EXTERNAL DATA",
        "FIRST",
        "LAST",
        "image/png",
        "canary://image",
        "image exact",
        r"\u{1b}[2J",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    ui.set_terminal_size(80, 20);
    for _ in 0..300 {
        ui.handle_mcp_key(key(KeyCode::Down));
    }
    let screen = render(&mut ui, 80, 20)?;
    assert!(screen.contains("image/png") || screen.contains("not executed"));
    assert!(ui.messages().is_empty());
    assert!(ui.input_text().is_empty());
    Ok(())
}

#[test]
fn grapheme_width_preview_tail_reaches_testbackend_after_scrolling()
-> Result<(), Box<dyn std::error::Error>> {
    use ratatui::layout::Rect;

    for (terminal_width, body_width, tail) in [(8, 2, "#\u{fe0f}Z"), (10, 4, "#\u{fe0f}#\u{fe0f}Z")]
    {
        let initial = state(vec![server("exact")]);
        let mut panel = super::McpPanelState::new(Arc::clone(&initial));
        for _ in 0..3 {
            assert!(panel.key(key(KeyCode::Tab)).is_none());
        }
        let Some(super::PanelAction::Request(request)) = panel.key(key(KeyCode::Enter)) else {
            panic!("advertised resource read");
        };
        let mut data = (*initial).clone();
        data.apply(&McpUpdate::Outcome(McpOutcome {
            id: 1,
            request,
            result: Ok(McpOperationResult::Preview(Arc::new(
                McpPreview::Resource {
                    contents: vec![McpContent::Text {
                        text: format!("{}\n{tail}", "external data\n".repeat(20)),
                        mime_type: None,
                        uri: None,
                    }],
                },
            ))),
        }));
        assert!(panel.refresh(Arc::new(data)));
        let area = Rect::new(0, 0, terminal_width, 12);
        let placed = crate::widgets::mcp_panel::placement(area, 10)
            .unwrap_or_else(|| panic!("available preview geometry"));
        assert_eq!(placed.body.width, body_width);
        panel.geometry(
            usize::from(placed.body.width),
            usize::from(placed.body.height),
        );
        assert!(panel.document.len() > panel.window);
        let mut terminal = Terminal::new(TestBackend::new(terminal_width, 12))?;
        let theme = crate::theme::resolve_no_color(crate::theme::ThemeId::CyrilDark);
        terminal.draw(|frame| {
            crate::widgets::mcp_panel::render(frame, area, 10, &panel, &theme);
        })?;
        assert!(
            !(placed.body.y..placed.body.bottom()).any(|y| {
                (placed.body.x..placed.body.right())
                    .any(|x| terminal.backend().buffer()[(x, y)].symbol() == "Z")
            }),
            "the preview tail must initially be below the viewport"
        );
        for _ in 0..panel.document.len() {
            assert!(panel.key(key(KeyCode::PageDown)).is_none());
        }
        assert_eq!(
            panel.scroll,
            panel.document.len().saturating_sub(panel.window.max(1)),
            "the real page-scroll path must reach the final viewport"
        );
        terminal.draw(|frame| {
            crate::widgets::mcp_panel::render(frame, area, 10, &panel, &theme);
        })?;
        assert!(
            (placed.body.y..placed.body.bottom()).any(|y| {
                (placed.body.x..placed.body.right())
                    .any(|x| terminal.backend().buffer()[(x, y)].symbol() == "Z")
            }),
            "the full read-only preview tail must be physically painted at width {body_width}"
        );
    }
    Ok(())
}

#[test]
fn operation_errors_survive_reopen_and_dispatch_failure_is_visible() {
    let initial = state(vec![server("exact")]);
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::clone(&initial));
    let request = ui
        .handle_mcp_key(key(KeyCode::Char('r')))
        .unwrap_or_else(|| panic!("reset"));
    ui.mcp_dispatch_failed("channel full".into());
    assert!(document(&ui).contains("channel full"));
    let mut data = (*initial).clone();
    data.apply(&McpUpdate::Outcome(McpOutcome {
        id: 1,
        request,
        result: Err(McpOperationError {
            kind: McpErrorKind::Transport,
            message: "endpoint refused exact".into(),
        }),
    }));
    ui.hide_mcp_panel();
    ui.show_mcp_panel(Arc::new(data));
    assert!(document(&ui).contains("Transport"));
    assert!(document(&ui).contains("endpoint refused exact"));
}

#[test]
fn focused_selection_and_argument_cursor_stay_in_actual_small_viewport()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(
        (0..20)
            .map(|index| server(&format!("server-{index:02}-{}", "日".repeat(30))))
            .collect(),
    ));
    ui.set_terminal_size(40, 12);
    for _ in 0..19 {
        ui.handle_mcp_key(key(KeyCode::Down));
    }
    let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
    let placed = crate::render::mcp_geometry(&ui).unwrap_or_else(|| panic!("geometry"));
    assert!(panel.focus_line >= panel.scroll);
    assert!(panel.focus_line < panel.scroll + usize::from(placed.body.height));
    let screen = render(&mut ui, 40, 12)?;
    assert!(screen.contains("server-19"));
    prompt_form(&mut ui);
    assert!(ui.mcp_argument_paste(&"x".repeat(500)));
    let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
    assert!(panel.focus_line >= panel.scroll);
    assert!(panel.focus_line < panel.scroll + usize::from(placed.body.height));
    Ok(())
}

#[test]
fn wrapped_cjk_cursor_stays_visible_through_editing_and_odd_width_resizes()
-> Result<(), Box<dyn std::error::Error>> {
    fn assert_cursor_visible(
        ui: &mut UiState,
        width: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let screen = render(ui, width, 12)?;
        let placed = crate::render::mcp_geometry(ui).unwrap_or_else(|| panic!("geometry"));
        assert_eq!(placed.body.width % 2, 1, "exercise unsplittable wide cells");
        let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
        // This input contains no cursor glyph; inspect the actual wrapped row,
        // rather than trusting the focus calculation under test.
        let cursor = panel
            .document
            .iter()
            .position(|row| row.contains('▏'))
            .unwrap_or_else(|| panic!("rendered argument cursor"));
        assert_eq!(
            panel.focus_line, cursor,
            "focus must follow actual wrapping"
        );
        assert!(cursor >= panel.scroll);
        assert!(cursor < panel.scroll + usize::from(placed.body.height));
        assert!(screen.contains('▏'), "cursor must reach the drawn viewport");
        Ok(())
    }

    let mut ui = open();
    prompt_form(&mut ui);
    let value = format!(
        "{}\n\t\r\u{1b}[2J e\u{301}{}",
        "日".repeat(1_000),
        "日".repeat(100)
    );
    assert!(ui.mcp_argument_paste(&value));
    for width in [41, 27, 15, 41] {
        assert_cursor_visible(&mut ui, width)?;
        for code in [
            KeyCode::Left,
            KeyCode::Backspace,
            KeyCode::Char('日'),
            KeyCode::Home,
            KeyCode::Right,
            KeyCode::End,
        ] {
            assert!(ui.handle_mcp_key(key(code)).is_none());
            assert_cursor_visible(&mut ui, width)?;
        }
    }
    assert!(ui.messages().is_empty());
    assert!(ui.input_text().is_empty());
    Ok(())
}

fn assert_semantic_caret_visible_and_argument_bytes_preserved(
    value: &str,
    movement: &[KeyCode],
) -> Result<(), Box<dyn std::error::Error>> {
    use ratatui::layout::Rect;

    let mut panel = super::McpPanelState::new(state(vec![server("exact")]));
    for _ in 0..2 {
        assert!(panel.key(key(KeyCode::Tab)).is_none());
    }
    assert!(panel.key(key(KeyCode::Enter)).is_none());
    assert!(panel.paste(value));
    let area = Rect::new(0, 0, 8, 12);
    let placed = crate::widgets::mcp_panel::placement(area, 10)
        .unwrap_or_else(|| panic!("available argument form geometry"));
    assert_eq!(placed.body.width, 2);
    panel.geometry(
        usize::from(placed.body.width),
        usize::from(placed.body.height),
    );
    for code in movement {
        assert!(panel.key(key(*code)).is_none());
    }
    assert!(panel.focus_line >= panel.scroll);
    assert!(panel.focus_line < panel.scroll + usize::from(placed.body.height));
    let caret_y = placed.body.y + u16::try_from(panel.focus_line - panel.scroll)?;
    let mut terminal = Terminal::new(TestBackend::new(8, 12))?;
    let theme = crate::theme::resolve_no_color(crate::theme::ThemeId::CyrilDark);
    terminal.draw(|frame| {
        crate::widgets::mcp_panel::render(frame, area, 10, &panel, &theme);
    })?;
    // The value contains no caret glyph. Inspect physical cells rather than
    // accepting a focus row whose oversized user-data cluster was not painted.
    let caret_visible = (placed.body.x..placed.body.right())
        .any(|x| terminal.backend().buffer()[(x, caret_y)].symbol() == "▏");
    let Some(super::PanelAction::Request(request)) = panel.key(key(KeyCode::Enter)) else {
        panic!("explicit prompt fetch");
    };
    assert_eq!(request.session_id, SessionId::new("root"));
    assert_eq!(request.server_name, "exact");
    let McpOperation::GetPrompt { name, arguments } = request.operation else {
        panic!("getPrompt request");
    };
    assert_eq!(name, "prompt / exact 日");
    assert_eq!(arguments["required key"].as_bytes(), value.as_bytes());
    assert!(!arguments.contains_key("optional key"));
    assert!(
        caret_visible,
        "the semantic caret must be independently painted at the original byte boundary"
    );
    Ok(())
}

#[test]
fn semantic_caret_is_painted_before_spacing_marks_without_changing_argument_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_semantic_caret_visible_and_argument_bytes_preserved(
        "\u{09cb}\u{09cb}Z",
        &[KeyCode::Home],
    )
}

#[test]
fn semantic_caret_is_painted_after_prepend_without_changing_argument_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_semantic_caret_visible_and_argument_bytes_preserved(
        "\u{0600}\u{0600}Z",
        &[KeyCode::Home, KeyCode::Right, KeyCode::Right],
    )
}

#[test]
fn semantic_caret_visual_snap_preserves_composed_data_and_logical_byte_cursor()
-> Result<(), Box<dyn std::error::Error>> {
    use ratatui::layout::Rect;

    fn argument_row(
        movement: &[KeyCode],
        expected_cursor: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let value = "e\u{301}Z";
        let mut panel = super::McpPanelState::new(state(vec![server("exact")]));
        for _ in 0..2 {
            assert!(panel.key(key(KeyCode::Tab)).is_none());
        }
        assert!(panel.key(key(KeyCode::Enter)).is_none());
        assert!(panel.paste(value));
        let area = Rect::new(0, 0, 10, 12);
        let placed = crate::widgets::mcp_panel::placement(area, 10)
            .unwrap_or_else(|| panic!("available argument form geometry"));
        assert_eq!(placed.body.width, 4);
        panel.geometry(
            usize::from(placed.body.width),
            usize::from(placed.body.height),
        );
        for code in movement {
            assert!(panel.key(key(*code)).is_none());
        }
        let super::Page::Arguments(form) = &panel.page else {
            panic!("original argument form");
        };
        assert_eq!(form.fields[form.focused].value.cursor, expected_cursor);
        assert_eq!(
            form.fields[form.focused].value.text.as_bytes(),
            value.as_bytes()
        );
        assert!(panel.focus_line >= panel.scroll);
        assert!(panel.focus_line < panel.scroll + usize::from(placed.body.height));
        let caret_y = placed.body.y + u16::try_from(panel.focus_line - panel.scroll)?;
        let mut terminal = Terminal::new(TestBackend::new(10, 12))?;
        let theme = crate::theme::resolve_no_color(crate::theme::ThemeId::CyrilDark);
        terminal.draw(|frame| {
            crate::widgets::mcp_panel::render(frame, area, 10, &panel, &theme);
        })?;
        let row = (placed.body.x..placed.body.right())
            .map(|x| {
                terminal.backend().buffer()[(x, caret_y)]
                    .symbol()
                    .to_owned()
            })
            .collect();
        let Some(super::PanelAction::Request(request)) = panel.key(key(KeyCode::Enter)) else {
            panic!("explicit prompt fetch");
        };
        assert_eq!(request.session_id, SessionId::new("root"));
        assert_eq!(request.server_name, "exact");
        let McpOperation::GetPrompt { name, arguments } = request.operation else {
            panic!("getPrompt request");
        };
        assert_eq!(name, "prompt / exact 日");
        assert_eq!(arguments["required key"].as_bytes(), value.as_bytes());
        assert!(!arguments.contains_key("optional key"));
        Ok(row)
    }

    fn positions(row: &[String]) -> (usize, usize, usize) {
        let caret = row
            .iter()
            .position(|symbol| symbol == "▏")
            .unwrap_or_else(|| panic!("independently painted semantic caret: {row:?}"));
        let composed = row
            .iter()
            .position(|symbol| symbol == "e\u{301}")
            .unwrap_or_else(|| panic!("composed data grapheme must remain painted: {row:?}"));
        let tail = row
            .iter()
            .position(|symbol| symbol == "Z")
            .unwrap_or_else(|| panic!("original data tail: {row:?}"));
        (caret, composed, tail)
    }

    // Check every logical cursor and exact outgoing value before judging any
    // pixels. Boundary controls must remain distinct from the inside case.
    let home = argument_row(&[KeyCode::Home], 0)?;
    let after = argument_row(&[KeyCode::Home, KeyCode::Right, KeyCode::Right], 3)?;
    let end = argument_row(&[KeyCode::End], 4)?;
    let inside = argument_row(&[KeyCode::Home, KeyCode::Right], 1)?;
    let (caret, composed, tail) = positions(&home);
    assert!(caret < composed && composed < tail);
    let (caret, composed, tail) = positions(&after);
    assert!(composed < caret && caret < tail);
    let (caret, composed, tail) = positions(&end);
    assert!(composed < tail && tail < caret);
    let (caret, composed, tail) = positions(&inside);
    assert!(composed < caret && caret < tail);
    assert_eq!(
        inside, after,
        "only the visual caret snaps after its data grapheme"
    );
    Ok(())
}

fn sound_mark_argument_producer(
    original: &super::McpPanelState,
    value: &str,
    movement: &[KeyCode],
    cursor: usize,
    area: Rect,
    theme: &crate::theme::Theme,
) -> Result<(super::McpPanelState, Buffer), Box<dyn std::error::Error>> {
    let mut panel = original.clone();
    let placed = crate::widgets::mcp_panel::placement(area, 10)
        .unwrap_or_else(|| panic!("available argument form geometry"));
    // Establish the real viewport at the pasted End boundary first. Moving
    // Home then retains a visible row for the following complete data.
    panel.geometry(
        usize::from(placed.body.width),
        usize::from(placed.body.height),
    );
    for code in movement {
        assert!(panel.key(key(*code)).is_none());
    }
    // Check presence, the logical byte cursor and a real cloned Enter request
    // before inspecting any rendering, including when the cursor is internal.
    assert_prompt_argument_bytes(&panel, value, cursor);
    let buffer = mcp_panel_producer_snapshot(&mut panel, area, 10, theme)?;
    assert_prompt_argument_bytes(&panel, value, cursor);
    Ok((panel, buffer))
}

fn assert_sound_mark_argument_width_contract(
    value: &str,
    expected_width: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    // The release fixes only halfwidth U+FF9E/U+FF9F; the attached ordinary
    // U+3099/U+309A controls must not receive that extra cell:
    // https://github.com/ratatui/ratatui/blob/e665c36cb14752a61cd777fbd06dbef8474f2add/ratatui-core/src/buffer/cell_width.rs
    // Each fixture is one DATA grapheme with a hardcoded one/two-cell width.
    // Standalone halfwidth marks have three bytes; attached pairs have six.
    assert!(matches!(value.len(), 3 | 6));
    let mut ui = diff_prompt_form();
    let empty = ui
        .mcp_panel()
        .unwrap_or_else(|| panic!("empty argument form"))
        .clone();
    assert!(ui.mcp_argument_paste(value));
    let original = ui.mcp_panel().unwrap_or_else(|| panic!("argument form"));
    let theme = ui.theme();
    let label_attached_symbol = (value.len() == 3).then(|| format!(" {value}"));

    // Expected (data column, caret column, caret row minus data row), relative
    // to the real body. The existing field label leaves one occupied column
    // at these widths; it is not an empty two/three/four-cell row.
    let placements = match expected_width {
        1 => [
            (2, (0, 1, -1), (1, 0, 1)),
            (3, (2, 1, 0), (1, 2, 0)),
            (4, (2, 1, 0), (1, 2, 0)),
        ],
        2 => [
            (2, (0, 1, -1), (0, 0, 1)),
            (3, (0, 1, -1), (1, 0, 1)),
            (4, (2, 1, 0), (1, 3, 0)),
        ],
        _ => panic!("bounded one/two-cell sound-mark fixtures"),
    };
    for (body_width, home_coordinates, after_coordinates) in placements {
        let area = Rect::new(0, 0, body_width + 6, 12);
        let placed = crate::widgets::mcp_panel::placement(area, 10)
            .unwrap_or_else(|| panic!("available argument form geometry"));
        assert_eq!(placed.body.width, body_width);
        assert!(placed.body.height >= 4);
        let mut empty_panel = empty.clone();
        let previous = mcp_panel_producer_snapshot(&mut empty_panel, area, 10, &theme)?;
        let (home_panel, home) =
            sound_mark_argument_producer(original, value, &[KeyCode::Home], 0, area, &theme)?;
        let (right_panel, right) = sound_mark_argument_producer(
            original,
            value,
            &[KeyCode::Home, KeyCode::Right],
            3,
            area,
            &theme,
        )?;
        let (end_panel, end) = sound_mark_argument_producer(
            original,
            value,
            &[KeyCode::End],
            value.len(),
            area,
            &theme,
        )?;
        // Serialize every real Buffer::diff update through Crossterm before
        // judging any producer cells or current continuation ownership.
        let home_diff = mcp_consumer_diff(&previous, &home)?;
        let right_diff = mcp_consumer_diff(&previous, &right)?;
        let end_diff = mcp_consumer_diff(&previous, &end)?;

        // Every source/canonical-request and complete consumer serialization
        // above precedes the physical verdicts. The actual producer buffers
        // come from CompletedFrame, never TestBackend's delivered diff buffer.
        for (panel, buffer, diff, expected_coordinates, after_data) in [
            (&home_panel, &home, &home_diff, home_coordinates, false),
            (&right_panel, &right, &right_diff, after_coordinates, true),
            (&end_panel, &end, &end_diff, after_coordinates, true),
        ] {
            let caret = panel
                .caret
                .as_ref()
                .unwrap_or_else(|| panic!("semantic argument caret"));
            assert_eq!(caret.row, panel.focus_line);
            assert!(panel.focus_line >= panel.scroll);
            assert!(panel.focus_line < panel.scroll + usize::from(placed.body.height));
            assert!(caret.column < usize::from(placed.body.width));
            assert_eq!(&panel.document[caret.row][caret.bytes.clone()], "▏");

            // At End, a bare Extend mark shares the existing label's final
            // SPACE grapheme. Inspect that actual two-cell producer lead,
            // without adding a separator to the argument or changing paint.
            // Home paints the bare mark through the independent caret suffix.
            let (symbol, painted_width, data_offset) =
                match (label_attached_symbol.as_deref(), after_data) {
                    (Some(symbol), true) => (symbol, 2, 1),
                    _ => (value, expected_width, 0),
                };
            let lead_position =
                assert_current_wide_diff(&previous, buffer, diff, symbol, painted_width);
            let data_position = (lead_position.0 + data_offset, lead_position.1);
            let caret_position = assert_caret_diff(&previous, buffer, diff);
            assert!(lead_position.0 >= placed.body.x);
            assert!(data_position.0 + expected_width <= placed.body.right());
            assert!((placed.body.y..placed.body.bottom()).contains(&data_position.1));
            for continuation in 1..painted_width {
                assert_eq!(
                    buffer[(lead_position.0 + continuation, lead_position.1)].symbol(),
                    " ",
                    "the complete data retains each physical continuation cell"
                );
            }
            assert_eq!(
                caret_position,
                (
                    placed.body.x + u16::try_from(caret.column)?,
                    placed.body.y + u16::try_from(panel.focus_line - panel.scroll)?,
                ),
                "real focus and independently painted caret must agree"
            );
            assert!(
                caret_position.1 != data_position.1
                    || caret_position.0 < data_position.0
                    || caret_position.0 >= data_position.0 + expected_width,
                "the semantic caret cannot overwrite any data cell"
            );
            assert_eq!(buffer[caret_position].bg, theme.selection);
            assert_eq!(
                (
                    data_position.0 - placed.body.x,
                    caret_position.0 - placed.body.x,
                    i32::from(caret_position.1) - i32::from(data_position.1),
                ),
                expected_coordinates,
                "complete data and caret placement for {value:?}, body width {body_width}"
            );
        }
        assert_eq!(
            right,
            end,
            "Home→Right at byte 3 and End at byte {} share the complete data boundary",
            value.len()
        );
        assert_ne!(
            home, end,
            "logical byte 0 must retain the distinct boundary before the data"
        );
    }
    Ok(())
}

#[test]
fn halfwidth_dakuten_argument_preserves_data_caret_and_canonical_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff76}\u{ff9e}", 2)
}

#[test]
fn halfwidth_handakuten_argument_preserves_data_caret_and_canonical_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff8a}\u{ff9f}", 2)
}

#[test]
fn ordinary_combining_dakuten_argument_preserves_data_caret_and_canonical_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{304b}\u{3099}", 2)
}

#[test]
fn ordinary_combining_handakuten_argument_preserves_data_caret_and_canonical_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{306f}\u{309a}", 2)
}

#[test]
fn standalone_halfwidth_dakuten_argument_preserves_one_cell_data_caret_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff9e}", 1)
}

#[test]
fn standalone_halfwidth_handakuten_argument_preserves_one_cell_data_caret_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff9f}", 1)
}

#[test]
fn halfwidth_base_combining_dakuten_argument_preserves_one_cell_data_caret_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff76}\u{3099}", 1)
}

#[test]
fn halfwidth_base_combining_handakuten_argument_preserves_one_cell_data_caret_and_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    assert_sound_mark_argument_width_contract("\u{ff8a}\u{309a}", 1)
}

#[test]
fn full_width_prepend_argument_survives_real_diff_and_crossterm_output()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = diff_prompt_form();
    let previous = mcp_ui_producer_snapshot(&ui)?;
    assert!(ui.mcp_argument_paste(PREPEND_ARGUMENT));
    for code in [KeyCode::Home, KeyCode::Right, KeyCode::Right] {
        assert!(ui.handle_mcp_key(key(code)).is_none());
    }
    let panel = ui.mcp_panel().unwrap_or_else(|| panic!("argument form"));
    assert_eq!(PREPEND_ARGUMENT.as_bytes(), &[0xd8, 0x80, 0xd8, 0x80, b'Z']);
    assert_prompt_argument_bytes(panel, PREPEND_ARGUMENT, 4);
    let placed = crate::render::mcp_geometry(&ui).unwrap_or_else(|| panic!("geometry"));
    let next = mcp_ui_producer_snapshot(&ui)?;
    let diff = mcp_consumer_diff(&previous, &next)?;
    let steady = mcp_ui_producer_snapshot(&ui)?;
    let unchanged = mcp_consumer_diff(&next, &steady)?;

    assert!(
        placed.body.width >= 8,
        "not the old two-column clipping fixture"
    );
    let (x, y) = assert_current_wide_diff(&previous, &next, &diff, PREPEND_ARGUMENT, 3);
    assert!(x >= placed.body.x && x + 3 < placed.body.right());
    assert_eq!(assert_caret_diff(&previous, &next, &diff), (x + 3, y));
    let caret = panel
        .caret
        .as_ref()
        .unwrap_or_else(|| panic!("semantic caret"));
    assert!(panel.document[caret.row][..caret.bytes.start].ends_with(PREPEND_ARGUMENT));
    assert_diff_cell(&diff, &next, (x, y));
    assert_diff_cell(&diff, &next, (x + 3, y));
    assert!(diff.ansi.contains(PREPEND_ARGUMENT) && diff.ansi.contains('▏'));
    assert_current_wide_diff(&next, &steady, &unchanged, PREPEND_ARGUMENT, 3);
    Ok(())
}

#[test]
fn prepend_argument_shrink_and_delete_restore_vacated_diff_columns()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = diff_prompt_form();
    ui.set_appearance(
        crate::theme::ThemeId::CyrilDark,
        crate::theme::ColorMode::TrueColor,
    );
    assert!(ui.mcp_argument_paste(PREPEND_ARGUMENT));
    for code in [KeyCode::Home, KeyCode::Right, KeyCode::Right] {
        assert!(ui.handle_mcp_key(key(code)).is_none());
    }
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let wide = mcp_ui_producer_snapshot(&ui)?;

    // Delete one Prepend scalar at the real logical byte-4 cursor. The
    // surviving data is a width-two grapheme, not standalone zero-width data.
    assert!(ui.handle_mcp_key(key(KeyCode::Backspace)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        "\u{0600}Z",
        2,
    );
    let shrunk = mcp_ui_producer_snapshot(&ui)?;
    let shrink_diff = mcp_consumer_diff(&wide, &shrunk)?;

    assert!(ui.handle_mcp_key(key(KeyCode::Home)).is_none());
    assert!(ui.handle_mcp_key(key(KeyCode::Delete)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        "Z",
        0,
    );
    let deleted = mcp_ui_producer_snapshot(&ui)?;
    let delete_diff = mcp_consumer_diff(&shrunk, &deleted)?;

    assert!(
        ui.handle_mcp_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL))
            .is_none()
    );
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        "",
        0,
    );
    let empty = mcp_ui_producer_snapshot(&ui)?;
    let empty_diff = mcp_consumer_diff(&deleted, &empty)?;

    let (x, y) = painted_symbol(&wide, PREPEND_ARGUMENT);
    assert_eq!(
        assert_current_wide_diff(&wide, &shrunk, &shrink_diff, "\u{0600}Z", 2),
        (x, y)
    );
    assert_eq!(assert_caret_diff(&wide, &shrunk, &shrink_diff), (x + 2, y));
    assert_former_columns_repainted(&shrunk, &shrink_diff, Rect::new(x, y, 4, 1));
    let narrow = painted_symbol(&deleted, "Z");
    assert_eq!(narrow, (x + 1, y));
    assert_eq!(assert_caret_diff(&shrunk, &deleted, &delete_diff), (x, y));
    assert_former_columns_repainted(&deleted, &delete_diff, Rect::new(x, y, 3, 1));
    assert_former_columns_repainted(&empty, &empty_diff, Rect::new(narrow.0, narrow.1, 1, 1));
    assert_eq!(painted_symbol(&empty, "▏"), (x, y));
    Ok(())
}

#[test]
fn prepend_argument_focus_and_appearance_repaint_protect_current_columns()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = diff_prompt_form();
    ui.set_appearance(
        crate::theme::ThemeId::CyrilDark,
        crate::theme::ColorMode::TrueColor,
    );
    assert!(ui.mcp_argument_paste(PREPEND_ARGUMENT));
    for code in [KeyCode::Home, KeyCode::Right, KeyCode::Right] {
        assert!(ui.handle_mcp_key(key(code)).is_none());
    }
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let focused = mcp_ui_producer_snapshot(&ui)?;

    assert!(ui.handle_mcp_key(key(KeyCode::Tab)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let inactive = mcp_ui_producer_snapshot(&ui)?;
    let blur_diff = mcp_consumer_diff(&focused, &inactive)?;
    assert!(ui.handle_mcp_key(key(KeyCode::BackTab)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let refocused = mcp_ui_producer_snapshot(&ui)?;
    let focus_diff = mcp_consumer_diff(&inactive, &refocused)?;

    ui.set_appearance(
        crate::theme::ThemeId::CyrilLight,
        crate::theme::ColorMode::TrueColor,
    );
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let light = mcp_ui_producer_snapshot(&ui)?;
    let appearance_diff = mcp_consumer_diff(&refocused, &light)?;
    assert!(
        ui.handle_mcp_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL))
            .is_none()
    );
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        "",
        0,
    );
    let empty = mcp_ui_producer_snapshot(&ui)?;
    let empty_diff = mcp_consumer_diff(&light, &empty)?;

    let position = painted_symbol(&focused, PREPEND_ARGUMENT);
    assert_eq!(
        assert_current_wide_diff(&focused, &inactive, &blur_diff, PREPEND_ARGUMENT, 3),
        position
    );
    assert_ne!(focused[position].modifier, inactive[position].modifier);
    assert_ne!(focused[position].bg, inactive[position].bg);
    assert_eq!(
        assert_current_wide_diff(&inactive, &refocused, &focus_diff, PREPEND_ARGUMENT, 3),
        position
    );
    assert_eq!(
        assert_caret_diff(&inactive, &refocused, &focus_diff),
        (position.0 + 3, position.1)
    );
    assert_eq!(
        assert_current_wide_diff(&refocused, &light, &appearance_diff, PREPEND_ARGUMENT, 3),
        position
    );
    assert_ne!(refocused[position].bg, light[position].bg);
    assert_caret_diff(&refocused, &light, &appearance_diff);
    assert_former_columns_repainted(&empty, &empty_diff, Rect::new(position.0, position.1, 4, 1));
    assert_eq!(
        empty[position].bg, light[position].bg,
        "retain the new selection style"
    );
    Ok(())
}

#[test]
fn prepend_argument_movement_geometry_and_close_restore_former_columns()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = UiState::new(100);
    ui.set_terminal_size(80, 24);
    ui.set_appearance(
        crate::theme::ThemeId::CyrilLight,
        crate::theme::ColorMode::TrueColor,
    );
    let underlying = mcp_ui_producer_snapshot(&ui)?;
    ui.show_mcp_panel(state(vec![server("exact")]));
    prompt_form(&mut ui);
    assert!(ui.mcp_argument_paste(PREPEND_ARGUMENT));
    for code in [KeyCode::Home, KeyCode::Right, KeyCode::Right] {
        assert!(ui.handle_mcp_key(key(code)).is_none());
    }
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        4,
    );
    let inside = mcp_ui_producer_snapshot(&ui)?;

    assert!(ui.handle_mcp_key(key(KeyCode::Home)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        0,
    );
    let home = mcp_ui_producer_snapshot(&ui)?;
    let home_diff = mcp_consumer_diff(&inside, &home)?;
    assert!(ui.handle_mcp_key(key(KeyCode::End)).is_none());
    assert_prompt_argument_bytes(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open")),
        PREPEND_ARGUMENT,
        5,
    );
    let end = mcp_ui_producer_snapshot(&ui)?;
    let end_diff = mcp_consumer_diff(&home, &end)?;

    // Exercise the widget's public placement/geometry path on the real form,
    // moving the overlay within equal fixed-canvas producer buffers.
    let mut panel = ui
        .mcp_panel()
        .unwrap_or_else(|| panic!("MCP panel should remain open"))
        .clone();
    let theme = ui.theme();
    assert_prompt_argument_bytes(&panel, PREPEND_ARGUMENT, 5);
    let placed = mcp_panel_producer_snapshot(&mut panel, Rect::new(0, 0, 80, 24), 22, &theme)?;
    let moved = mcp_panel_producer_snapshot(&mut panel, Rect::new(8, 2, 64, 20), 20, &theme)?;
    assert_prompt_argument_bytes(&panel, PREPEND_ARGUMENT, 5);
    let move_diff = mcp_consumer_diff(&placed, &moved)?;

    assert!(ui.handle_mcp_key(key(KeyCode::Esc)).is_none());
    assert!(matches!(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open"))
            .page,
        super::Page::Catalog
    ));
    let catalog = mcp_ui_producer_snapshot(&ui)?;
    let leave_diff = mcp_consumer_diff(&end, &catalog)?;
    let popup = crate::render::mcp_geometry(&ui)
        .unwrap_or_else(|| panic!("MCP geometry should be available"))
        .popup;
    assert!(ui.handle_mcp_key(key(KeyCode::Esc)).is_none());
    assert!(
        ui.mcp_panel().is_none(),
        "close must remove the actual UiState overlay"
    );
    let closed = mcp_ui_producer_snapshot(&ui)?;
    let close_diff = mcp_consumer_diff(&catalog, &closed)?;

    let (x, y) = painted_symbol(&inside, PREPEND_ARGUMENT);
    assert_eq!(
        assert_current_wide_diff(&inside, &home, &home_diff, PREPEND_ARGUMENT, 3),
        (x + 1, y)
    );
    assert_eq!(assert_caret_diff(&inside, &home, &home_diff), (x, y));
    assert_former_columns_repainted(&home, &home_diff, Rect::new(x, y, 4, 1));
    assert_eq!(
        assert_current_wide_diff(&home, &end, &end_diff, PREPEND_ARGUMENT, 3),
        (x, y)
    );
    assert_eq!(assert_caret_diff(&home, &end, &end_diff), (x + 3, y));
    assert_former_columns_repainted(&end, &end_diff, Rect::new(x, y, 4, 1));
    let old = painted_symbol(&placed, PREPEND_ARGUMENT);
    let new = assert_current_wide_diff(&placed, &moved, &move_diff, PREPEND_ARGUMENT, 3);
    assert_ne!(old, new, "the real placement must move the data");
    assert_eq!(
        assert_caret_diff(&placed, &moved, &move_diff),
        (new.0 + 3, new.1)
    );
    assert_former_columns_repainted(&moved, &move_diff, Rect::new(old.0, old.1, 4, 1));
    assert_former_columns_repainted(&catalog, &leave_diff, Rect::new(x, y, 4, 1));
    for column in x..x + 4 {
        assert_eq!(
            closed[(column, y)],
            underlying[(column, y)],
            "underlying UI restored"
        );
        if catalog[(column, y)] != closed[(column, y)] {
            assert_diff_cell(&close_diff, &closed, (column, y));
        }
    }
    let removed = (popup.y..popup.bottom())
        .flat_map(|row| (popup.x..popup.right()).map(move |column| (column, row)))
        .find(|&position| {
            catalog[position].symbol().is_ascii()
                && catalog[position].symbol() != " "
                && closed[position].symbol() == " "
        })
        .unwrap_or_else(|| panic!("closing must expose underlying UI beneath panel text"));
    assert_diff_cell(&close_diff, &closed, removed);
    Ok(())
}

#[test]
fn offscreen_resource_activation_reanchors_before_dispatching_the_retained_identity() {
    let mut entries = (*catalog()).clone();
    entries.resources = (0..80)
        .map(|index| McpResource {
            uri: format!("canary://resource/{index:02}?exact=a%20b&c=日"),
            name: format!("Resource {index:02}"),
            title: None,
            description: None,
            mime_type: None,
        })
        .collect();
    let entries = Arc::new(entries);
    for scroll_up in [false, true] {
        let mut connected = server("server exact / 日");
        connected.status = McpServerStatus::Connected {
            catalog: Arc::clone(&entries),
        };
        let mut ui = UiState::new(100);
        ui.show_mcp_panel(state(vec![connected]));
        ui.set_terminal_size(80, 16);
        for _ in 0..3 {
            assert!(ui.handle_mcp_key(key(KeyCode::Tab)).is_none());
        }
        if scroll_up {
            for _ in 0..79 {
                assert!(ui.handle_mcp_key(key(KeyCode::Down)).is_none());
            }
        }
        let selected = entries.resources[if scroll_up { 79 } else { 0 }]
            .uri
            .clone();
        for _ in 0..3 {
            assert!(
                ui.handle_mcp_key(key(if scroll_up {
                    KeyCode::PageUp
                } else {
                    KeyCode::PageDown
                }))
                .is_none()
            );
        }
        let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
        assert_eq!(panel.selected_entry.as_ref(), Some(&selected));
        assert!(
            panel.focus_line < panel.scroll || panel.focus_line >= panel.scroll + panel.window,
            "free scrolling must leave the retained target offscreen"
        );
        assert!(
            ui.handle_mcp_key(key(KeyCode::Enter)).is_none(),
            "the first activation must only restore the hidden target"
        );
        let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
        assert!(matches!(panel.page, super::Page::Catalog));
        assert!(panel.submitted.is_none());
        assert_eq!(panel.selected_entry.as_ref(), Some(&selected));
        assert!(panel.focus_line >= panel.scroll);
        assert!(panel.focus_line < panel.scroll + panel.window);
        let request = ui
            .handle_mcp_key(key(KeyCode::Enter))
            .unwrap_or_else(|| panic!("second explicit visible activation"));
        assert_eq!(request.session_id, SessionId::new("root"));
        assert_eq!(request.server_name, "server exact / 日");
        assert_eq!(
            request.operation,
            McpOperation::GetResource { uri: selected }
        );
        assert!(ui.messages().is_empty());
        assert!(ui.input_text().is_empty());
    }
}

fn assert_offscreen_server_action_reanchors(
    code: KeyCode,
    start_oauth: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let original = "server exact / 日";
    let other = "other visible / 日";
    let draft = "untouched chat draft";
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(vec![server(original), server(other)]));
    ui.insert_text(draft);
    let initial = render(&mut ui, 80, 6)?;
    let placed = crate::render::mcp_geometry(&ui).unwrap_or_else(|| panic!("compact popup"));
    assert_eq!(placed.popup.height, 2, "real compact popup has no header");
    assert_eq!(placed.body.height, 1, "one painted row plus the footer");
    assert!(initial.contains(&format!("> {original}")));
    assert!(!initial.contains(other));

    assert!(ui.handle_mcp_key(key(KeyCode::PageDown)).is_none());
    let scrolled = render(&mut ui, 80, 6)?;
    assert!(
        scrolled.contains(other),
        "another server is actually painted"
    );
    assert!(
        !scrolled.contains(original),
        "the action target is not painted"
    );
    let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
    assert_eq!(panel.selected_server.as_deref(), Some(original));
    assert!(panel.focus_line < panel.scroll);
    let hidden_scroll = panel.scroll;

    for modifiers in [
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SHIFT,
    ] {
        assert!(ui.handle_mcp_key(KeyEvent::new(code, modifiers)).is_none());
        assert_eq!(
            ui.mcp_panel()
                .unwrap_or_else(|| panic!("MCP panel should remain open"))
                .scroll,
            hidden_scroll
        );
    }
    assert!(ui.handle_mcp_key(key(KeyCode::Char('x'))).is_none());
    assert_eq!(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("MCP panel should remain open"))
            .scroll,
        hidden_scroll
    );
    assert!(
        ui.handle_mcp_key(key(code)).is_none(),
        "the first hidden-target reset/auth activation must not dispatch before draw"
    );
    let panel = ui.mcp_panel().unwrap_or_else(|| panic!("panel"));
    assert!(matches!(panel.page, super::Page::Catalog));
    assert!(panel.submitted.is_none());
    assert_eq!(panel.selected_server.as_deref(), Some(original));
    assert!(panel.focus_line >= panel.scroll);
    assert!(panel.focus_line < panel.scroll + panel.window);
    assert_eq!(ui.input_text(), draft);

    let reanchored = render(&mut ui, 80, 6)?;
    assert!(reanchored.contains(&format!("> {original}")));
    assert!(!reanchored.contains(other));
    let request = ui
        .handle_mcp_key(key(code))
        .unwrap_or_else(|| panic!("second explicit activation of the painted original server"));
    assert_eq!(
        request,
        McpRequest {
            session_id: SessionId::new("root"),
            server_name: original.into(),
            operation: McpOperation::Reset { start_oauth },
        }
    );
    assert!(ui.messages().is_empty());
    assert_eq!(ui.input_text(), draft);
    assert!(ui.handle_mcp_key(key(KeyCode::Esc)).is_none());
    assert!(ui.mcp_panel().is_none());
    assert_eq!(ui.input_text(), draft);
    Ok(())
}

#[test]
fn offscreen_server_reset_reanchors_before_dispatching_the_retained_identity()
-> Result<(), Box<dyn std::error::Error>> {
    assert_offscreen_server_action_reanchors(KeyCode::Char('r'), false)
}

#[test]
fn offscreen_server_authenticate_reanchors_before_dispatching_the_retained_identity()
-> Result<(), Box<dyn std::error::Error>> {
    assert_offscreen_server_action_reanchors(KeyCode::Char('a'), true)
}

#[test]
fn titled_prompt_keeps_its_display_label_and_dispatches_only_its_canonical_name() {
    let mut entries = (*catalog()).clone();
    entries.prompts[0].name = "wire/prompt exact 日".into();
    entries.prompts[0].title = Some("Advertised display label".into());
    entries.prompts.push(McpPrompt {
        name: "Advertised display label".into(),
        title: None,
        description: None,
        arguments: Vec::new(),
    });
    let mut connected = server("server exact / 日");
    connected.status = McpServerStatus::Connected {
        catalog: Arc::new(entries),
    };
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(vec![connected]));
    ui.handle_mcp_key(key(KeyCode::Tab));
    ui.handle_mcp_key(key(KeyCode::Tab));
    assert!(document(&ui).contains("> Advertised display label"));
    assert!(document(&ui).contains("Prompt name: wire/prompt exact 日"));
    assert_eq!(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("panel"))
            .selected_entry
            .as_deref(),
        Some("wire/prompt exact 日")
    );
    assert!(ui.handle_mcp_key(key(KeyCode::Enter)).is_none());
    assert!(
        ui.mcp_panel()
            .unwrap_or_else(|| panic!("panel"))
            .heading
            .contains("Arguments: Advertised display label")
    );
    assert!(ui.mcp_argument_paste(" exact string "));
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("canonical prompt request"));
    assert_eq!(request.session_id, SessionId::new("root"));
    assert_eq!(request.server_name, "server exact / 日");
    let McpOperation::GetPrompt { name, arguments } = request.operation else {
        panic!("prompt read")
    };
    assert_eq!(name, "wire/prompt exact 日");
    assert_eq!(arguments["required key"], " exact string ");
}

#[test]
fn session_boundaries_close_editor_and_no_read_or_paste_reaches_chat() {
    let mut ui = open();
    prompt_form(&mut ui);
    ui.mcp_argument_paste("old draft");
    ui.apply_notification(&Notification::SessionCreated {
        session_id: SessionId::new("new-root"),
        current_mode: None,
        current_model: None,
        available_modes: Vec::new(),
        available_models: Vec::new(),
    });
    assert!(ui.mcp_panel().is_none());
    assert!(ui.handle_mcp_key(key(KeyCode::Enter)).is_none());
    assert!(!ui.mcp_argument_paste("dropped"));
    assert!(ui.input_text().is_empty());
}

#[test]
fn both_elicitation_modes_are_visible_with_real_context_without_opening_overlay() {
    for mode in [McpElicitationMode::Form, McpElicitationMode::Url] {
        let mut ui = UiState::new(100);
        assert!(
            ui.apply_notification(&Notification::McpUpdate(McpUpdate::ElicitationDeclined(
                McpElicitationNotice {
                    session_id: SessionId::new("actual-session"),
                    tool_call_id: ToolCallId::new("actual-tool"),
                    mode,
                    message: "requested message".into(),
                    url: Some("https://unopened.invalid/".into()),
                }
            )))
        );
        let ChatMessageKind::System(text) = ui
            .messages()
            .last()
            .unwrap_or_else(|| panic!("notice"))
            .kind()
        else {
            panic!("system notice")
        };
        for expected in [
            "elicitation declined",
            "actual-session",
            "actual-tool",
            "not opened",
            "https://unopened.invalid/",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
        assert!(ui.topmost_overlay().is_none());
    }
}

#[test]
fn mcp_paste_is_inert_when_review_owns_keyboard() {
    let mut ui = open();
    prompt_form(&mut ui);
    // Overlay order itself is a public shared paint/input contract.
    let mcp = Overlay::ALL
        .iter()
        .position(|overlay| *overlay == Overlay::Mcp)
        .unwrap_or_else(|| panic!("MCP"));
    let review = Overlay::ALL
        .iter()
        .position(|overlay| *overlay == Overlay::Review)
        .unwrap_or_else(|| panic!("review"));
    let approval = Overlay::ALL
        .iter()
        .position(|overlay| *overlay == Overlay::Approval)
        .unwrap_or_else(|| panic!("approval"));
    assert!(mcp < review && review < approval);
    ui.show_review_form(crate::traits::ReviewForm::opening(
        cyril_core::review::target::ReviewTarget::Auto,
    ));
    assert_eq!(ui.topmost_overlay(), Some(Overlay::Review));
    assert!(!ui.mcp_argument_paste("blocked by review"));
    assert!(ui.handle_mcp_key(key(KeyCode::Enter)).is_none());
    ui.close_review_form();
    assert!(
        ui.handle_mcp_key(key(KeyCode::Enter)).is_none(),
        "required field remains omitted"
    );
    ui.hide_mcp_panel();
    assert!(!ui.mcp_argument_paste("hidden"));
}

#[test]
fn hidden_geometry_and_key_releases_cannot_fire_actions() {
    let mut ui = open();
    ui.set_terminal_size(2, 2);
    assert!(ui.handle_mcp_key(key(KeyCode::Char('r'))).is_none());
    ui.set_terminal_size(80, 24);
    assert!(
        ui.handle_mcp_key(KeyEvent::new_with_kind(
            KeyCode::Char('r'),
            KeyModifiers::NONE,
            crossterm::event::KeyEventKind::Release,
        ))
        .is_none()
    );
    assert!(
        ui.handle_mcp_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .is_none()
    );
    assert!(ui.handle_mcp_key(key(KeyCode::Char('r'))).is_some());
}

#[test]
fn opaque_names_and_manual_urls_preserve_printable_punctuation() {
    let name = "server 'single' \"double\" \\ 日";
    let url = "https://auth.invalid/a'b\"c\\日?state=x#fragment";
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(state(vec![McpServer {
        name: name.into(),
        auth_type: None,
        status: McpServerStatus::Failed {
            failed_authorization: true,
            authorization_url: Some(url.into()),
            error_message: "failure 'quoted' \\ 日".into(),
            error_code: None,
        },
        provenance: None,
    }]));
    let text = document(&ui);
    assert!(text.contains(name));
    // The URI may wrap physically, so inspect it without row separators.
    assert!(text.replace('\n', "").contains(url));
    assert!(text.contains("failure 'quoted' \\ 日"));
}

#[test]
fn passive_authorization_url_survives_connecting_and_reopen_until_authoritative_resolution() {
    let initial = state(vec![McpServer {
        name: "auth".into(),
        auth_type: None,
        status: McpServerStatus::Failed {
            failed_authorization: true,
            authorization_url: Some("https://auth.invalid/retained".into()),
            error_message: "authorize".into(),
            error_code: None,
        },
        provenance: None,
    }]);
    let mut data = (*initial).clone();
    let mut connecting = server("auth");
    connecting.status = McpServerStatus::Connecting;
    data.apply(&McpUpdate::Snapshot(Arc::clone(
        state(vec![connecting])
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("connecting snapshot")),
    )));
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::new(data.clone()));
    assert!(document(&ui).contains("connecting"));
    assert!(document(&ui).contains("https://auth.invalid/retained"));
    ui.hide_mcp_panel();
    ui.show_mcp_panel(Arc::new(data.clone()));
    assert!(document(&ui).contains("https://auth.invalid/retained"));
    data.apply(&McpUpdate::Snapshot(Arc::clone(
        state(vec![server("auth")])
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("connected snapshot")),
    )));
    ui.refresh_mcp_panel(Arc::new(data.clone()));
    assert!(!document(&ui).contains("https://auth.invalid/retained"));
    data.apply(&McpUpdate::Snapshot(Arc::clone(
        state(Vec::new())
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("empty snapshot")),
    )));
    ui.refresh_mcp_panel(Arc::new(data));
    assert!(!document(&ui).contains("https://auth.invalid/retained"));
}

#[test]
fn invisible_argument_editor_refuses_paste_without_changing_presence() {
    let mut ui = open();
    prompt_form(&mut ui);
    ui.set_terminal_size(2, 2);
    assert!(!ui.mcp_argument_paste("hidden value"));
    ui.set_terminal_size(80, 24);
    assert!(ui.handle_mcp_key(key(KeyCode::Enter)).is_none());
    assert!(document(&ui).contains("Required argument"));
    assert!(ui.mcp_argument_paste("visible value"));
    let request = ui
        .handle_mcp_key(key(KeyCode::Enter))
        .unwrap_or_else(|| panic!("visible argument fetch"));
    let McpOperation::GetPrompt { arguments, .. } = request.operation else {
        panic!("getPrompt only");
    };
    assert_eq!(arguments["required key"], "visible value");
}

#[test]
fn result_shortcut_requires_an_unmodified_key() {
    let mut data = (*state(vec![server("exact")])).clone();
    data.apply(&McpUpdate::Outcome(McpOutcome {
        id: 1,
        request: McpRequest {
            session_id: SessionId::new("root"),
            server_name: "exact".into(),
            operation: McpOperation::Reset { start_oauth: false },
        },
        result: Ok(McpOperationResult::Acknowledged),
    }));
    let mut ui = UiState::new(100);
    ui.show_mcp_panel(Arc::new(data));
    let before = document(&ui);
    assert!(
        ui.handle_mcp_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL))
            .is_none()
    );
    assert_eq!(document(&ui), before);
    assert!(ui.handle_mcp_key(key(KeyCode::Char('v'))).is_none());
    assert!(
        document(&ui).contains("Reset acknowledged. Only a status snapshot can report connected.")
    );
}
