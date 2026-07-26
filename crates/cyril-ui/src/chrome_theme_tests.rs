//! cyril-dij8 chrome theme fences: frozen-baseline equivalence, marker-role
//! wiring, and no-color guarantees for the application chrome (toolbar,
//! status bar, crew panel, voice indicator).
//!
//! The 18 scenes reproduce the prove-it probe's 13 branch-maximal scenarios
//! (.cyril-dij8/probe-styles.txt) plus 5 edge scenes. Normalization helpers
//! mirror the conversation fences in `render.rs`; consolidating that test
//! plumbing is cyril-xv3e.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::prelude::*;

use cyril_core::types::{
    ContextBreakdown, ContextBucket, LoopState, Notification, PendingStage, SessionId, StopReason,
    SubagentInfo, SubagentStatus, TokenCounts, TurnSummary, VoiceStatus,
};

use crate::traits::Activity;
use crate::traits::test_support::MockTuiState;

/// Commit whose widget sources rendered the frozen baseline (pre-migration).
const PINNED_COMMIT: &str = "44bd61c7064e20031e9a9c4514ed4965e6400068";

/// The production theme the baseline equivalence contract is defined
/// against (render.rs resolves the same identity).
fn cyril_dark() -> crate::theme::Theme {
    crate::theme::resolve(
        crate::theme::ThemeId::CyrilDark,
        crate::theme::ColorMode::TrueColor,
    )
}

/// One rendered chrome scene: a stable name plus its raw buffer.
struct Scene {
    name: &'static str,
    buffer: Buffer,
}

fn draw(width: u16, height: u16, render: impl Fn(&mut Frame)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| render(frame)).expect("draw");
    terminal.backend().buffer().clone()
}

fn toolbar_scene(
    theme: &crate::theme::Theme,
    name: &'static str,
    width: u16,
    state: MockTuiState,
) -> Scene {
    Scene {
        name,
        buffer: draw(width, 1, |frame| {
            crate::widgets::toolbar::render(frame, frame.area(), &state, theme);
        }),
    }
}

fn status_scene(
    theme: &crate::theme::Theme,
    name: &'static str,
    width: u16,
    state: MockTuiState,
) -> Scene {
    Scene {
        name,
        buffer: draw(width, 1, |frame| {
            crate::widgets::toolbar::render_status_bar(frame, frame.area(), &state, theme);
        }),
    }
}

fn crew_scene(
    theme: &crate::theme::Theme,
    name: &'static str,
    height: u16,
    state: MockTuiState,
) -> Scene {
    Scene {
        name,
        buffer: draw(80, height, |frame| {
            crate::widgets::crew_panel::render(frame, frame.area(), &state, theme);
        }),
    }
}

fn voice_scene(theme: &crate::theme::Theme, name: &'static str, status: VoiceStatus) -> Scene {
    let mut state = crate::state::UiState::new(100);
    state.set_voice_status(status);
    state.set_voice_level(0.5);
    Scene {
        name,
        buffer: draw(60, 1, |frame| {
            crate::widgets::voice::render(frame, frame.area(), &state, theme);
        }),
    }
}

fn working(id: &str, name: &str, group: Option<&str>) -> SubagentInfo {
    SubagentInfo::new(
        SessionId::new(id),
        name,
        name,
        "q",
        SubagentStatus::Working {
            message: Some("Running".into()),
        },
    )
    .with_group(group.map(String::from))
}

fn crew_state(subagents: Vec<SubagentInfo>, pending_stages: Vec<PendingStage>) -> MockTuiState {
    let mut state = MockTuiState::default();
    state
        .subagent_tracker
        .apply_notification(&Notification::SubagentListUpdated {
            subagents,
            pending_stages,
        });
    state
}

