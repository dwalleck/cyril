//! cyril-dij8 prove-it probe: dump the distinct (fg, bg, modifier) style
//! tuples the chrome surfaces emit at runtime today. TEMPORARY — archived to
//! .cyril-dij8/ and removed from the tree after the probe run.

use std::collections::BTreeMap;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

use crate::traits::test_support::MockTuiState;
use crate::traits::{Activity, TuiState};
use cyril_core::types::{
    ContextBreakdown, ContextBucket, LoopState, Notification, PendingStage, SessionId, StopReason,
    SubagentInfo, SubagentStatus, TokenCounts, TurnSummary, VoiceStatus,
};

fn dump(label: &str, w: u16, h: u16, draw: impl Fn(&mut Frame)) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("terminal");
    terminal.draw(|frame| draw(frame)).expect("draw");
    let buf = terminal.backend().buffer();
    let mut tuples: BTreeMap<String, String> = BTreeMap::new();
    for y in 0..h {
        for x in 0..w {
            let cell = &buf[(x, y)];
            let key = format!("{:?}|{:?}|{:?}", cell.fg, cell.bg, cell.modifier);
            let sample = tuples.entry(key).or_default();
            let sym = cell.symbol();
            if sym != " " && sample.chars().count() < 24 {
                sample.push_str(sym);
            }
        }
    }
    println!("SCENARIO {label}");
    for (tuple, sample) in tuples {
        println!("  {tuple}|{sample}");
    }
}

fn working(id: &str, name: &str) -> SubagentInfo {
    let status = SubagentStatus::Working {
        message: Some("Running".into()),
    };
    SubagentInfo::new(SessionId::new(id), name, name, "q", status).with_group(Some("crew-a".into()))
}

#[test]
fn emit_chrome_style_probe() {
    let base = MockTuiState {
        activity: Activity::Sending,
        activity_elapsed: Some(std::time::Duration::from_secs(5)),
        session_label: Some("main".into()),
        current_mode: Some("code".into()),
        current_model: Some("claude-opus-4.8".into()),
        effort: Some(cyril_core::types::EffortLevel::High),
        steering_queued: 2,
        code_intelligence_active: true,
        ..Default::default()
    };
    dump("toolbar_sending_full", 120, 1, |f| {
        crate::widgets::toolbar::render(f, f.area(), &base);
    });
    for (label, activity) in [
        ("toolbar_streaming_nosession", Activity::Streaming),
        ("toolbar_toolrunning_nosession", Activity::ToolRunning),
    ] {
        let state = MockTuiState {
            activity,
            activity_elapsed: Some(std::time::Duration::from_secs(5)),
            ..Default::default()
        };
        dump(label, 80, 1, |f| {
            crate::widgets::toolbar::render(f, f.area(), &state);
        });
    }

    let status_cases: [(&str, u16, MockTuiState); 6] = [
        (
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
        (
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
        (
            "status_crit_refused",
            80,
            MockTuiState {
                context_usage: Some(95.0),
                last_turn: Some(TurnSummary::new(StopReason::Refusal, None, None)),
                ..Default::default()
            },
        ),
        (
            "status_cancelled",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::Cancelled, None, None)),
                ..Default::default()
            },
        ),
        (
            "status_turnlimit",
            80,
            MockTuiState {
                last_turn: Some(TurnSummary::new(StopReason::MaxTurnRequests, None, None)),
                ..Default::default()
            },
        ),
        ("status_empty_fallback", 80, MockTuiState::default()),
    ];
    for (label, w, state) in &status_cases {
        dump(label, *w, 1, |f| {
            crate::widgets::toolbar::render_status_bar(f, f.area(), state);
        });
    }

    // Crew: 8 subagents + 1 pending = overflow (visible 5 sorted rows + "+4 more").
    let mut crew_over = MockTuiState::default();
    let mut agents = vec![
        working("s0", "a-looper").with_loop_state(LoopState::new(1, 2)),
        SubagentInfo::new(
            SessionId::new("s1"),
            "b-done",
            "b-done",
            "q",
            SubagentStatus::Terminated,
        ),
    ];
    agents.extend((0..6).map(|i| working(&format!("w{i}"), &format!("w-{i}"))));
    crew_over
        .subagent_tracker
        .apply_notification(&Notification::SubagentListUpdated {
            subagents: agents,
            pending_stages: vec![],
        });
    dump("crew_overflow", 80, 10, |f| {
        crate::widgets::crew_panel::render(f, f.area(), &crew_over);
    });

    let mut crew_small = MockTuiState::default();
    crew_small
        .subagent_tracker
        .apply_notification(&Notification::SubagentListUpdated {
            subagents: vec![working("s0", "writer")],
            pending_stages: vec![PendingStage::new(
                "summary",
                None,
                Some("crew-a".into()),
                None,
                vec!["writer".into()],
            )],
        });
    dump("crew_small_pending", 80, 6, |f| {
        crate::widgets::crew_panel::render(f, f.area(), &crew_small);
    });

    // Voice: real UiState (voice fields live there, not on the mock).
    let mut voice = crate::state::UiState::new(100);
    voice.set_voice_status(VoiceStatus::Listening);
    voice.set_voice_level(0.5);
    dump("voice_listening", 60, 1, |f| {
        crate::widgets::voice::render(f, f.area(), &voice);
    });
    voice.set_voice_status(VoiceStatus::Transcribing);
    dump("voice_transcribing", 60, 1, |f| {
        crate::widgets::voice::render(f, f.area(), &voice);
    });
}
