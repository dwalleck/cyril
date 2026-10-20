//! Real root-consumer work fences. Timings belong to conversation_render, not these tests.
//! Existing frozen conversation and pinned Paragraph differential fixtures remain the
//! fidelity oracles; positive controls here prevent empty/fallback work-counter passes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;

use cyril_core::types::{
    AgentMessage, AgentThought, Notification, SessionId, StopReason, ToolCall, ToolCallContent,
    ToolCallId, ToolCallStatus, ToolKind,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::render::{self, RenderState};
use crate::state::UiState;
use crate::theme::{ColorMode, ThemeId};
use crate::traits::{Activity, ChatMessageKind, SteerEchoStatus, TuiState};
use crate::widgets::{chat, markdown};

const WIDTH: u16 = 160;
const CONTENT_WIDTH: u16 = WIDTH - 1;
const HEIGHT: u16 = 48;
const STABLE_DRAWS: usize = 100;
const SIZES: [usize; 4] = [100, 1_000, 10_000, 50_000];

struct Root {
    terminal: Terminal<TestBackend>,
    retained: RenderState,
}

impl Root {
    fn new() -> Self {
        Self {
            terminal: Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap(),
            retained: RenderState::new(),
        }
    }

    fn draw(&mut self, state: &UiState) {
        self.terminal
            .draw(|frame| render::draw(frame, state, &mut self.retained))
            .unwrap();
    }

    fn resize(&mut self, state: &mut UiState, width: u16, height: u16) {
        state.set_terminal_size(width, height);
        self.terminal.backend_mut().resize(width, height);
        self.terminal.autoresize().unwrap();
    }

    fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    fn text(&self) -> String {
        buffer_text(self.buffer())
    }

    fn contains(&self, controls: &[&str]) {
        let text = self.text();
        for control in controls {
            assert!(
                text.contains(control),
                "missing visible source {control:?}\n{text}"
            );
        }
        assert!(
            !text.contains("rendering error"),
            "root fallback is not a pass: {text}"
        );
    }

    fn absent(&self, controls: &[&str]) {
        let text = self.text();
        for control in controls {
            assert!(
                !text.contains(control),
                "stale/wrong transcript {control:?}\n{text}"
            );
        }
    }

    fn styled(&self, text: &str, color: Color, modifier: Modifier) {
        self.style_control(text, Some(color), modifier);
    }

    fn style_control(&self, text: &str, color: Option<Color>, modifier: Modifier) {
        assert!(text.is_ascii(), "style controls must be ASCII");
        let buffer = self.buffer();
        for row in buffer.content.chunks(usize::from(buffer.area.width).max(1)) {
            for cells in row.windows(text.len()) {
                if cells
                    .iter()
                    .zip(text.bytes())
                    .all(|(cell, byte)| cell.symbol().as_bytes() == [byte])
                {
                    for cell in cells {
                        if let Some(expected) = color {
                            assert_eq!(cell.fg, expected, "source foreground: {text}");
                        }
                        assert!(cell.modifier.contains(modifier), "source modifier: {text}");
                    }
                    return;
                }
            }
        }
        panic!("missing styled control {text:?}\n{}", self.text());
    }
}

fn buffer_text(buffer: &Buffer) -> String {
    let mut text = String::with_capacity(buffer.content.len());
    for row in buffer.content.chunks(usize::from(buffer.area.width).max(1)) {
        for cell in row {
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    text
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Work {
    messages: usize,
    live: usize,
    hashes: usize,
    parses: usize,
    wraps: usize,
    myers: usize,
    rows: usize,
    runs: usize,
    seeks: usize,
}

impl Work {
    fn now() -> Self {
        let chat = chat::work_counts();
        let (hashes, parses) = markdown::work_counts();
        Self {
            messages: chat.prepared_messages,
            live: chat.prepared_live,
            hashes,
            parses,
            wraps: chat::wrap_count(),
            myers: crate::tool_diff::text_diff_constructions(),
            rows: chat.painted_rows,
            runs: chat.resolved_runs,
            seeks: chat.index_steps,
        }
    }

    fn since(self, before: Self) -> Self {
        Self {
            messages: self.messages - before.messages,
            live: self.live - before.live,
            hashes: self.hashes - before.hashes,
            parses: self.parses - before.parses,
            wraps: self.wraps - before.wraps,
            myers: self.myers - before.myers,
            rows: self.rows - before.rows,
            runs: self.runs - before.runs,
            seeks: self.seeks - before.seeks,
        }
    }

    fn no_derivation(self) {
        assert_eq!(self.messages, 0, "unchanged history was prepared: {self:?}");
        assert_eq!(self.live, 0, "unchanged live source was prepared: {self:?}");
        assert_eq!(
            self.hashes, 0,
            "unchanged complete source was hashed: {self:?}"
        );
        assert_eq!(self.parses, 0, "unchanged Markdown was parsed: {self:?}");
        assert_eq!(self.wraps, 0, "unchanged source was wrapped: {self:?}");
        assert_eq!(
            self.myers, 0,
            "unchanged Write reconstructed Myers: {self:?}"
        );
    }

    fn selected_window(self, draws: usize, blocks: usize, width: u16, height: u16) {
        // These fixtures contain ASCII positive-width runs, plus a bounded
        // number of generated gutters/labels. A run cannot outnumber cells
        // in a selected row. No hidden-message/hidden-line overscan is allowed.
        assert!(
            self.rows > 0,
            "no paint operations were exercised: {self:?}"
        );
        assert!(
            self.rows <= draws * usize::from(height),
            "paint walked hidden rows: {self:?}"
        );
        assert!(self.runs > 0, "no source runs were resolved: {self:?}");
        assert!(
            self.runs <= draws * usize::from(width) * usize::from(height),
            "resolve walked hidden runs: {self:?}"
        );
        // Prefix retirement can leave at most a second retained block count
        // before metadata-only compaction. Binary lifting visits each bit once.
        let index_bits =
            usize::BITS as usize - (blocks.saturating_mul(2).max(1)).leading_zeros() as usize;
        assert!(
            self.seeks <= draws * index_bits,
            "seek is not logarithmic in retained blocks: {self:?}"
        );
    }
}

fn new_state(limit: usize) -> UiState {
    let mut state = UiState::new(limit);
    state.set_terminal_size(WIDTH, HEIGHT);
    state.set_appearance(ThemeId::CyrilDark, ColorMode::TrueColor);
    state
}

fn agent(text: impl Into<String>, streaming: bool) -> Notification {
    Notification::AgentMessage(AgentMessage {
        text: text.into(),
        is_streaming: streaming,
    })
}

fn send(state: &mut UiState, notification: Notification) {
    assert!(state.apply_notification(&notification));
}

fn tool(id: &str, kind: ToolKind, output: &str) -> ToolCall {
    let input = match kind {
        ToolKind::Execute => serde_json::json!({"command": format!("printf {id}")}),
        _ => serde_json::json!({"path": format!("fixture_{id}.rs")}),
    };
    let call = ToolCall::new(
        ToolCallId::new(id),
        format!("Fixture {id}"),
        kind,
        ToolCallStatus::Completed,
        Some(input),
    );
    if kind == ToolKind::Write {
        call.with_content(vec![ToolCallContent::Diff {
            path: format!("fixture_{id}.rs"),
            old_text: Some("const OLD_CONTROL: u64 = 1;\n".into()),
            new_text: "const NEW_CONTROL: u64 = 2;\n".into(),
        }])
    } else {
        call.with_content(vec![ToolCallContent::Text(output.into())])
            .with_raw_output(Some(serde_json::json!(output)))
    }
}

fn insert_tool(state: &mut UiState, id: &str, kind: ToolKind, output: &str) {
    send(state, Notification::ToolCallStarted(tool(id, kind, output)));
    state.commit_streaming();
}

fn edit_tool(state: &mut UiState, id: &str, output: &str) {
    send(
        state,
        Notification::ToolCallUpdated(
            ToolCall::new(
                ToolCallId::new(id),
                String::new(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Text(output.into())])
            .with_raw_output(Some(serde_json::json!(output))),
        ),
    );
}

// Uses only production UiState/typed-notification APIs also present before
// c6la. This exact source fixture is portable to the baseline RED lane.
fn mixed_history(size: usize) -> UiState {
    let mut state = new_state(size);
    insert_tool(
        &mut state,
        "older-tool",
        ToolKind::Execute,
        "OLDER_TOOL_ORIGINAL",
    );
    state.add_steer_echo("OLDER_RECEIPT_CONTROL");
    send(
        &mut state,
        Notification::SteeringQueued {
            message: None,
            message_id: Some("steer-older-receipt".into()),
        },
    );
    for index in 2..size - 6 {
        match index % 9 {
            0 => state.add_user_message(&format!("unique_user_{index:05} /workspace/{index}.rs")),
            1 | 2 => send(
                &mut state,
                agent(
                    format!("unique_md_{index:05} **bold** *italic*\n\n- item_{index}\n"),
                    false,
                ),
            ),
            3 => send(
                &mut state,
                agent(
                    format!("```rust\nconst UNIQUE_{index:05}: u64 = {index};\n```\n"),
                    false,
                ),
            ),
            4 => send(
                &mut state,
                agent(
                    format!("| unique_{index:05} | value |\n|---|---|\n| retained | {index} |\n"),
                    false,
                ),
            ),
            5 => {
                send(
                    &mut state,
                    Notification::AgentThought(AgentThought {
                        text: format!("thought_{index:05}"),
                    }),
                );
                state.commit_streaming();
            }
            6 => insert_tool(
                &mut state,
                &format!("read-{index}"),
                ToolKind::Read,
                "read source",
            ),
            7 => insert_tool(
                &mut state,
                &format!("execute-{index}"),
                ToolKind::Execute,
                "execute source",
            ),
            _ => insert_tool(&mut state, &format!("write-{index}"), ToolKind::Write, ""),
        }
    }
    // Constant padding fills any space above the five source controls, so
    // the entire visible window—not merely its last rows—is size-independent.
    state.add_system_message("VISIBLE_SUFFIX_PADDING\n".repeat(usize::from(HEIGHT)));
    state.add_user_message("VISIBLE_USER_CONTROL /workspace/control.rs");
    send(
        &mut state,
        agent(
            "VISIBLE_AGENT_CONTROL **bold_control**\n\n```rust\nfn visible_code_control() {}\n```\n\n| visible_table_control | value |\n|---|---|\n| retained | yes |\n",
            false,
        ),
    );
    send(
        &mut state,
        Notification::AgentThought(AgentThought {
            text: "VISIBLE_THOUGHT_CONTROL".into(),
        }),
    );
    state.commit_streaming();
    insert_tool(&mut state, "visible-write", ToolKind::Write, "");
    insert_tool(
        &mut state,
        "visible-execute",
        ToolKind::Execute,
        "VISIBLE_EXECUTE_CONTROL",
    );
    state.set_activity(Activity::Idle);
    state
}

const VISIBLE: &[&str] = &[
    "VISIBLE_USER_CONTROL",
    "VISIBLE_AGENT_CONTROL",
    "bold_control",
    "visible_code_control",
    "visible_table_control",
    "VISIBLE_THOUGHT_CONTROL",
    "│- const OLD_CONTROL",
    "│+ const NEW_CONTROL",
    "VISIBLE_EXECUTE_CONTROL",
];

fn unchanged(root: &mut Root, state: &UiState, controls: &[&str], draws: usize) -> Work {
    root.contains(controls);
    let before = Work::now();
    for _ in 0..draws {
        root.draw(state);
    }
    let delta = Work::now().since(before);
    root.contains(controls);
    delta.no_derivation();
    delta
}

#[test]
fn c1_same_visible_suffix_has_no_historical_work_at_all_retained_sizes() {
    let mut suffix = None;
    for size in SIZES {
        let state = mixed_history(size);
        assert_eq!(
            state.history().messages().len(),
            size,
            "silent retention pruning"
        );
        let markdown_sources = state
            .history()
            .messages()
            .iter()
            .filter(|message| matches!(message.kind(), ChatMessageKind::AgentText(_)))
            .count();
        assert!(size < 1_000 || markdown_sources > 256);
        let mut root = Root::new();
        let before = Work::now();
        root.draw(&state);
        let cold = Work::now().since(before);
        assert_eq!(
            cold.messages, size,
            "cold preparation must reach every retained message"
        );
        assert!(
            cold.hashes >= markdown_sources,
            "actual hash instrumentation did not execute"
        );
        assert!(
            cold.wraps >= size,
            "actual wrapper instrumentation did not execute"
        );
        assert_eq!(root.retained.layout_metrics().messages, size);
        root.contains(VISIBLE);
        root.styled("You:", state.theme().user, Modifier::BOLD);
        root.styled("Kiro:", state.theme().agent, Modifier::BOLD);
        root.styled(
            "VISIBLE_THOUGHT_CONTROL",
            state.theme().muted,
            Modifier::ITALIC,
        );
        // Exclude only the rightmost scrollbar column; its thumb necessarily
        // changes with total retained rows, while every source/style cell agrees.
        let mut cells = Vec::new();
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        for y in 1..render::input_top(area, &state) {
            for x in 0..WIDTH - 1 {
                cells.push(root.buffer()[(x, y)].clone());
            }
        }
        if let Some(expected) = &suffix {
            assert_eq!(
                &cells, expected,
                "same retained suffix changed cells/styles at {size}"
            );
        } else {
            suffix = Some(cells);
        }
        unchanged(&mut root, &state, VISIBLE, STABLE_DRAWS).selected_window(
            STABLE_DRAWS,
            size,
            WIDTH,
            chat_height(&root, &state),
        );
    }
}

// Deliberately independent of LayoutMetrics, MessageHistory and visible-paint
// counters, so baseline instrumentation never needs fake zero values for an
// old renderer's unavailable row/run/index telemetry.
fn portable_operations() -> [usize; 5] {
    let (hashes, parses) = markdown::work_counts();
    [
        chat::work_counts().prepared_messages,
        hashes,
        parses,
        chat::wrap_count(),
        crate::tool_diff::text_diff_constructions(),
    ]
}

fn portable_unchanged(root: &mut Root, state: &UiState, controls: &[&str]) {
    root.contains(controls);
    let before = portable_operations();
    for _ in 0..STABLE_DRAWS {
        root.draw(state);
    }
    root.contains(controls);
    assert_eq!(
        portable_operations(),
        before,
        "actual historical operations changed: [prepared_messages, source_hashes, parses, wraps, Myers]"
    );
}

/// Portable baseline-sensitivity test: only Root's retained draw adapter differs
/// on pre-c6la source. Counters must be placed at the old actual render_message,
/// Markdown hash/parser, Paragraph wrap/line_count/render and Myers operations.
/// No metrics/history-owner API is consumed by this test or mixed_history.
#[test]
fn c1_portable_baseline_unchanged_root_draw_does_not_reprepare_history() {
    let state = mixed_history(1_000);
    let mut root = Root::new();
    let before = portable_operations();
    root.draw(&state);
    let after = portable_operations();
    let cold = std::array::from_fn::<_, 5, _>(|index| after[index] - before[index]);
    assert_eq!(
        cold[0], 1_000,
        "baseline must actually retain and prepare the complete fixture"
    );
    assert!(cold[1] > 256, "unique source hash path was not exercised");
    assert!(cold[3] > 0, "actual wrap path was not exercised");
    root.contains(VISIBLE);
    eprintln!(
        "C6LA_BASELINE_CONTROLS_EXECUTED case=mixed retained_prepared={} source_hashes={} actual_wraps={} visible_controls={} stable_draws={STABLE_DRAWS}",
        cold[0],
        cold[1],
        cold[3],
        VISIBLE.len()
    );
    // A RED qualifies only after the cold/source controls above execute, and
    // only at one of these actual-operation zero-work assertions.
    portable_unchanged(&mut root, &state, VISIBLE);
}

fn chat_height(root: &Root, state: &UiState) -> u16 {
    // These work-bound fixtures have no crew/voice rows. Use the actual root
    // input geometry rather than assuming the input widget's minimum height.
    render::input_top(root.buffer().area, state).saturating_sub(1)
}

fn scroll_to(root: &Root, state: &mut UiState, top: usize) {
    let chat_height = usize::from(chat_height(root, state));
    let total = root.retained.layout_metrics().rows;
    state.chat_scroll_reset();
    state.chat_scroll_up(total.saturating_sub(chat_height).saturating_sub(top));
}

fn opaque_auth_url(length: usize, last: char) -> String {
    let mut source = "https://auth.example.invalid/oauth/authorize?response_type=code&client_id=cyril&redirect_uri=http%3A%2F%2F127.0.0.1%3A43117%2Fcallback&scope=openid&code_challenge=".to_owned();
    assert!(source.len() < length);
    source.extend(std::iter::repeat_n('a', length - source.len() - 1));
    source.push(last);
    assert!(source.is_ascii());
    assert_eq!(source.len(), length);
    source
}

fn assert_gutter(root: &Root, state: &UiState, overflow: bool) {
    let symbols = ratatui::symbols::scrollbar::DOUBLE_VERTICAL;
    let content_width = root.buffer().area.width - 1;
    let bottom = render::input_top(root.buffer().area, state);
    for y in 1..bottom {
        for x in 0..content_width {
            assert!(
                ![symbols.begin, symbols.end, symbols.track, symbols.thumb]
                    .contains(&root.buffer()[(x, y)].symbol()),
                "indicator entered content at ({x}, {y})"
            );
        }
        let gutter = root.buffer()[(content_width, y)].symbol();
        if overflow {
            assert!(
                [symbols.begin, symbols.end, symbols.track, symbols.thumb].contains(&gutter),
                "missing gutter indicator at ({content_width}, {y}): {gutter:?}"
            );
        } else {
            assert_eq!(gutter, " ", "reserved gutter contains source at row {y}");
        }
    }
    if overflow {
        assert_eq!(root.buffer()[(content_width, 1)].symbol(), symbols.begin);
        assert_eq!(
            root.buffer()[(content_width, bottom - 1)].symbol(),
            symbols.end
        );
    }
}

fn assert_complete_auth_url(root: &Root, state: &UiState, source: &str) {
    root.contains(&["https://auth.example.invalid/oauth/authorize?"]);
    let content_width = root.buffer().area.width - 1;
    let bottom = render::input_top(root.buffer().area, state);
    let first = (1..bottom)
        .find(|&y| root.buffer()[(0, y)].symbol() == "h")
        .expect("opaque URL starts at the content's left edge");
    let mut reconstructed = String::new();
    for (row, expected) in source
        .as_bytes()
        .chunks(usize::from(content_width))
        .enumerate()
    {
        let y = first + u16::try_from(row).unwrap();
        assert!(y < bottom, "complete URL must remain in the chat viewport");
        let mut rendered = String::new();
        for x in 0..content_width {
            let cell = &root.buffer()[(x, y)];
            if let Some(&byte) = expected.get(usize::from(x)) {
                assert_eq!(cell.symbol().as_bytes(), [byte], "URL byte at ({x}, {y})");
                assert_eq!(cell.fg, state.theme().system);
                assert!(cell.modifier.contains(Modifier::ITALIC));
            } else {
                assert_eq!(cell.symbol(), " ", "unexpected content after URL tail");
            }
            rendered.push_str(cell.symbol());
        }
        reconstructed.push_str(rendered.trim_end());
    }
    assert_eq!(
        reconstructed, source,
        "wrapped URL lost or reordered source bytes"
    );
}

#[test]
fn c2_c5_stable_gutter_preserves_complete_auth_url_at_bottom_and_page_up() {
    const URL_WIDTH: u16 = 140;
    const URL_HEIGHT: u16 = 40;
    // Exercise the final content column, the old outer-right edge, and the
    // character immediately after that edge. These are whole opaque sources,
    // not a prefix assertion that could miss an overwritten token tail.
    for length in [
        2 * usize::from(URL_WIDTH - 1),
        2 * usize::from(URL_WIDTH),
        2 * usize::from(URL_WIDTH) + 1,
    ] {
        let url = opaque_auth_url(length, 'Z');
        let mut state = new_state(8);
        send(
            &mut state,
            agent(format!("**GUTTER_HISTORY_HEAD_{length}**"), false),
        );
        let mut source = String::new();
        for row in 0..2 * URL_HEIGHT {
            writeln!(source, "gutter_padding_{row:03}").unwrap();
        }
        writeln!(source, "{url}").unwrap();
        // App's PageUp uses half the terminal height. Leave that many rows
        // after the URL so the complete source is visible before and after it.
        let half_page = usize::from(URL_HEIGHT / 2);
        for row in 0..half_page {
            writeln!(source, "gutter_tail_{row:03}").unwrap();
        }
        state.add_system_message(source.clone());
        state.set_activity(Activity::Idle);
        let mut root = Root::new();
        root.resize(&mut state, URL_WIDTH, URL_HEIGHT);
        let before = Work::now();
        root.draw(&state);
        let cold = Work::now().since(before);
        assert_eq!(cold.messages, 2);
        assert!(
            cold.hashes > 0 && cold.wraps >= 2,
            "cold controls: {cold:?}"
        );
        let metrics = root.retained.layout_metrics();
        assert!(metrics.rows > usize::from(chat_height(&root, &state)));
        root.contains(&["gutter_tail_019"]);
        assert_complete_auth_url(&root, &state, &url);
        assert_gutter(&root, &state, true);
        for (up, rows, offset) in [
            (true, half_page, Some(half_page)),
            (false, half_page / 2, Some(half_page / 2)),
            (false, half_page / 2, None),
        ] {
            if up {
                state.chat_scroll_up(rows);
            } else {
                state.chat_scroll_down(rows);
            }
            assert_eq!(state.chat_scroll_back(), offset);
            let before = Work::now();
            root.draw(&state);
            let work = Work::now().since(before);
            work.no_derivation();
            work.selected_window(1, 2, URL_WIDTH - 1, chat_height(&root, &state));
            assert_eq!(root.retained.layout_metrics(), metrics);
            assert_complete_auth_url(&root, &state, &url);
            assert_gutter(&root, &state, true);
        }
        for browse in [true, false] {
            if browse {
                state.chat_scroll_up(0);
            } else {
                state.chat_scroll_reset();
            }
            assert_eq!(state.chat_scroll_back(), browse.then_some(0));
            let before = Work::now();
            root.draw(&state);
            Work::now().since(before).no_derivation();
            assert_complete_auth_url(&root, &state, &url);
            assert_gutter(&root, &state, true);
            assert_eq!(root.retained.layout_metrics(), metrics);
        }
        assert!(matches!(
            state.history().messages()[1].kind(),
            ChatMessageKind::System(text) if text == &source
        ));
    }
}

#[test]
fn c1_c5_gutter_overflow_threshold_keeps_committed_content_width_and_work_stable() {
    let url = opaque_auth_url(2 * usize::from(WIDTH), 'Q');
    let mut state = new_state(8);
    send(&mut state, agent("**GUTTER_THRESHOLD_MARKDOWN**", false));
    state.add_system_message(url.clone());
    state.set_activity(Activity::Idle);
    let mut root = Root::new();
    root.draw(&state);
    let metrics = root.retained.layout_metrics();
    let chrome = HEIGHT - chat_height(&root, &state);
    let fit = u16::try_from(metrics.rows).unwrap() + chrome;
    for (height, overflow) in [
        (fit + 1, false),
        (fit, false),
        (fit - 1, true),
        (fit, false),
    ] {
        root.resize(&mut state, WIDTH, height);
        let before = Work::now();
        root.draw(&state);
        let work = Work::now().since(before);
        work.no_derivation();
        work.selected_window(1, 2, CONTENT_WIDTH, chat_height(&root, &state));
        assert_eq!(
            metrics.rows > usize::from(chat_height(&root, &state)),
            overflow,
            "fixture must cross the actual root overflow threshold"
        );
        assert_eq!(root.retained.layout_metrics(), metrics);
        root.contains(&["GUTTER_THRESHOLD_MARKDOWN"]);
        root.style_control("GUTTER_THRESHOLD_MARKDOWN", None, Modifier::BOLD);
        assert_complete_auth_url(&root, &state, &url);
        assert_gutter(&root, &state, overflow);
    }
    // Crossing through a real append may derive the new block, but none of
    // the two already committed sources or the stable live/Markdown blocks.
    root.resize(&mut state, WIDTH, fit + 1);
    let before = Work::now();
    root.draw(&state);
    Work::now().since(before).no_derivation();
    assert_gutter(&root, &state, false);
    state.add_system_message("GUTTER_APPEND_CONTROL".into());
    let before = Work::now();
    root.draw(&state);
    let appended = Work::now().since(before);
    assert_eq!(
        (
            appended.messages,
            appended.live,
            appended.hashes,
            appended.parses,
            appended.wraps,
            appended.myers,
        ),
        (1, 0, 0, 0, 1, 0),
        "only the appended plain System block may derive: {appended:?}"
    );
    appended.selected_window(1, 3, CONTENT_WIDTH, chat_height(&root, &state));
    let after = root.retained.layout_metrics();
    assert_eq!(after.messages, metrics.messages + 1);
    assert_eq!(after.rows, metrics.rows + 2);
    assert!(after.rows > usize::from(chat_height(&root, &state)));
    root.contains(&["GUTTER_THRESHOLD_MARKDOWN", "GUTTER_APPEND_CONTROL"]);
    assert_complete_auth_url(&root, &state, &url);
    assert_gutter(&root, &state, true);
    let before = Work::now();
    state.chat_scroll_up(1);
    root.draw(&state);
    Work::now().since(before).no_derivation();
    assert_complete_auth_url(&root, &state, &url);
    assert_gutter(&root, &state, true);
    assert_eq!(root.retained.layout_metrics(), after);
}

#[test]
fn c2_physical_rows_first_middle_last_follow_back_and_overlarge_back() {
    const ROWS: usize = 80_010;
    let mut source = String::new();
    for row in 0..ROWS {
        writeln!(source, "physical_{row:05}").unwrap();
    }
    let mut state = new_state(8);
    state.add_user_message(&source);
    let mut root = Root::new();
    root.draw(&state);
    let metrics = root.retained.layout_metrics();
    assert!(metrics.rows > usize::from(u16::MAX));
    // The plain source has no derived body: only the four-byte user label
    // and at most two indentation bytes per logical line may be owned.
    // This is fixture presentation math, not a universal storage budget.
    assert!(
        metrics.owned_text_bytes <= "You:".len() + 2 * ROWS,
        "plain authoritative source was copied into prepared backing: {metrics:?}"
    );
    root.contains(&["physical_80009"]);
    for (top, marker) in [
        (0, "physical_00000"),
        (40_001, "physical_40000"),
        (70_001, "physical_70000"),
    ] {
        scroll_to(&root, &mut state, top);
        let before = Work::now();
        root.draw(&state);
        let delta = Work::now().since(before);
        delta.no_derivation();
        delta.selected_window(1, 1, WIDTH, chat_height(&root, &state));
        root.contains(&[marker]);
    }
    state.chat_scroll_reset();
    root.draw(&state);
    let follow = root.buffer().clone();
    state.chat_scroll_up(7);
    root.draw(&state);
    root.contains(&["physical_80002"]);
    root.absent(&["physical_80009"]);
    state.chat_scroll_down(7);
    root.draw(&state);
    assert_eq!(
        root.buffer(),
        &follow,
        "rows-back did not restore exact follow cells"
    );
    state.chat_scroll_up(usize::MAX);
    root.draw(&state);
    root.contains(&["You:", "physical_00000"]);
    root.absent(&["physical_70000", "physical_80009"]);
    let first = root.buffer().clone();
    state.chat_scroll_reset();
    state.chat_scroll_up(metrics.rows);
    root.draw(&state);
    assert_eq!(
        root.buffer(),
        &first,
        "overlarge back must clamp to first physical row"
    );
    state.insert_text("submit resets browse");
    assert_eq!(state.take_input(), "submit resets browse");
    root.draw(&state);
    assert_eq!(root.buffer(), &follow);
}

#[test]
fn c1_c2_giant_markdown_and_single_unbroken_line_seek_only_selected_rows_and_runs() {
    const ROWS: usize = 80_000;
    for unbroken in [false, true] {
        let mut source = String::with_capacity(ROWS * usize::from(CONTENT_WIDTH));
        for row in 0..ROWS {
            if unbroken {
                let token = format!("unbroken_{row:05}_");
                source.push_str(&token);
                source.extend(std::iter::repeat_n(
                    'x',
                    usize::from(CONTENT_WIDTH) - token.len(),
                ));
            } else {
                writeln!(source, "markdown_{row:05} **styled_control** retained multi-megabyte source with hard breaks.  ").unwrap();
            }
        }
        assert!(source.len() > 2 * 1_024 * 1_024);
        assert!(!unbroken || !source.contains('\n'));
        let mut state = new_state(8);
        send(&mut state, agent(source, false));
        state.set_activity(Activity::Idle);
        let mut root = Root::new();
        root.draw(&state);
        assert!(root.retained.layout_metrics().rows > usize::from(u16::MAX));
        let bottom = if unbroken {
            "unbroken_79999_"
        } else {
            "markdown_79999"
        };
        root.contains(&[bottom]);
        unchanged(&mut root, &state, &[bottom], STABLE_DRAWS).selected_window(
            STABLE_DRAWS,
            1,
            WIDTH,
            chat_height(&root, &state),
        );
        for (top, row) in [(0, 0), (40_001, 40_000), (70_001, 70_000)] {
            scroll_to(&root, &mut state, top);
            let before = Work::now();
            root.draw(&state);
            let delta = Work::now().since(before);
            delta.no_derivation();
            delta.selected_window(1, 1, WIDTH, chat_height(&root, &state));
            let marker = if unbroken {
                format!("unbroken_{row:05}_")
            } else {
                format!("markdown_{row:05}")
            };
            root.contains(&[&marker]);
            if !unbroken {
                root.style_control("styled_control", None, Modifier::BOLD);
            }
            unchanged(&mut root, &state, &[&marker], STABLE_DRAWS).selected_window(
                STABLE_DRAWS,
                1,
                WIDTH,
                chat_height(&root, &state),
            );
        }
    }
}

#[test]
fn c3_old_tool_and_receipt_edits_coalesce_without_repreparing_other_sources() {
    let mut state = mixed_history(1_000);
    let mut root = Root::new();
    root.draw(&state);
    state.chat_scroll_up(usize::MAX);
    root.draw(&state);
    root.contains(&["OLDER_TOOL_ORIGINAL", "OLDER_RECEIPT_CONTROL"]);
    let live = state.streaming_text().to_owned();
    let before = Work::now();
    edit_tool(&mut state, "older-tool", "OLDER_TOOL_INTERMEDIATE");
    edit_tool(&mut state, "older-tool", "OLDER_TOOL_FINAL");
    root.draw(&state);
    let delta = Work::now().since(before);
    assert_eq!(
        delta.messages, 1,
        "coalesced old tool must prepare exactly once: {delta:?}"
    );
    assert_eq!(delta.hashes, 0);
    assert_eq!(delta.parses, 0);
    assert_eq!(delta.myers, 0);
    root.contains(&["OLDER_TOOL_FINAL"]);
    root.absent(&["OLDER_TOOL_ORIGINAL", "OLDER_TOOL_INTERMEDIATE"]);
    let before = Work::now();
    send(
        &mut state,
        agent("[STEERING steer-older-receipt: RECEIPT_INTERMEDIATE]", true),
    );
    send(
        &mut state,
        agent("[STEERING steer-older-receipt: RECEIPT_FINAL]", true),
    );
    assert_eq!(
        state.streaming_text(),
        live,
        "receipt-only edit changed live source"
    );
    root.draw(&state);
    let delta = Work::now().since(before);
    assert_eq!(
        delta.messages, 1,
        "coalesced receipt must prepare exactly once: {delta:?}"
    );
    assert_eq!(
        delta.hashes, 0,
        "receipt-only source should not rehash unchanged live text"
    );
    assert_eq!(delta.parses, 0);
    assert_eq!(delta.myers, 0);
    root.contains(&["RECEIPT_FINAL"]);
    root.absent(&["RECEIPT_INTERMEDIATE", "[STEERING"]);
    assert!(state.history().messages().iter().any(|message| matches!(message.kind(), ChatMessageKind::SteerEcho { status: SteerEchoStatus::Applied, note: Some(note), .. } if note == "RECEIPT_FINAL")));
    unchanged(
        &mut root,
        &state,
        &["OLDER_TOOL_FINAL", "RECEIPT_FINAL"],
        STABLE_DRAWS,
    );
}

#[test]
fn c3_append_prune_compaction_clear_reload_and_reused_tool_ids_preserve_sources() {
    let mut state = new_state(3);
    insert_tool(&mut state, "evicted-A", ToolKind::Execute, "A_ORIGINAL");
    state.add_system_message("RETAINED_SENTINEL".into());
    insert_tool(&mut state, "retained-B", ToolKind::Execute, "B_ORIGINAL");
    let mut root = Root::new();
    root.draw(&state);
    let before = Work::now();
    state.add_user_message("APPEND_PRUNE_CONTROL");
    root.draw(&state);
    assert_eq!(
        Work::now().since(before).messages,
        1,
        "prefix retirement must reuse retained prepared blocks"
    );
    root.absent(&["A_ORIGINAL"]);
    root.contains(&["B_ORIGINAL", "APPEND_PRUNE_CONTROL"]);
    assert_eq!(state.history().messages().len(), 3);
    let before = Work::now();
    // Reviewed1v25 treats an unknown update as its own chronological row.
    // It must never reuse A's former index to overwrite retained B.
    edit_tool(&mut state, "evicted-A", "A_LATE_OWN_ROW");
    state.commit_streaming();
    root.draw(&state);
    assert_eq!(Work::now().since(before).messages, 1);
    root.contains(&["B_ORIGINAL", "A_LATE_OWN_ROW"]);
    let b = state
        .history()
        .messages()
        .iter()
        .find_map(|message| match message.kind() {
            ChatMessageKind::ToolCall(call) if call.id().as_str() == "retained-B" => Some(call),
            _ => None,
        })
        .expect("B must survive the stale A update");
    assert_eq!(b.kind(), ToolKind::Execute);
    assert_eq!(b.status(), ToolCallStatus::Completed);
    assert_eq!(b.content(), &[ToolCallContent::Text("B_ORIGINAL".into())]);
    let before = Work::now();
    edit_tool(&mut state, "retained-B", "B_UPDATED_ONCE");
    root.draw(&state);
    let delta = Work::now().since(before);
    assert_eq!(delta.messages, 1, "B must update exactly once: {delta:?}");
    root.contains(&["B_UPDATED_ONCE", "A_LATE_OWN_ROW"]);
    // Activity animation is a separate invalidation lane, not historical work.
    state.set_activity(Activity::Idle);
    root.draw(&state);
    unchanged(&mut root, &state, &["B_UPDATED_ONCE"], STABLE_DRAWS);
    // Repeated prefix retirement crosses metadata compaction, but prepares
    // exactly the one appended block each time, never the retained suffix.
    for index in 0..12 {
        let before = Work::now();
        state.add_user_message(&format!("COMPACTION_APPEND_{index:02}"));
        root.draw(&state);
        assert_eq!(Work::now().since(before).messages, 1);
        assert_eq!(root.retained.layout_metrics().messages, 3);
    }
    state.clear_messages();
    root.draw(&state);
    assert_eq!(root.retained.layout_metrics().messages, 0);
    root.absent(&["B_UPDATED_ONCE", "A_LATE_OWN_ROW", "COMPACTION_APPEND"]);
    insert_tool(
        &mut state,
        "retained-B",
        ToolKind::Execute,
        "RELOADED_B_NEW_EPOCH",
    );
    let before = Work::now();
    root.draw(&state);
    assert_eq!(Work::now().since(before).messages, 1);
    edit_tool(&mut state, "retained-B", "RELOADED_B_UPDATED");
    let before = Work::now();
    root.draw(&state);
    assert_eq!(Work::now().since(before).messages, 1);
    root.contains(&["RELOADED_B_UPDATED"]);
    root.absent(&["B_UPDATED_ONCE", "RELOADED_B_NEW_EPOCH"]);
    // A distinct authoritative history with the same count and local revisions
    // cannot collide with the selected history's identity.
    let mut replacement = new_state(3);
    replacement.add_user_message("DISTINCT_RELOADED_HISTORY");
    let before = Work::now();
    root.draw(&replacement);
    assert_eq!(Work::now().since(before).messages, 1);
    root.contains(&["DISTINCT_RELOADED_HISTORY"]);
    root.absent(&["RELOADED_B_UPDATED"]);
}

#[test]
fn c4_thousand_live_deltas_leave_committed_history_and_independent_blocks_unchanged() {
    let mut state = mixed_history(10_000);
    let mut root = Root::new();
    root.draw(&state);
    let committed = state.history().revision();
    for delta in 0..1_000 {
        let before = Work::now();
        send(&mut state, agent(format!(" live_delta_{delta:04}"), true));
        root.draw(&state);
        let work = Work::now().since(before);
        assert_eq!(
            work.messages, 0,
            "live delta touched committed history at {delta}: {work:?}"
        );
        assert_eq!(
            work.live, 1,
            "exactly changed text block should prepare: {work:?}"
        );
        assert_eq!(
            work.hashes, 1,
            "exactly changed Markdown source should hash: {work:?}"
        );
        assert_eq!(work.myers, 0);
        assert_eq!(state.history().revision(), committed);
    }
    root.contains(&["live_delta_0999"]);
    unchanged(&mut root, &state, &["live_delta_0999"], STABLE_DRAWS);
    let before = Work::now();
    send(
        &mut state,
        Notification::AgentThought(AgentThought {
            text: "INDEPENDENT_THOUGHT_ONE".into(),
        }),
    );
    root.draw(&state);
    let thought = Work::now().since(before);
    assert_eq!(thought.messages, 0);
    assert_eq!(thought.live, 1);
    assert_eq!(thought.hashes, 0, "thought must not rehash live Markdown");
    root.contains(&["INDEPENDENT_THOUGHT_ONE"]);
    root.styled(
        "INDEPENDENT_THOUGHT_ONE",
        state.theme().muted,
        Modifier::ITALIC,
    );
    let before = Work::now();
    send(
        &mut state,
        Notification::AgentThought(AgentThought {
            text: "_TWO".into(),
        }),
    );
    root.draw(&state);
    let thought = Work::now().since(before);
    assert_eq!(thought.messages, 0);
    assert_eq!(thought.live, 1);
    assert_eq!(thought.hashes, 0);
    root.contains(&["INDEPENDENT_THOUGHT_ONE_TWO"]);
    let before = Work::now();
    state.insert_text("input-only redraw");
    root.draw(&state);
    Work::now().since(before).no_derivation();
    root.contains(&["input-only redraw", "INDEPENDENT_THOUGHT_ONE_TWO"]);
    state.set_activity(Activity::Waiting);
    for frame in 0..STABLE_DRAWS {
        let elapsed = std::time::Duration::from_millis(
            u64::try_from(crate::spinner::SPINNER_FRAME_MS).unwrap() * frame as u64,
        );
        state.set_activity_elapsed_for_test(elapsed);
        let before = Work::now();
        root.draw(&state);
        let spinner = Work::now().since(before);
        assert_eq!(
            spinner.messages, 0,
            "spinner re-prepared history: {spinner:?}"
        );
        assert_eq!(
            spinner.live, 0,
            "spinner re-prepared text/thought: {spinner:?}"
        );
        assert_eq!(spinner.hashes, 0);
        assert_eq!(spinner.parses, 0);
        assert_eq!(spinner.myers, 0);
        assert!(
            spinner.wraps <= 1,
            "only the activity block may change: {spinner:?}"
        );
        root.contains(&["Thinking...", "INDEPENDENT_THOUGHT_ONE_TWO"]);
    }
    let before_count = state.history().messages().len();
    insert_tool(
        &mut state,
        "flush-boundary",
        ToolKind::Execute,
        "TOOL_BOUNDARY_CONTROL",
    );
    root.draw(&state);
    assert_eq!(
        state.history().messages().len(),
        before_count,
        "flush must honor the actual retention limit"
    );
    let tail = &state.history().messages()[before_count - 3..];
    assert!(
        matches!(tail[0].kind(), ChatMessageKind::AgentText(text) if text.ends_with("live_delta_0999"))
    );
    assert!(
        matches!(tail[1].kind(), ChatMessageKind::Thought(text) if text == "INDEPENDENT_THOUGHT_ONE_TWO")
    );
    assert!(
        matches!(tail[2].kind(), ChatMessageKind::ToolCall(call) if call.id().as_str() == "flush-boundary")
    );
    assert!(state.streaming_text().is_empty());
    assert!(state.streaming_thought().is_none());
    root.contains(&["TOOL_BOUNDARY_CONTROL", "INDEPENDENT_THOUGHT_ONE_TWO"]);
}

#[test]
fn c4_focus_completion_removal_and_occupied_workflow_adoption_do_not_leak_layouts() {
    let mut state = mixed_history(100);
    let mut root = Root::new();
    root.draw(&state);
    let sid = SessionId::new("qualification-child");
    state.apply_subagent_notification(&sid, &agent("CHILD_OLD_COMMITTED", false));
    state.apply_subagent_notification(&sid, &agent("CHILD_LIVE_CONTROL", true));
    state.chat_scroll_up(usize::MAX);
    assert!(state.focus_subagent(sid.clone()));
    root.draw(&state);
    root.contains(&[
        "qualification-child",
        "[Esc] Back",
        "CHILD_OLD_COMMITTED",
        "CHILD_LIVE_CONTROL",
    ]);
    root.absent(&["OLDER_TOOL_ORIGINAL", "VISIBLE_AGENT_CONTROL"]);
    unchanged(&mut root, &state, &["CHILD_LIVE_CONTROL"], STABLE_DRAWS);
    let info = cyril_core::types::SubagentInfo::new(
        sid.clone(),
        "RENAMED_CHILD_CONTROL",
        "qualification-agent",
        "fixture query",
        cyril_core::types::SubagentStatus::Working { message: None },
    );
    let name_update = Notification::SubagentListUpdated {
        subagents: vec![info],
        pending_stages: Vec::new(),
    };
    let before = Work::now();
    state.apply_subagent_tracker_notification(&name_update);
    if let Notification::SubagentListUpdated { subagents, .. } = &name_update {
        state.apply_subagent_list_update(subagents);
    }
    root.draw(&state);
    let renamed = Work::now().since(before);
    assert_eq!(renamed.messages, 0);
    assert_eq!(
        renamed.live, 0,
        "name-only changes must reuse live Markdown"
    );
    assert_eq!(renamed.hashes, 0);
    assert_eq!(renamed.parses, 0);
    root.contains(&["RENAMED_CHILD_CONTROL", "CHILD_LIVE_CONTROL"]);
    state.apply_subagent_notification(
        &sid,
        &Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        },
    );
    root.draw(&state);
    root.contains(&["CHILD_OLD_COMMITTED", "CHILD_LIVE_CONTROL"]);
    assert_eq!(
        root.text().matches("CHILD_LIVE_CONTROL").count(),
        1,
        "child completion duplicated live text"
    );
    state.apply_subagent_list_update(&[]);
    root.draw(&state);
    root.contains(&["CHILD_LIVE_CONTROL"]);
    state.unfocus_subagent();
    state.chat_scroll_reset();
    root.draw(&state);
    root.contains(VISIBLE);
    root.absent(&["CHILD_OLD_COMMITTED", "CHILD_LIVE_CONTROL"]);

    // Occupied adoption prepends the earlier child history to workflow storage;
    // workflows remain storage-only and adoption clears selected child focus.
    state.apply_workflow_notification(&sid, &agent("WORKFLOW_LATER_CONTROL", false));
    assert!(state.focus_subagent(sid.clone()));
    root.draw(&state);
    assert!(state.claim_stream_for_workflow(&sid));
    let adopted = state.workflow_streams()[&sid].history().messages();
    assert_eq!(adopted.len(), 3);
    assert!(
        matches!(adopted[0].kind(), ChatMessageKind::AgentText(text) if text == "CHILD_OLD_COMMITTED")
    );
    assert!(
        matches!(adopted[1].kind(), ChatMessageKind::AgentText(text) if text == "CHILD_LIVE_CONTROL")
    );
    assert!(
        matches!(adopted[2].kind(), ChatMessageKind::AgentText(text) if text == "WORKFLOW_LATER_CONTROL")
    );
    assert!(state.subagent_ui().focused_session_id().is_none());
    root.draw(&state);
    root.contains(VISIBLE);
    root.absent(&[
        "CHILD_OLD_COMMITTED",
        "CHILD_LIVE_CONTROL",
        "WORKFLOW_LATER_CONTROL",
    ]);
    // Reusing the same session id creates a new selected history identity.
    state.apply_subagent_notification(&sid, &agent("RECREATED_CHILD_CONTROL", false));
    assert!(state.focus_subagent(sid));
    root.draw(&state);
    root.contains(&["RECREATED_CHILD_CONTROL"]);
    root.absent(&[
        "CHILD_OLD_COMMITTED",
        "WORKFLOW_LATER_CONTROL",
        "VISIBLE_AGENT_CONTROL",
    ]);
}

#[test]
fn c5_root_width_height_and_structural_theme_invalidations_are_explicit() {
    // Shared-Arc identity/eviction is covered by markdown's existing isolated
    // cache tests (five_hundred_markdown_cache_hits_share_the_same_allocation
    // and retained_markdown_result_survives_later_renders_and_eviction).
    // Do not race an extra global-cache pointer assertion against these large
    // histories when the ordinary suite runs with parallel test threads.
    let mut state = mixed_history(1_000);
    let mut root = Root::new();
    root.draw(&state);
    let original = root.buffer().clone();
    for (width, height, rebuild) in [
        (80, 48, true),
        (160, 48, true),
        (160, 12, false),
        (160, 48, false),
    ] {
        root.resize(&mut state, width, height);
        let before = Work::now();
        root.draw(&state);
        let delta = Work::now().since(before);
        if rebuild {
            assert_eq!(
                delta.messages, 1_000,
                "width change is a declared bulk invalidation: {delta:?}"
            );
            assert!(delta.hashes > 256);
            assert!(delta.wraps >= 1_000);
        } else {
            delta.no_derivation();
        }
        root.contains(&["VISIBLE_EXECUTE_CONTROL"]);
        unchanged(
            &mut root,
            &state,
            &["VISIBLE_EXECUTE_CONTROL"],
            STABLE_DRAWS,
        );
    }
    assert_eq!(
        root.buffer(),
        &original,
        "160/80/160 and height-only cycle changed reference cells"
    );
    for (theme, color) in [
        (ThemeId::CyrilDark, ColorMode::None),
        (ThemeId::CyrilLight, ColorMode::None),
        (ThemeId::CyrilLight, ColorMode::TrueColor),
        (ThemeId::CyrilDark, ColorMode::TrueColor),
    ] {
        let previous_theme = state.theme();
        state.set_appearance(theme, color);
        let before = Work::now();
        root.draw(&state);
        let delta = Work::now().since(before);
        // No-color palettes resolve identically; a real structural change is
        // a bulk invalidation, while an identical resolved style is warm.
        assert_eq!(
            delta.messages,
            if previous_theme == state.theme() {
                0
            } else {
                1_000
            },
            "structural theme invalidation: {delta:?}"
        );
        root.contains(VISIBLE);
        root.styled("You:", state.theme().user, Modifier::BOLD);
        root.styled("Kiro:", state.theme().agent, Modifier::BOLD);
        root.style_control("bold_control", None, Modifier::BOLD);
        root.styled(
            "VISIBLE_THOUGHT_CONTROL",
            state.theme().muted,
            Modifier::ITALIC,
        );
        unchanged(&mut root, &state, VISIBLE, STABLE_DRAWS);
    }
    assert_eq!(
        root.buffer(),
        &original,
        "return to truecolor/theme changed source cells/styles"
    );
    // Zero width and zero height must not consume mutation invalidations.
    for (width, height) in [(0, HEIGHT), (WIDTH, 0), (0, 0)] {
        root.resize(&mut state, width, height);
        root.draw(&state);
        let marker = format!("AFTER_ZERO_GEOMETRY_{width}x{height}_CONTROL");
        state.add_user_message(&marker);
        root.resize(&mut state, WIDTH, HEIGHT);
        root.draw(&state);
        root.contains(&[&marker]);
    }
    unchanged(
        &mut root,
        &state,
        &["AFTER_ZERO_GEOMETRY_0x0_CONTROL"],
        STABLE_DRAWS,
    );
}

#[test]
fn c2_growing_input_preserves_the_real_three_row_chat_floor_without_preparation() {
    let mut state = new_state(8);
    let mut source = String::new();
    for row in 0..100 {
        writeln!(source, "geometry_row_{row:03}").unwrap();
    }
    state.add_user_message(&source);
    let mut root = Root::new();
    root.draw(&state);
    let metrics = root.retained.layout_metrics();
    let mut draft = String::new();
    for row in 0..100 {
        writeln!(draft, "input_draft_{row:03}").unwrap();
    }
    state.insert_text(&draft);
    root.resize(&mut state, WIDTH, 12);
    let before = Work::now();
    root.draw(&state);
    let delta = Work::now().since(before);
    delta.no_derivation();
    assert_eq!(
        delta.rows, 3,
        "growing input must leave the actual chat floor"
    );
    assert_eq!(render::input_top(Rect::new(0, 0, WIDTH, 12), &state), 4);
    root.contains(&["geometry_row_099", "input_draft_099"]);
    assert_eq!(root.retained.layout_metrics(), metrics);
    state.take_input();
    root.resize(&mut state, WIDTH, HEIGHT);
    let before = Work::now();
    root.draw(&state);
    Work::now().since(before).no_derivation();
    root.contains(&["geometry_row_099"]);
}

#[test]
fn c3_coalesced_old_write_content_rebuilds_exactly_one_block_and_one_real_myers() {
    let mut state = new_state(1_000);
    insert_tool(&mut state, "old-write", ToolKind::Write, "");
    for index in 1..999 {
        send(
            &mut state,
            agent(
                format!("unrelated_markdown_{index:04} **untouched**"),
                false,
            ),
        );
    }
    state.add_user_message("WRITE_TEST_FOLLOW_CONTROL");
    state.set_activity(Activity::Idle);
    let mut root = Root::new();
    root.draw(&state);
    root.contains(&["WRITE_TEST_FOLLOW_CONTROL"]);
    state.chat_scroll_up(usize::MAX);
    root.draw(&state);
    root.contains(&["const OLD_CONTROL", "const NEW_CONTROL"]);
    let before = Work::now();
    for new in [
        "const INTERMEDIATE_WRITE: u64 = 3;\n",
        "const UPDATED_WRITE_CONTROL: u64 = 4;\n",
    ] {
        send(
            &mut state,
            Notification::ToolCallUpdated(
                ToolCall::new(
                    ToolCallId::new("old-write"),
                    String::new(),
                    ToolKind::Write,
                    ToolCallStatus::Completed,
                    None,
                )
                .with_content(vec![ToolCallContent::Diff {
                    path: "fixture_old-write.rs".into(),
                    old_text: Some("const OLD_CONTROL: u64 = 1;\n".into()),
                    new_text: new.into(),
                }]),
            ),
        );
    }
    root.draw(&state);
    let delta = Work::now().since(before);
    assert_eq!(
        delta.messages, 1,
        "old Write edits must coalesce: {delta:?}"
    );
    assert_eq!(
        delta.myers, 1,
        "derive only the final changed diff once: {delta:?}"
    );
    assert_eq!(
        delta.hashes, 0,
        "unrelated historical Markdown was revisited"
    );
    assert_eq!(delta.parses, 0);
    root.contains(&["const UPDATED_WRITE_CONTROL"]);
    root.absent(&["const INTERMEDIATE_WRITE", "const NEW_CONTROL"]);
    root.styled(
        "const UPDATED_WRITE_CONTROL",
        state.theme().subdued_positive,
        Modifier::empty(),
    );
    unchanged(
        &mut root,
        &state,
        &["const UPDATED_WRITE_CONTROL"],
        STABLE_DRAWS,
    );
}

#[test]
fn c2_c4_focused_child_follows_beyond_u16_and_ignores_main_browse() {
    let mut state = new_state(8);
    state.add_user_message("MAIN_SOURCE_MUST_NOT_LEAK");
    let sid = SessionId::new("deep-child");
    let mut source = String::new();
    for row in 0..80_000 {
        writeln!(source, "child_row_{row:05}  ").unwrap();
    }
    state.apply_subagent_notification(&sid, &agent(source, false));
    state.chat_scroll_up(usize::MAX);
    assert!(state.focus_subagent(sid));
    let mut root = Root::new();
    root.draw(&state);
    assert!(root.retained.layout_metrics().rows > usize::from(u16::MAX));
    root.contains(&["child_row_79999"]);
    root.absent(&["child_row_00000", "MAIN_SOURCE_MUST_NOT_LEAK"]);
    unchanged(&mut root, &state, &["child_row_79999"], STABLE_DRAWS).selected_window(
        STABLE_DRAWS,
        1,
        WIDTH,
        chat_height(&root, &state),
    );
    state.unfocus_subagent();
    state.chat_scroll_reset();
    root.draw(&state);
    root.contains(&["MAIN_SOURCE_MUST_NOT_LEAK"]);
    root.absent(&["child_row_79999"]);
}

#[test]
fn c1_portable_baseline_giant_middle_window_does_not_scan_hidden_prefix() {
    const ROWS: usize = 80_000;
    let mut source = String::new();
    for row in 0..ROWS {
        writeln!(
            source,
            "portable_row_{row:05} authoritative giant-message source retained without clipping"
        )
        .unwrap();
    }
    assert!(source.len() > 2 * 1_024 * 1_024);
    let mut state = new_state(8);
    state.add_user_message(&source);
    // The portable RED lane deliberately selects a middle row below u16::MAX:
    // the old renderer must draw the correct source before work can falsify it.
    state.chat_scroll_up(usize::MAX);
    let mut root = Root::new();
    let before = portable_operations();
    root.draw(&state);
    let after = portable_operations();
    let cold = std::array::from_fn::<_, 5, _>(|index| after[index] - before[index]);
    assert_eq!(
        cold[0], 1,
        "giant source must be one actual prepared message"
    );
    assert!(cold[3] > 0);
    root.contains(&["You:", "portable_row_00000"]);
    let chat_height = usize::from(render::input_top(Rect::new(0, 0, WIDTH, HEIGHT), &state) - 1);
    // The source is 80,000 short plain rows, one user label and one separator.
    // This is fixture arithmetic, not an alternate renderer/reference oracle.
    state.chat_scroll_reset();
    state.chat_scroll_up(
        (ROWS + 2)
            .saturating_sub(chat_height)
            .saturating_sub(40_001),
    );
    root.draw(&state);
    root.contains(&["portable_row_40000"]);
    root.absent(&["portable_row_00000", "portable_row_79999"]);
    eprintln!(
        "C6LA_BASELINE_CONTROLS_EXECUTED case=giant-middle retained_prepared={} actual_wraps={} selected_row=40001 stable_draws={STABLE_DRAWS}",
        cold[0], cold[3]
    );
    portable_unchanged(&mut root, &state, &["portable_row_40000"]);
}

#[test]
fn c3_occupied_adoption_rebases_both_tool_indices_without_invalidating_main_layout() {
    let mut state = new_state(8);
    state.add_user_message("MAIN_ADOPTION_CONTROL");
    let sid = SessionId::new("occupied-adoption-tools");
    state.apply_subagent_notification(
        &sid,
        &Notification::ToolCallStarted(tool(
            "early-tool",
            ToolKind::Execute,
            "EARLY_TOOL_ORIGINAL",
        )),
    );
    state.apply_subagent_notification(&sid, &agent("EARLY_AGENT_CONTROL", false));
    state.apply_workflow_notification(
        &sid,
        &Notification::ToolCallStarted(tool(
            "later-tool",
            ToolKind::Execute,
            "LATER_TOOL_ORIGINAL",
        )),
    );
    state.apply_workflow_notification(&sid, &agent("LATER_AGENT_CONTROL", false));
    assert!(state.focus_subagent(sid.clone()));
    let mut root = Root::new();
    root.draw(&state);
    root.contains(&["EARLY_TOOL_ORIGINAL", "EARLY_AGENT_CONTROL"]);
    assert!(state.claim_stream_for_workflow(&sid));
    root.draw(&state);
    root.contains(&["MAIN_ADOPTION_CONTROL"]);
    root.absent(&[
        "EARLY_TOOL_ORIGINAL",
        "EARLY_AGENT_CONTROL",
        "LATER_AGENT_CONTROL",
    ]);
    let before = Work::now();
    for (id, text) in [
        ("early-tool", "EARLY_TOOL_UPDATED"),
        ("later-tool", "LATER_TOOL_UPDATED"),
    ] {
        state.apply_workflow_notification(
            &sid,
            &Notification::ToolCallUpdated(
                ToolCall::new(
                    ToolCallId::new(id),
                    String::new(),
                    ToolKind::Execute,
                    ToolCallStatus::Completed,
                    None,
                )
                .with_content(vec![ToolCallContent::Text(text.into())])
                .with_raw_output(Some(serde_json::json!(text))),
            ),
        );
    }
    root.draw(&state);
    Work::now().since(before).no_derivation();
    root.contains(&["MAIN_ADOPTION_CONTROL"]);
    root.absent(&["EARLY_TOOL_UPDATED", "LATER_TOOL_UPDATED"]);
    let history = state.workflow_streams()[&sid].history().messages();
    assert_eq!(
        history.len(),
        4,
        "updates must not duplicate either adopted tool"
    );
    for (index, id, output) in [
        (0, "early-tool", "EARLY_TOOL_UPDATED"),
        (2, "later-tool", "LATER_TOOL_UPDATED"),
    ] {
        assert!(
            matches!(history[index].kind(), ChatMessageKind::ToolCall(call)
            if call.id().as_str() == id && call.status() == ToolCallStatus::Completed
                && call.content() == [ToolCallContent::Text(output.into())])
        );
    }
    assert!(
        matches!(history[1].kind(), ChatMessageKind::AgentText(text) if text == "EARLY_AGENT_CONTROL")
    );
    assert!(
        matches!(history[3].kind(), ChatMessageKind::AgentText(text) if text == "LATER_AGENT_CONTROL")
    );
}

#[test]
fn review_hidden_diff_blocks_are_not_reselected_by_unchanged_root_draws() {
    const CONTROLS: &[&str] = &[
        "✓ Edit(review_hidden_diff.rs)  +1 -1",
        "│- const REVIEW_OLD_DIFF",
        "│+ const REVIEW_NEW_DIFF",
    ];
    let mut expected_cells = None;
    let mut workloads = Vec::new();
    for hidden_blocks in [0, 256, 4_096] {
        let mut content = Vec::with_capacity(hidden_blocks + 1);
        for index in 0..hidden_blocks {
            content.push(ToolCallContent::Text(format!(
                "REVIEW_HIDDEN_TEXT_{index:04}"
            )));
        }
        content.push(ToolCallContent::Diff {
            path: "review_hidden_diff.rs".into(),
            old_text: Some("const REVIEW_OLD_DIFF: u64 = 1;\n".into()),
            new_text: "const REVIEW_NEW_DIFF: u64 = 2;\n".into(),
        });
        let mut state = new_state(8);
        send(
            &mut state,
            Notification::ToolCallStarted(
                ToolCall::new(
                    ToolCallId::new("review-hidden-diff"),
                    "Review hidden Diff".into(),
                    ToolKind::Write,
                    ToolCallStatus::Completed,
                    Some(serde_json::json!({"path": "review_hidden_diff.rs"})),
                )
                .with_content(content),
            ),
        );
        state.commit_streaming();
        state.set_activity(Activity::Idle);
        let mut root = Root::new();
        let before = Work::now();
        let selection_before = crate::traits::diff_selection_steps();
        root.draw(&state);
        let cold = Work::now().since(before);
        let cold_selection = crate::traits::diff_selection_steps() - selection_before;
        root.contains(CONTROLS);
        root.absent(&["REVIEW_HIDDEN_TEXT"]);
        root.styled(
            "const REVIEW_OLD_DIFF",
            state.theme().subdued_negative,
            Modifier::empty(),
        );
        root.styled(
            "const REVIEW_NEW_DIFF",
            state.theme().subdued_positive,
            Modifier::empty(),
        );
        assert_eq!(cold.messages, 1, "completed Write must actually prepare");
        assert_eq!(
            cold.myers, 1,
            "completed Write must construct its real diff"
        );
        assert!(cold.wraps > 0, "actual wrapping did not execute: {cold:?}");
        assert!(
            cold_selection > hidden_blocks,
            "actual first-Diff selection did not visit the fixture: {cold_selection}"
        );
        if let Some(expected) = &expected_cells {
            assert_eq!(
                root.buffer(),
                expected,
                "hidden content changed the visible summary, rows or styles"
            );
        } else {
            expected_cells = Some(root.buffer().clone());
        }
        let before = Work::now();
        let selection_before = crate::traits::diff_selection_steps();
        for _ in 0..STABLE_DRAWS {
            root.draw(&state);
        }
        let warm = Work::now().since(before);
        let warm_selection = crate::traits::diff_selection_steps() - selection_before;
        root.contains(CONTROLS);
        root.absent(&["REVIEW_HIDDEN_TEXT"]);
        assert!(warm.rows > 0 && warm.runs > 0, "empty warm paint: {warm:?}");
        eprintln!(
            "C6LA_REVIEW_CONTROLS_EXECUTED case=hidden-diff hidden_text_blocks={hidden_blocks} diff_blocks=1 stable_draws={STABLE_DRAWS} cold_selection_visits={cold_selection} warm_selection_visits={warm_selection} cold={cold:?} warm={warm:?}"
        );
        workloads.push((hidden_blocks, warm, warm_selection));
    }
    // Exercise every hidden-prefix size and print its real controls before
    // asserting the work bound, so an early RED cannot hide a later workload.
    for (hidden_blocks, warm, warm_selection) in workloads {
        assert_eq!(
            warm_selection, 0,
            "unchanged visible Diff rows rescanned {hidden_blocks} hidden Text blocks"
        );
        warm.no_derivation();
        warm.selected_window(STABLE_DRAWS, 1, WIDTH, HEIGHT);
    }
}

fn draw_with_fixed_activity_clock(root: &mut Root, state: &mut UiState) {
    state.set_activity_elapsed_for_test(std::time::Duration::from_secs(3));
    root.draw(state);
}

fn warmed_live_receipt(id: &str) -> (UiState, Root, String, Work, Work) {
    const PARAGRAPHS: usize = 4_096;
    let mut source = format!("# REVIEW_LIVE_HEAD {id}\n\n**REVIEW_LIVE_BOLD**\n\n");
    for row in 0..PARAGRAPHS {
        writeln!(
            source,
            "review_live_{row:04} **retained emphasis** with authoritative Markdown bytes that must remain prepared.\n"
        )
        .unwrap();
    }
    source.push_str("REVIEW_LIVE_TAIL\n");
    assert!(source.len() > 256 * 1_024, "live source must be nontrivial");
    let mut state = new_state(8);
    state.add_steer_echo("REVIEW_RECEIPT_ECHO");
    send(
        &mut state,
        Notification::SteeringQueued {
            message: None,
            message_id: Some(id.into()),
        },
    );
    send(&mut state, agent(source.clone(), true));
    assert_eq!(state.streaming_text().as_bytes(), source.as_bytes());
    assert_receipt_chip(&state, id, SteerEchoStatus::Queued, None);
    // Browse the earlier chip and the nonempty live head together, rather than
    // letting a giant live tail hide all reconciliation/source controls.
    state.chat_scroll_up(usize::MAX);
    let mut root = Root::new();
    let before = Work::now();
    draw_with_fixed_activity_clock(&mut root, &mut state);
    let cold = Work::now().since(before);
    root.contains(&[
        "REVIEW_RECEIPT_ECHO",
        "queued",
        "Kiro:",
        "REVIEW_LIVE_HEAD",
        "REVIEW_LIVE_BOLD",
    ]);
    root.style_control("REVIEW_LIVE_BOLD", None, Modifier::BOLD);
    assert_eq!(cold.messages, 1, "the bound chip must actually prepare");
    assert!(cold.live > 0, "nonempty live preparation did not execute");
    assert_eq!(cold.hashes, 1, "the large live Markdown must actually hash");
    assert!(cold.wraps > 0, "the large live Markdown must actually wrap");
    let before = Work::now();
    draw_with_fixed_activity_clock(&mut root, &mut state);
    let warm = Work::now().since(before);
    root.contains(&[
        "REVIEW_RECEIPT_ECHO",
        "REVIEW_LIVE_HEAD",
        "REVIEW_LIVE_BOLD",
    ]);
    assert!(warm.rows > 0 && warm.runs > 0, "empty warm paint: {warm:?}");
    (state, root, source, cold, warm)
}

fn assert_receipt_chip(
    state: &UiState,
    id: &str,
    expected_status: SteerEchoStatus,
    expected_note: Option<&str>,
) {
    assert_eq!(state.history().messages().len(), 1);
    match state.history().messages()[0].kind() {
        ChatMessageKind::SteerEcho {
            text,
            status,
            message_id,
            note,
        } => {
            assert_eq!(text, "REVIEW_RECEIPT_ECHO");
            assert_eq!(*status, expected_status);
            assert_eq!(message_id.as_deref(), Some(id));
            assert_eq!(note.as_deref(), expected_note);
        }
        other => panic!("receipt fixture lost its real bound chip: {other:?}"),
    }
    assert_eq!(
        state.steering_queued(),
        usize::from(expected_status == SteerEchoStatus::Queued)
    );
}

fn receipt_only_work(work: Work) {
    assert_eq!(
        work.messages, 1,
        "only the reconciled chip may prepare: {work:?}"
    );
    assert_eq!(
        work.live, 0,
        "unchanged live Markdown was prepared: {work:?}"
    );
    assert_eq!(
        work.hashes, 0,
        "unchanged live Markdown was hashed: {work:?}"
    );
    assert_eq!(
        work.parses, 0,
        "unchanged live Markdown was parsed: {work:?}"
    );
    assert_eq!(work.wraps, 1, "only the reconciled chip may wrap: {work:?}");
    assert_eq!(work.myers, 0);
}

#[test]
fn review_complete_receipt_keeps_large_live_markdown_prepared() {
    const ID: &str = "steer-review-complete";
    const NOTE: &str = "REVIEW_COMPLETE_NOTE";
    let (mut state, mut root, source, cold, warm) = warmed_live_receipt(ID);
    let before = Work::now();
    send(&mut state, agent(format!("[STEERING {ID}: {NOTE}]"), true));
    assert_eq!(
        state.streaming_text().as_bytes(),
        source.as_bytes(),
        "a complete receipt changed the displayed source bytes"
    );
    assert_receipt_chip(&state, ID, SteerEchoStatus::Applied, Some(NOTE));
    draw_with_fixed_activity_clock(&mut root, &mut state);
    let receipt = Work::now().since(before);
    root.contains(&[
        "REVIEW_RECEIPT_ECHO",
        "applied",
        NOTE,
        "REVIEW_LIVE_HEAD",
        "REVIEW_LIVE_BOLD",
    ]);
    root.absent(&["[STEERING"]);
    assert!(
        receipt.rows > 0 && receipt.runs > 0,
        "empty receipt paint: {receipt:?}"
    );
    eprintln!(
        "C6LA_REVIEW_CONTROLS_EXECUTED case=complete-receipt live_paragraphs=4096 live_bytes={} marker_bytes={} cold={cold:?} warm={warm:?} receipt={receipt:?}",
        source.len(),
        format!("[STEERING {ID}: {NOTE}]").len(),
    );
    warm.no_derivation();
    receipt_only_work(receipt);
}

fn withheld_receipt_chunks(id: &str, chunks: &[&str], note: &str) {
    let (mut state, mut root, source, cold, warm) = warmed_live_receipt(id);
    let mut emitted = String::new();
    let mut withheld_work = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        emitted.push_str(chunk);
        let before = Work::now();
        send(&mut state, agent(*chunk, true));
        assert_eq!(
            state.streaming_text().as_bytes(),
            source.as_bytes(),
            "incomplete receipt escaped withholding at chunk {index}: {emitted:?}"
        );
        assert_receipt_chip(&state, id, SteerEchoStatus::Queued, None);
        draw_with_fixed_activity_clock(&mut root, &mut state);
        let work = Work::now().since(before);
        root.contains(&[
            "REVIEW_RECEIPT_ECHO",
            "queued",
            "REVIEW_LIVE_HEAD",
            "REVIEW_LIVE_BOLD",
        ]);
        root.absent(&["[STEERING", note]);
        assert!(
            work.rows > 0 && work.runs > 0,
            "empty chunk paint: {work:?}"
        );
        eprintln!(
            "C6LA_REVIEW_CONTROLS_EXECUTED case=withheld-receipt id={id} phase=incomplete chunk_index={index} chunk_bytes={} marker_bytes={} live_paragraphs=4096 live_bytes={} cold={cold:?} warm={warm:?} chunk={work:?}",
            chunk.len(),
            emitted.len(),
            source.len(),
        );
        withheld_work.push(work);
    }
    assert_eq!(emitted, format!("[STEERING {id}: {note}"));
    let before = Work::now();
    send(&mut state, agent("]", true));
    assert_eq!(state.streaming_text().as_bytes(), source.as_bytes());
    assert_receipt_chip(&state, id, SteerEchoStatus::Applied, Some(note));
    draw_with_fixed_activity_clock(&mut root, &mut state);
    let completed = Work::now().since(before);
    root.contains(&[
        "REVIEW_RECEIPT_ECHO",
        "applied",
        note,
        "REVIEW_LIVE_HEAD",
        "REVIEW_LIVE_BOLD",
    ]);
    root.absent(&["[STEERING"]);
    assert!(
        completed.rows > 0 && completed.runs > 0,
        "empty completed receipt paint: {completed:?}"
    );
    eprintln!(
        "C6LA_REVIEW_CONTROLS_EXECUTED case=withheld-receipt id={id} phase=completed chunks={} marker_bytes={} live_paragraphs=4096 live_bytes={} completed={completed:?}",
        chunks.len() + 1,
        emitted.len() + 1,
        source.len(),
    );
    // All chunks, withholding controls and final status/note reconciliation
    // execute before any zero-work bound is asserted.
    warm.no_derivation();
    for work in withheld_work {
        work.no_derivation();
    }
    receipt_only_work(completed);
}

#[test]
fn review_chunked_receipt_opening_keeps_large_live_markdown_prepared() {
    withheld_receipt_chunks(
        "steer-review-opening",
        &[
            "[",
            "S",
            "TEER",
            "ING",
            " ",
            "st",
            "eer-",
            "review-opening",
            ": ",
            "REVIEW_OPENING_NOTE",
        ],
        "REVIEW_OPENING_NOTE",
    );
}

#[test]
fn review_chunked_receipt_body_keeps_large_live_markdown_prepared() {
    withheld_receipt_chunks(
        "steer-review-body",
        &[
            "[STEERING steer-review-body: ",
            "REVIEW_BODY_NOTE",
            " kept ",
            "the visible Markdown",
            " unchanged.",
        ],
        "REVIEW_BODY_NOTE kept the visible Markdown unchanged.",
    );
}

#[test]
fn review_nested_receipt_visible_prefix_semantics() {
    const ID: &str = "steer-older-receipt";
    let equal_before = "A[STEERING steer-older-receipt: [x";
    let replacement = "z".repeat(equal_before.len() - 1);
    let equal_after = format!("A{replacement}");
    assert_eq!(equal_before.len(), equal_after.len());
    assert_ne!(equal_before, equal_after);
    let mut workloads = Vec::new();
    for (case, raw_before, displayed_before, append, displayed_after, note) in [
        (
            "equal-length-visible-replacement",
            equal_before,
            equal_before,
            format!("]{replacement}"),
            equal_after,
            "[x",
        ),
        (
            "earlier-prefix-becomes-withheld",
            "[STEER[",
            "[STEER",
            "STEERING steer-older-receipt: n]".into(),
            String::new(),
            "n",
        ),
    ] {
        let mut state = new_state(8);
        state.add_steer_echo("REVIEW_RECEIPT_ECHO");
        send(
            &mut state,
            Notification::SteeringQueued {
                message: None,
                message_id: Some(ID.into()),
            },
        );
        send(&mut state, agent(raw_before, true));
        assert_eq!(
            state.streaming_text().as_bytes(),
            displayed_before.as_bytes()
        );
        assert_receipt_chip(&state, ID, SteerEchoStatus::Queued, None);
        let mut root = Root::new();
        let before = Work::now();
        draw_with_fixed_activity_clock(&mut root, &mut state);
        let cold = Work::now().since(before);
        root.contains(&["REVIEW_RECEIPT_ECHO", "queued", "Kiro:", displayed_before]);
        assert!(
            cold.live > 0,
            "initial live source did not prepare: {cold:?}"
        );
        assert_eq!(cold.hashes, 1, "initial live source did not hash: {cold:?}");
        assert!(cold.wraps > 0, "initial live source did not wrap: {cold:?}");
        let before = Work::now();
        draw_with_fixed_activity_clock(&mut root, &mut state);
        let warm = Work::now().since(before);
        root.contains(&["REVIEW_RECEIPT_ECHO", "Kiro:", displayed_before]);

        // Legacy harvest can remove previously displayed bytes when a note
        // contains '['. Its splice can also expose an earlier plausible prefix;
        // neither a length-only key nor a suffix-only cutoff preserves this.
        let before = Work::now();
        send(&mut state, agent(append.clone(), true));
        assert_eq!(
            state.streaming_text().as_bytes(),
            displayed_after.as_bytes(),
            "legacy nested receipt standing changed: {case}"
        );
        assert_receipt_chip(&state, ID, SteerEchoStatus::Applied, Some(note));
        draw_with_fixed_activity_clock(&mut root, &mut state);
        let changed = Work::now().since(before);
        root.contains(&["REVIEW_RECEIPT_ECHO", "applied"]);
        root.absent(&[displayed_before]);
        if displayed_after.is_empty() {
            root.contains(&["⤷ n"]);
            root.absent(&["Kiro:", "[STEER"]);
        } else {
            root.contains(&["Kiro:", &displayed_after, "⤷ [x"]);
        }
        assert!(
            changed.rows > 0 && changed.runs > 0,
            "nested receipt standing did not paint: {changed:?}"
        );
        eprintln!(
            "C6LA_REVIEW_CONTROLS_EXECUTED case={case} raw_before_bytes={} displayed_before_bytes={} append_bytes={} displayed_after_bytes={} cold={cold:?} warm={warm:?} changed={changed:?}",
            raw_before.len(),
            displayed_before.len(),
            append.len(),
            displayed_after.len(),
        );
        workloads.push((case, warm, changed, !displayed_after.is_empty()));
    }
    // These are positive behavioral controls, not the receipt-only RED lane:
    // both updates really change the displayed source and must prepare it.
    for (case, warm, changed, nonempty_after) in workloads {
        warm.no_derivation();
        assert_eq!(changed.messages, 1, "receipt chip must reconcile: {case}");
        assert_eq!(
            changed.live, 1,
            "changed live source was not prepared: {case}"
        );
        assert_eq!(
            changed.hashes,
            usize::from(nonempty_after),
            "only a nonempty changed source requires Markdown hashing: {case}"
        );
        assert!(changed.wraps > 0, "changed source was not wrapped: {case}");
        assert_eq!(changed.myers, 0);
        if !nonempty_after {
            assert_eq!(changed.parses, 0, "empty source was parsed: {case}");
        }
    }
}