/// All 18 chrome scenes, deterministic order and content.
fn scenes(theme: &crate::theme::Theme) -> Vec<Scene> {
    let mut scenes = vec![
        toolbar_scene(
            theme,
            "toolbar_sending_full",
            120,
            MockTuiState {
                activity: Activity::Sending,
                activity_elapsed: Some(std::time::Duration::from_secs(5)),
                session_label: Some("main".into()),
                current_mode: Some("code".into()),
                current_model: Some("claude-opus-4.8".into()),
                effort: Some(cyril_core::types::EffortLevel::High),
                steering_queued: 2,
                code_intelligence_active: true,
                ..Default::default()
            },
        ),
        toolbar_scene(
            theme,
            "toolbar_streaming_nosession",
            80,
            MockTuiState {
                activity: Activity::Streaming,
                activity_elapsed: Some(std::time::Duration::from_secs(5)),
                ..Default::default()
            },
        ),
        toolbar_scene(
            theme,
            "toolbar_toolrunning_nosession",
            80,
            MockTuiState {
                activity: Activity::ToolRunning,
                activity_elapsed: Some(std::time::Duration::from_secs(5)),
                ..Default::default()
            },
        ),
        toolbar_scene(theme, "toolbar_idle", 80, MockTuiState::default()),
        status_scene(
            theme,
            "status_ok_tokens_credits",
            120,
            MockTuiState {
                context_usage: Some(50.0),
                last_turn: Some(TurnSummary::new(
                    StopReason::EndTurn,
                    Some(TokenCounts::new(1500, 800, Some(300))),
                    None,
                )),
                credit_usage: Some((5.25, 10.0)),
                ..Default::default()
            },
        ),
        status_scene(
            theme,
            "status_warn_breakdown_scroll",
            200,
            MockTuiState {
                context_usage: Some(75.0),
                context_breakdown: Some(ContextBreakdown::new(
                    ContextBucket::new(1, 11.0),
                    ContextBucket::new(2, 22.0),
                    ContextBucket::new(3, 33.0),
                    ContextBucket::new(4, 44.0),
                    ContextBucket::new(5, 55.0),
                )),
                last_turn: Some(TurnSummary::new(StopReason::MaxTokens, None, None)),
                chat_scroll_back: Some(10),
                ..Default::default()
            },
        ),
        status_scene(
            theme,
            "status_crit_refused",
            80,
            MockTuiState {
                context_usage: Some(95.0),
                last_turn: Some(TurnSummary::new(StopReason::Refusal, None, None)),
                ..Default::default()
            },
        ),
        status_scene(
            theme,
            "status_cancelled",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::Cancelled, None, None)),
                ..Default::default()
            },
        ),
        status_scene(
            theme,
            "status_turnlimit",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::MaxTurnRequests, None, None)),
                ..Default::default()
            },
        ),
        status_scene(theme, "status_empty_fallback", 80, MockTuiState::default()),
        status_scene(
            theme,
            "status_boundary_70",
            80,
            MockTuiState {
                context_usage: Some(70.0),
                ..Default::default()
            },
        ),
        status_scene(
            theme,
            "status_boundary_90",
            80,
            MockTuiState {
                context_usage: Some(90.0),
                ..Default::default()
            },
        ),
    ];

    let mut overflow = vec![
        working("s0", "a-looper", Some("crew-a")).with_loop_state(LoopState::new(1, 2)),
        SubagentInfo::new(
            SessionId::new("s1"),
            "b-done",
            "b-done",
            "q",
            SubagentStatus::Terminated,
        ),
    ];
    overflow.extend((0..6).map(|i| working(&format!("w{i}"), &format!("w-{i}"), Some("crew-a"))));
    scenes.push(crew_scene(
        theme,
        "crew_overflow",
        10,
        crew_state(overflow, vec![]),
    ));
    scenes.push(crew_scene(
        theme,
        "crew_small_pending",
        6,
        crew_state(
            vec![working("s0", "writer", Some("crew-a"))],
            vec![PendingStage::new(
                "summary",
                None,
                Some("crew-a".into()),
                None,
                vec!["writer".into()],
            )],
        ),
    ));
    scenes.push(crew_scene(
        theme,
        "crew_no_group",
        5,
        crew_state(
            vec![SubagentInfo::new(
                SessionId::new("s0"),
                "solo",
                "solo",
                "q",
                SubagentStatus::Working { message: None },
            )],
            vec![],
        ),
    ));
    scenes.push(crew_scene(
        theme,
        "crew_multi_group",
        6,
        crew_state(
            vec![
                working("s0", "alpha", Some("crew-a")),
                working("s1", "beta", Some("crew-b")),
            ],
            vec![],
        ),
    ));
    scenes.push(voice_scene(
        theme,
        "voice_listening",
        VoiceStatus::Listening,
    ));
    scenes.push(voice_scene(
        theme,
        "voice_transcribing",
        VoiceStatus::Transcribing,
    ));
    scenes
}

