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

fn toolbar_scene(name: &'static str, width: u16, state: MockTuiState) -> Scene {
    Scene {
        name,
        buffer: draw(width, 1, |frame| {
            crate::widgets::toolbar::render(frame, frame.area(), &state, &cyril_dark());
        }),
    }
}

fn status_scene(name: &'static str, width: u16, state: MockTuiState) -> Scene {
    Scene {
        name,
        buffer: draw(width, 1, |frame| {
            crate::widgets::toolbar::render_status_bar(frame, frame.area(), &state, &cyril_dark());
        }),
    }
}

fn crew_scene(name: &'static str, height: u16, state: MockTuiState) -> Scene {
    Scene {
        name,
        buffer: draw(80, height, |frame| {
            crate::widgets::crew_panel::render(frame, frame.area(), &state);
        }),
    }
}

fn voice_scene(name: &'static str, status: VoiceStatus) -> Scene {
    let mut state = crate::state::UiState::new(100);
    state.set_voice_status(status);
    state.set_voice_level(0.5);
    Scene {
        name,
        buffer: draw(60, 1, |frame| {
            crate::widgets::voice::render(frame, frame.area(), &state);
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
fn scenes() -> Vec<Scene> {
    let mut scenes = vec![
        toolbar_scene(
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
            "toolbar_streaming_nosession",
            80,
            MockTuiState {
                activity: Activity::Streaming,
                activity_elapsed: Some(std::time::Duration::from_secs(5)),
                ..Default::default()
            },
        ),
        toolbar_scene(
            "toolbar_toolrunning_nosession",
            80,
            MockTuiState {
                activity: Activity::ToolRunning,
                activity_elapsed: Some(std::time::Duration::from_secs(5)),
                ..Default::default()
            },
        ),
        toolbar_scene("toolbar_idle", 80, MockTuiState::default()),
        status_scene(
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
            "status_crit_refused",
            80,
            MockTuiState {
                context_usage: Some(95.0),
                last_turn: Some(TurnSummary::new(StopReason::Refusal, None, None)),
                ..Default::default()
            },
        ),
        status_scene(
            "status_cancelled",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::Cancelled, None, None)),
                ..Default::default()
            },
        ),
        status_scene(
            "status_turnlimit",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::MaxTurnRequests, None, None)),
                ..Default::default()
            },
        ),
        status_scene("status_empty_fallback", 80, MockTuiState::default()),
        status_scene(
            "status_boundary_70",
            80,
            MockTuiState {
                context_usage: Some(70.0),
                ..Default::default()
            },
        ),
        status_scene(
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
        "crew_overflow",
        10,
        crew_state(overflow, vec![]),
    ));
    scenes.push(crew_scene(
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
    scenes.push(voice_scene("voice_listening", VoiceStatus::Listening));
    scenes.push(voice_scene("voice_transcribing", VoiceStatus::Transcribing));
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
    for scene in scenes() {
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
    scenes()
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
        "RGB:008000|DEFAULT|0",
        "RGB:008000|RGB:1e1e2e|0",
        "RGB:008080|DEFAULT|0",
        "RGB:008080|RGB:1e1e2e|0",
        "RGB:800000|RGB:1e1e2e|0",
        "RGB:800000|RGB:1e1e2e|1",
        "RGB:800080|DEFAULT|0",
        "RGB:800080|RGB:1e1e2e|0",
        "RGB:808000|DEFAULT|4",
        "RGB:808000|RGB:1e1e2e|0",
        "RGB:808000|RGB:1e1e2e|1",
        "RGB:808080|DEFAULT|0",
        "RGB:808080|RGB:1e1e2e|0",
        "RGB:808080|RGB:1e1e2e|1",
        "RGB:8ab4f8|DEFAULT|0",
        "RGB:8c8c8c|DEFAULT|0",
        "RGB:b48ead|DEFAULT|0",
        "RGB:c0c0c0|DEFAULT|0",
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
    for spinner in crate::palette::SPINNER_CHARS {
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
        ("status_boundary_90", "RGB:808000"),
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

/// cyril-dij8 C10: idle voice occupies no height and paints no cell.
#[test]
fn edge_voice_idle_renders_nothing() {
    let state = crate::state::UiState::new(100);
    assert_eq!(crate::widgets::voice::height_for(&state), 0);
    let buffer = draw(60, 1, |frame| {
        crate::widgets::voice::render(frame, frame.area(), &state);
    });
    for x in 0..60 {
        let cell = &buffer[(x, 0)];
        assert_eq!(cell.symbol(), " ", "idle voice painted a symbol");
        assert_eq!(normalized_color(cell.fg), "DEFAULT");
        assert_eq!(normalized_color(cell.bg), "DEFAULT");
    }
}