/// Collapse a rendered color to its canonical comparison form (the ghuu
/// scheme: named ANSI -> the VGA RGB value the NAMED table assigns it).
fn normalized_color(color: Color) -> String {
    let rgb = match color {
        Color::Reset => return "DEFAULT".into(),
        Color::Black => (0x00, 0x00, 0x00),
        Color::Red => (0x80, 0x00, 0x00),
        Color::Green => (0x00, 0x80, 0x00),
        Color::Yellow => (0x80, 0x80, 0x00),
        Color::Blue => (0x00, 0x00, 0x80),
        Color::Magenta => (0x80, 0x00, 0x80),
        Color::Cyan => (0x00, 0x80, 0x80),
        Color::Gray => (0xc0, 0xc0, 0xc0),
        Color::DarkGray => (0x80, 0x80, 0x80),
        Color::LightRed => (0xff, 0x00, 0x00),
        Color::LightGreen => (0x00, 0xff, 0x00),
        Color::LightYellow => (0xff, 0xff, 0x00),
        Color::LightBlue => (0x00, 0x00, 0xff),
        Color::LightMagenta => (0xff, 0x00, 0xff),
        Color::LightCyan => (0x00, 0xff, 0xff),
        Color::White => (0xff, 0xff, 0xff),
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Indexed(index) => return format!("INDEX:{index}"),
    };
    format!("RGB:{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

fn symbol_hex(symbol: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(symbol.len() * 2);
    for byte in symbol.bytes() {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

/// Every scene flattened to normalized TSV rows (the baseline's row shape).
fn scene_rows() -> Vec<String> {
    let mut rows = Vec::with_capacity(4_096);
    for scene in scenes(&cyril_dark()) {
        let area = scene.buffer.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &scene.buffer[(x, y)];
                rows.push(format!(
                    "{}\t{x}\t{y}\t{}\t{}\t{}\t{}",
                    scene.name,
                    symbol_hex(cell.symbol()),
                    normalized_color(cell.fg),
                    normalized_color(cell.bg),
                    cell.modifier.bits(),
                ));
            }
        }
    }
    rows
}

/// Generator for tests/fixtures/chrome-theme-baseline.tsv. Data goes to
/// stdout inside BEGIN/END fences (extract with `--nocapture`, same
/// convention as the ghuu generator and the dij8 probe).
#[test]
fn emit_chrome_baseline() {
    println!("BEGIN_CHROME_BASELINE");
    println!("commit\t{PINNED_COMMIT}");
    println!("scene\tx\ty\tsymbol_hex\tforeground\tbackground\tmodifier_bits");
    for row in scene_rows() {
        println!("{row}");
    }
    println!("END_CHROME_BASELINE");
}

/// Slice-2 stress: scene production is deterministic. Two in-process runs
/// build distinct `HashMap` instances inside the trackers, so iteration-
/// order nondeterminism in a scene builder would flap this assert.
#[test]
fn baseline_generation_is_deterministic() {
    assert_eq!(scene_rows(), scene_rows());
}

const FIXTURE: &str = include_str!("../tests/fixtures/chrome-theme-baseline.tsv");

/// Fixture body rows (headers validated and stripped).
fn fixture_rows() -> Vec<&'static str> {
    let mut lines = FIXTURE.lines();
    let commit_header = format!("commit\t{PINNED_COMMIT}");
    assert_eq!(
        lines.next(),
        Some(commit_header.as_str()),
        "baseline commit header must remain pinned"
    );
    assert_eq!(
        lines.next(),
        Some("scene\tx\ty\tsymbol_hex\tforeground\tbackground\tmodifier_bits")
    );
    lines.collect()
}

fn scene_buffer(name: &str) -> Buffer {
    scenes(&cyril_dark())
        .into_iter()
        .find(|scene| scene.name == name)
        .unwrap_or_else(|| panic!("unknown chrome scene {name}"))
        .buffer
}

fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
    }
    text
}

/// cyril-dij8 C2: chrome renders are cell-for-cell equivalent to the frozen
/// pre-migration baseline under canonical normalization. Trivially green
/// until migration starts; from then on it guards every migration slice.
/// A failing row names its scene and cell (rows lead with `scene\tx\ty`).
#[test]
fn chrome_baseline_equivalence() {
    let expected = fixture_rows();
    let actual = scene_rows();
    assert_eq!(
        expected.len(),
        actual.len(),
        "chrome cell count diverged from the frozen baseline"
    );
    for (expected_row, actual_row) in expected.iter().zip(&actual) {
        assert_eq!(
            *expected_row, actual_row,
            "chrome cell diverged from the frozen baseline"
        );
    }
}

/// cyril-dij8 C8: the scene set jointly reaches the complete normalized
/// tuple inventory — the probe's 23 raw styled tuples minus the 3 named
/// collapses (Yellow/Green/DarkGray on the chrome bg each occur in both
/// toolbar and status bar), transcribed from .cyril-dij8/probe-styles.txt.
/// Without this, equivalence is vacuous for unreached tuples.
#[test]
fn baseline_covers_probe_inventory() {
    const EXPECTED: [&str; 20] = [
        // cyril-leiq: the five brightened roles replace their dim VGA tuples
        // (accent_quinary 008080->56c7d0, subdued_negative 800000->d98a8a,
        // accent_quaternary 800080->cd9ee6, emphasis 808000->d7ba7d; link
        // 000080->6cb6ff isn't in chrome). Derived by applying the same
        // substitution as the fixture and verified to equal fixture_rows().
        "RGB:008000|DEFAULT|0",
        "RGB:008000|RGB:1e1e2e|0",
        "RGB:56c7d0|DEFAULT|0",
        "RGB:56c7d0|RGB:1e1e2e|0",
        "RGB:808080|DEFAULT|0",
        "RGB:808080|RGB:1e1e2e|0",
        "RGB:808080|RGB:1e1e2e|1",
        "RGB:8ab4f8|DEFAULT|0",
        "RGB:8c8c8c|DEFAULT|0",
        "RGB:b48ead|DEFAULT|0",
        "RGB:c0c0c0|DEFAULT|0",
        "RGB:cd9ee6|DEFAULT|0",
        "RGB:cd9ee6|RGB:1e1e2e|0",
        "RGB:d7ba7d|DEFAULT|4",
        "RGB:d7ba7d|RGB:1e1e2e|0",
        "RGB:d7ba7d|RGB:1e1e2e|1",
        "RGB:d98a8a|RGB:1e1e2e|0",
        "RGB:d98a8a|RGB:1e1e2e|1",
        "RGB:ffffff|DEFAULT|1",
        "RGB:ffffff|RGB:1e1e2e|1",
    ];
    let found: std::collections::BTreeSet<String> = fixture_rows()
        .iter()
        .filter_map(|row| {
            let fields: Vec<&str> = row.split('\t').collect();
            (fields[4] != "DEFAULT").then(|| format!("{}|{}|{}", fields[4], fields[5], fields[6]))
        })
        .collect();
    let expected: std::collections::BTreeSet<String> =
        EXPECTED.iter().map(ToString::to_string).collect();
    assert_eq!(found, expected, "baseline styled-tuple inventory drifted");
}

/// cyril-dij8 C10: idle toolbar shows no spinner and styles nothing but
/// the "No session" label (subdued family).
#[test]
fn edge_toolbar_idle_has_no_spinner() {
    let buffer = scene_buffer("toolbar_idle");
    let text = buffer_text(&buffer);
    for spinner in crate::spinner::SPINNER_CHARS {
        assert!(!text.contains(*spinner), "idle toolbar rendered a spinner");
    }
    let styled: std::collections::BTreeSet<String> = (0..buffer.area.width)
        .map(|x| normalized_color(buffer[(x, 0)].fg))
        .filter(|fg| fg != "DEFAULT")
        .collect();
    assert_eq!(
        styled,
        std::collections::BTreeSet::from(["RGB:808080".to_string()]),
        "idle toolbar styled something beyond the No-session label"
    );
}

/// cyril-dij8 C10: the context gauge thresholds are STRICT — exactly 70
/// stays in the OK band, exactly 90 stays in the warn band. Expected
/// values pre-written in the plan (slice 3): 70 -> RGB:008000,
/// 90 -> RGB:808000. A migration flipping `>` to `>=` fails here.
#[test]
fn edge_status_boundaries_pin_strict_thresholds() {
    for (scene, expected) in [
        ("status_boundary_70", "RGB:008000"),
        ("status_boundary_90", "RGB:d7ba7d"), // cyril-leiq: emphasis brightened off 0x808000
    ] {
        let buffer = scene_buffer(scene);
        let styled: std::collections::BTreeSet<String> = (0..buffer.area.width)
            .map(|x| normalized_color(buffer[(x, 0)].fg))
            .filter(|fg| fg != "DEFAULT")
            .collect();
        assert_eq!(
            styled,
            std::collections::BTreeSet::from([expected.to_string()]),
            "{scene}: gauge band drifted"
        );
    }
}

/// cyril-dij8 C10: crew header variants and message-less Working status.
#[test]
fn edge_crew_headers_and_plain_working() {
    let no_group = buffer_text(&scene_buffer("crew_no_group"));
    assert!(no_group.contains("subagents"), "groupless header missing");
    assert!(no_group.contains("Working"), "message-less status missing");
    let multi = buffer_text(&scene_buffer("crew_multi_group"));
    assert!(multi.contains("2 crews"), "multi-group header missing");
}

/// Column of `needle`'s first char on row `y` (ASCII needles only: one
/// char per cell, so symbol windows map 1:1 to columns).
fn find_text_x(buffer: &Buffer, y: u16, needle: &str) -> u16 {
    let symbols: Vec<&str> = (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect();
    let needle_syms: Vec<String> = needle.chars().map(String::from).collect();
    for x in 0..=(symbols.len() - needle_syms.len()) {
        if symbols[x..x + needle_syms.len()]
            .iter()
            .zip(&needle_syms)
            .all(|(a, b)| a == b)
        {
            return x as u16;
        }
    }
    panic!("needle {needle:?} not found on row {y}");
}

fn marker() -> crate::theme::Theme {
    crate::traits::test_support::marker_theme()
}

/// cyril-dij8 C3: under the pairwise-distinct marker theme every toolbar
/// element renders its MAPPED role's marker color. The element→role table
/// is hand-pinned from the design mapping, not read from render output.
#[test]
fn marker_wiring_toolbar() {
    let state = MockTuiState {
        activity: Activity::Sending,
        activity_elapsed: Some(std::time::Duration::from_secs(5)),
        session_label: Some("main".into()),
        current_mode: Some("code".into()),
        current_model: Some("claude-opus-4.8".into()),
        ..Default::default()
    };
    let buffer = draw(120, 1, |frame| {
        crate::widgets::toolbar::render(frame, frame.area(), &state, &marker());
    });
    // chrome bg blankets the paragraph area (trailing blank cell included).
    assert_eq!(buffer[(119, 0)].bg, Color::Indexed(2), "bg -> chrome");
    assert_eq!(buffer[(0, 0)].fg, Color::Indexed(20), "spinner -> emphasis");
    let session = find_text_x(&buffer, 0, "main");
    assert_eq!(
        buffer[(session, 0)].fg,
        Color::Indexed(5),
        "session -> text"
    );
    let mode = find_text_x(&buffer, 0, "code");
    assert_eq!(
        buffer[(mode, 0)].fg,
        Color::Indexed(23),
        "mode -> accent_quinary"
    );
    let model = find_text_x(&buffer, 0, "claude");
    assert_eq!(
        buffer[(model, 0)].fg,
        Color::Indexed(22),
        "model -> accent_quaternary"
    );
}

/// cyril-dij8 C3: status-bar element→role marker wiring.
#[test]
fn marker_wiring_status() {
    let warn = MockTuiState {
        context_usage: Some(75.0),
        last_turn: Some(TurnSummary::new(StopReason::MaxTokens, None, None)),
        credit_usage: Some((5.25, 10.0)),
        chat_scroll_back: Some(10),
        ..Default::default()
    };
    let buffer = draw(120, 1, |frame| {
        crate::widgets::toolbar::render_status_bar(frame, frame.area(), &warn, &marker());
    });
    assert_eq!(buffer[(119, 0)].bg, Color::Indexed(2), "bg -> chrome");
    let gauge = find_text_x(&buffer, 0, "Context:");
    assert_eq!(
        buffer[(gauge, 0)].fg,
        Color::Indexed(20),
        "warn -> emphasis"
    );
    let label = find_text_x(&buffer, 0, "Token limit");
    assert_eq!(buffer[(label, 0)].fg, Color::Indexed(20));
    let credits = find_text_x(&buffer, 0, "Credits:");
    assert_eq!(
        buffer[(credits, 0)].fg,
        Color::Indexed(24),
        "credits -> subdued"
    );
    let scroll = find_text_x(&buffer, 0, "SCROLL");
    assert_eq!(buffer[(scroll, 0)].fg, Color::Indexed(20));

    let refused = MockTuiState {
        context_usage: Some(95.0),
        last_turn: Some(TurnSummary::new(StopReason::Refusal, None, None)),
        ..Default::default()
    };
    let buffer = draw(80, 1, |frame| {
        crate::widgets::toolbar::render_status_bar(frame, frame.area(), &refused, &marker());
    });
    let gauge = find_text_x(&buffer, 0, "Context:");
    assert_eq!(
        buffer[(gauge, 0)].fg,
        Color::Indexed(26),
        "critical -> subdued_negative"
    );
    let label = find_text_x(&buffer, 0, "Refused");
    assert_eq!(buffer[(label, 0)].fg, Color::Indexed(26));
}

/// cyril-dij8 C3: crew element→role marker wiring, including the roles that
/// share a Cyril Dark value with a twin (`subdued` vs `muted`,
/// `text_secondary`) — the cross-wiring class equivalence cannot see.
#[test]
fn marker_wiring_crew() {
    let state = crew_state(
        vec![working("s0", "writer", Some("crew-a")).with_loop_state(LoopState::new(0, 2))],
        vec![PendingStage::new(
            "summary",
            None,
            Some("crew-a".into()),
            None,
            vec![],
        )],
    );
    let buffer = draw(80, 6, |frame| {
        crate::widgets::crew_panel::render(frame, frame.area(), &state, &marker());
    });
    let title = find_text_x(&buffer, 0, "crew: crew-a");
    assert_eq!(
        buffer[(title, 0)].fg,
        Color::Indexed(23),
        "title -> accent_quinary"
    );
    let icon = find_text_x(&buffer, 1, "●");
    assert_eq!(
        buffer[(icon, 1)].fg,
        Color::Indexed(25),
        "working icon -> subdued_positive"
    );
    let name = find_text_x(&buffer, 1, "writer");
    assert_eq!(buffer[(name, 1)].fg, Color::Indexed(5), "name -> text");
    let status = find_text_x(&buffer, 1, "Running");
    assert_eq!(
        buffer[(status, 1)].fg,
        Color::Indexed(24),
        "status -> subdued, NOT muted (6): twin pair"
    );
    let badge = find_text_x(&buffer, 1, "↻");
    assert_eq!(
        buffer[(badge, 1)].fg,
        Color::Indexed(22),
        "loop badge -> accent_quaternary"
    );
    let stage = find_text_x(&buffer, 2, "summary");
    assert_eq!(
        buffer[(stage, 2)].fg,
        Color::Indexed(30),
        "pending name -> text_secondary"
    );
    // Border stays default-styled (negative-space #3).
    assert_eq!(buffer[(0, 0)].fg, Color::Reset, "border unstyled");
}

/// cyril-dij8 C3: voice twin wiring — the three chrome/speaker twin pairs
/// coincide under Cyril Dark, so ONLY this marker fence can catch a
/// speaker-role cross-wire (soft_accent 27 vs user 10, muted 6 vs
/// border 7, accent_alt 9 vs system 12).
#[test]
fn marker_wiring_voice() {
    let mut state = crate::state::UiState::new(100);
    state.set_voice_status(VoiceStatus::Listening);
    state.set_voice_level(0.5);
    let buffer = draw(60, 1, |frame| {
        crate::widgets::voice::render(frame, frame.area(), &state, &marker());
    });
    let mic = buffer[(0, 0)].fg;
    assert_eq!(mic, Color::Indexed(27), "listening -> soft_accent");
    assert_ne!(mic, Color::Indexed(10), "listening must NOT wire to user");
    let hint = find_text_x(&buffer, 0, "/voice");
    assert_eq!(buffer[(hint, 0)].fg, Color::Indexed(6), "hint -> muted");
    assert_ne!(
        buffer[(hint, 0)].fg,
        Color::Indexed(7),
        "hint must NOT wire to border"
    );

    state.set_voice_status(VoiceStatus::Transcribing);
    let buffer = draw(60, 1, |frame| {
        crate::widgets::voice::render(frame, frame.area(), &state, &marker());
    });
    let hourglass = buffer[(0, 0)].fg;
    assert_eq!(hourglass, Color::Indexed(9), "transcribing -> accent_alt");
    assert_ne!(
        hourglass,
        Color::Indexed(12),
        "transcribing must NOT wire to system"
    );
}

/// cyril-dij8 C10: idle voice occupies no height and paints no cell.
#[test]
fn edge_voice_idle_renders_nothing() {
    let state = crate::state::UiState::new(100);
    assert_eq!(crate::widgets::voice::height_for(&state), 0);
    let buffer = draw(60, 1, |frame| {
        crate::widgets::voice::render(frame, frame.area(), &state, &cyril_dark());
    });
    for x in 0..60 {
        let cell = &buffer[(x, 0)];
        assert_eq!(cell.symbol(), " ", "idle voice painted a symbol");
        assert_eq!(normalized_color(cell.fg), "DEFAULT");
        assert_eq!(normalized_color(cell.bg), "DEFAULT");
    }
}

/// cyril-dij8 C6: under the NoColor projection every chrome cell carries
/// zero color, and the glyphs are identical to the truecolor scenes — the
/// no-color mode drops style, never content.
#[test]
fn no_color_scenarios_reset() {
    let no_color = crate::theme::resolve(
        crate::theme::ThemeId::CyrilDark,
        crate::theme::ColorMode::None,
    );
    for (colored, plain) in scenes(&cyril_dark()).iter().zip(&scenes(&no_color)) {
        assert_eq!(colored.name, plain.name);
        let area = plain.buffer.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &plain.buffer[(x, y)];
                assert_eq!(
                    cell.fg,
                    Color::Reset,
                    "{}[{x},{y}] carries a fg under no-color",
                    plain.name
                );
                assert_eq!(
                    cell.bg,
                    Color::Reset,
                    "{}[{x},{y}] carries a bg under no-color",
                    plain.name
                );
                assert_eq!(
                    cell.symbol(),
                    colored.buffer[(x, y)].symbol(),
                    "{}[{x},{y}] glyph drifted between color modes",
                    plain.name
                );
            }
        }
    }
}

/// cyril-dij8 C7 (issue AC2): status meaning survives with color stripped —
/// every state the chrome signals by color is also carried by a label or
/// symbol. A migration that dropped the "Cancelled" label and signaled by
/// gray alone would fail exactly here.
#[test]
fn no_color_status_distinguishable() {
    let no_color = crate::theme::resolve(
        crate::theme::ThemeId::CyrilDark,
        crate::theme::ColorMode::None,
    );
    let required: &[(&str, &[&str])] = &[
        (
            "toolbar_sending_full",
            &["⇄ 2 steers", "◇ high", "✦ code intel"],
        ),
        ("toolbar_streaming_nosession", &["No session"]),
        (
            "status_warn_breakdown_scroll",
            &["Token limit", "SCROLL", "Context: 75%"],
        ),
        ("status_crit_refused", &["Refused", "Context: 95%"]),
        ("status_cancelled", &["Cancelled"]),
        ("status_turnlimit", &["Turn limit"]),
        ("status_empty_fallback", &["cyril"]),
        ("crew_overflow", &["●", "◆", "+3 more", "Terminated"]),
        ("crew_small_pending", &["○", "Waiting"]),
        ("voice_listening", &["🎙", "listening", "/voice to stop"]),
        ("voice_transcribing", &["⏳", "transcribing"]),
    ];
    let rendered = scenes(&no_color);
    for (name, needles) in required {
        let scene = rendered
            .iter()
            .find(|scene| scene.name == *name)
            .unwrap_or_else(|| panic!("missing scene {name}"));
        let text = buffer_text(&scene.buffer);
        for needle in *needles {
            assert!(
                text.contains(needle),
                "{name}: status meaning lost without color — missing {needle:?}"
            );
        }
    }
}
