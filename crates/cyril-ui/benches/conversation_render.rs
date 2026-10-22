//! Full-root rendering measurements; no wall-clock acceptance threshold.
//!
//! Run: `cargo bench -p cyril-ui --bench conversation_render -- completed-write`.
//! Or: `cargo bench -p cyril-ui --bench conversation_render -- long-history`.
//! Record the checked revision, CPU, `rustc -Vv`, and exact Cargo invocation with
//! the output. Optional `CYRIL_BENCH_*` labels attach those facts to the receipt.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::{Duration, Instant};

use anyhow::{Result, bail, ensure};
use cyril_core::types::{
    AgentMessage, AgentThought, Notification, PermissionOption, PermissionOptionId,
    PermissionOptionKind, PermissionRequest, SessionId, StopReason, ToolCall, ToolCallContent,
    ToolCallId, ToolCallStatus, ToolKind,
};
use cyril_ui::render;
use cyril_ui::state::UiState;
use cyril_ui::theme::{ColorMode, ThemeId};
use cyril_ui::traits::{ChatMessageKind, TuiState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;

const WIDTH: u16 = 160;
const HEIGHT: u16 = 48;
const WARMUP_DRAWS: usize = 100;
const SAMPLES: usize = 30;
const DRAWS_PER_SAMPLE: usize = 100;
const RETAINED_MESSAGES: usize = 2;
const USAGE: &str = "Usage: conversation_render <completed-write|long-history> [--bench]\n\
    completed-write: sparse 2,000-line and dense 20,000-line Writes, plus pending approval.\n\
    long-history: retained 100/1k/10k/50k histories, giant sources, mutations and invalidations.\n\
    --bench is accepted for Cargo's benchmark invocation; unknown modes/arguments are errors.";
const CODE_CONTROL: &str =
    "Write rendering fixture:\n```rust\nfn benchmark_control() -> bool { true }\n```\n";

#[derive(Clone, Copy)]
enum Mode {
    CompletedWrite,
    LongHistory,
    Help,
}

fn parse_mode(arguments: impl IntoIterator<Item = OsString>) -> Result<Mode> {
    let mut mode = None;
    let mut cargo_bench = false;
    for argument in arguments {
        match argument.to_str() {
            Some("completed-write") if mode.is_none() => mode = Some(Mode::CompletedWrite),
            Some("long-history") if mode.is_none() => mode = Some(Mode::LongHistory),
            Some("--help" | "-h") if mode.is_none() => mode = Some(Mode::Help),
            Some("--bench") if !cargo_bench => cargo_bench = true,
            _ => bail!("unknown or duplicate mode/argument {argument:?}\n{USAGE}"),
        }
    }
    mode.ok_or_else(|| anyhow::anyhow!("a benchmark mode is required\n{USAGE}"))
}

fn main() -> Result<()> {
    match parse_mode(std::env::args_os().skip(1))? {
        Mode::Help => println!("{USAGE}"),
        Mode::CompletedWrite => {
            report_environment("completed-write");
            for workload in [WriteWorkload::Sparse, WriteWorkload::Dense] {
                run_completed_write(workload)?;
            }
            run_pending_preview()?;
        }
        Mode::LongHistory => {
            report_environment("long-history");
            run_long_histories()?;
        }
    }
    Ok(())
}

fn report_environment(mode: &str) {
    println!(
        "benchmark=conversation_render mode={mode} package_version={} os={} arch={} family={} debug_assertions={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        cfg!(debug_assertions),
    );
    println!(
        "backend=ratatui::backend::TestBackend viewport={WIDTH}x{HEIGHT} theme=CyrilDark color_mode=TrueColor process=single sequential_cases=true"
    );
    println!(
        "recommended_profile=bench declared_ui_features=none declared_core_dev_features=kas,test-support toolchain_msrv={}",
        env!("CARGO_PKG_RUST_VERSION"),
    );
    println!(
        "timing=Instant transition_draw=separate mutation_and_fixture_setup=excluded process_caches=shared_between_cases warmup_draws={WARMUP_DRAWS} samples={SAMPLES} draws_per_sample={DRAWS_PER_SAMPLE}"
    );
    println!(
        "statistics=median_and_nearest_rank_p95_of_sample_mean_ns_per_frame timing_threshold=none"
    );
    println!(
        "receipt_requires=checked_revision,CPU,rustc_-Vv,exact_profile,target,features,Cargo_invocation"
    );
    for key in [
        "CYRIL_BENCH_REVISION",
        "CYRIL_BENCH_CPU",
        "CYRIL_BENCH_RUSTC",
        "CYRIL_BENCH_PROFILE",
        "CYRIL_BENCH_TARGET",
        "CYRIL_BENCH_FEATURES",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "CARGO_PROFILE_BENCH_OPT_LEVEL",
        "CARGO_PROFILE_BENCH_LTO",
        "CARGO_PROFILE_BENCH_CODEGEN_UNITS",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "TERM",
        "COLORTERM",
        "NO_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
    ] {
        match std::env::var_os(key) {
            Some(value) => println!("environment {key}={value:?}"),
            None => println!("environment {key}=<unset>"),
        }
    }
}

#[derive(Clone, Copy)]
enum WriteWorkload {
    Sparse,
    Dense,
}

impl WriteWorkload {
    fn name(self) -> &'static str {
        match self {
            Self::Sparse => "completed-write-sparse-2k",
            Self::Dense => "completed-write-dense-20k",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Sparse => "benchmark_sparse.rs",
            Self::Dense => "benchmark_dense.rs",
        }
    }

    fn line_count(self) -> usize {
        match self {
            Self::Sparse => 2_000,
            Self::Dense => 20_000,
        }
    }

    fn changed(self, line: usize) -> bool {
        match self {
            Self::Sparse => {
                (16..18).contains(&line)
                    || (996..998).contains(&line)
                    || (1_990..1_992).contains(&line)
            }
            // Three changed lines followed by an unchanged anchor: dense
            // replacement groups, without an artificial all-unrelated file.
            Self::Dense => !line.is_multiple_of(4),
        }
    }

    fn visible_controls(self) -> &'static [&'static str] {
        match self {
            Self::Sparse => &[
                "✓ Edit(benchmark_sparse.rs)  +6 -6",
                "│- const OLD_00016",
                "│+ const NEW_00016",
                "benchmark_control",
            ],
            Self::Dense => &[
                "✓ Edit(benchmark_dense.rs)  +15000 -15000",
                "│- const OLD_00001",
                "│+ const NEW_00001",
                "      ...",
                "benchmark_control",
            ],
        }
    }
}

fn write_fixture(workload: WriteWorkload) -> Result<(String, String)> {
    let capacity = workload.line_count() * 40;
    let mut old = String::with_capacity(capacity);
    let mut new = String::with_capacity(capacity);
    for line in 0..workload.line_count() {
        if workload.changed(line) {
            writeln!(old, "const OLD_{line:05}: u64 = {line};")?;
            writeln!(new, "const NEW_{line:05}: u64 = {line};")?;
        } else {
            writeln!(old, "const KEEP_{line:05}: u64 = {line};")?;
            writeln!(new, "const KEEP_{line:05}: u64 = {line};")?;
        }
    }
    Ok((old, new))
}

fn new_state() -> UiState {
    let mut state = UiState::new(32);
    state.set_terminal_size(WIDTH, HEIGHT);
    state.set_appearance(ThemeId::CyrilDark, ColorMode::TrueColor);
    state
}

fn run_completed_write(workload: WriteWorkload) -> Result<()> {
    let (old_text, new_text) = write_fixture(workload)?;
    ensure!(old_text.lines().count() == workload.line_count());
    ensure!(new_text.lines().count() == workload.line_count());
    let old_bytes = old_text.len();
    let new_bytes = new_text.len();
    let mut state = new_state();
    ensure!(
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: CODE_CONTROL.into(),
            is_streaming: true,
        }))
    );
    let id = ToolCallId::new(workload.name());
    ensure!(
        state.apply_notification(&Notification::ToolCallStarted(ToolCall::new(
            id.clone(),
            format!("Editing {}", workload.path()),
            ToolKind::Write,
            ToolCallStatus::InProgress,
            None,
        )))
    );
    ensure!(
        state.apply_notification(&Notification::ToolCallUpdated(
            ToolCall::new(
                id.clone(),
                String::new(),
                ToolKind::Write,
                ToolCallStatus::InProgress,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: workload.path().into(),
                old_text: Some(old_text),
                new_text,
            }]),
        ))
    );
    // Completion is deliberately content-free: exercise the real merge seam
    // retaining the earlier diff, then render the historical snapshot alone.
    ensure!(
        state.apply_notification(&Notification::ToolCallUpdated(ToolCall::new(
            id.clone(),
            String::new(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )))
    );
    ensure!(state.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    }));
    ensure!(
        state.history().messages().len() == RETAINED_MESSAGES,
        "fixture was evicted"
    );
    ensure!(
        state.active_tool_calls().is_empty(),
        "fixture is not historical"
    );
    let retained_writes = state
        .history()
        .messages()
        .iter()
        .filter(|message| {
            matches!(message.kind(), ChatMessageKind::ToolCall(call)
            if call.id() == &id
                && call.kind() == ToolKind::Write
                && call.status() == ToolCallStatus::Completed
                && call.primary_path() == Some(workload.path())
                && matches!(call.content(), [ToolCallContent::Diff { .. }]))
        })
        .count();
    ensure!(
        retained_writes == 1,
        "expected one retained completed Write"
    );
    println!(
        "workload={} old_lines={} new_lines={} old_bytes={old_bytes} new_bytes={new_bytes} retained_messages={} retained_completed_writes={retained_writes} active_tool_calls=0",
        workload.name(),
        workload.line_count(),
        workload.line_count(),
        state.history().messages().len(),
    );
    measure(workload.name(), &state, workload.visible_controls())
}

fn run_pending_preview() -> Result<()> {
    let mut state = new_state();
    state.add_system_message("Pending preview control".into());
    let tool_call = ToolCall::new(
        ToolCallId::new("pending-preview"),
        "Editing benchmark_preview.rs".into(),
        ToolKind::Write,
        ToolCallStatus::Pending,
        None,
    )
    .with_content(vec![ToolCallContent::Diff {
        path: "benchmark_preview.rs".into(),
        old_text: Some("const PREVIEW_OLD: u64 = 1;\n".into()),
        new_text: "const PREVIEW_NEW: u64 = 2;\n".into(),
    }]);
    let (responder, receiver) = tokio::sync::oneshot::channel();
    state.show_approval(PermissionRequest {
        session_id: SessionId::new("benchmark-main"),
        tool_call,
        message: "Allow benchmark edit?".into(),
        options: vec![PermissionOption {
            id: PermissionOptionId::new("allow-once"),
            label: "Allow Once".into(),
            kind: PermissionOptionKind::AllowOnce,
            is_destructive: false,
        }],
        trust_options: Vec::new(),
        can_reject_with_reason: false,
        responder,
        consent: None,
        workflow_plan: None,
    });
    ensure!(
        state.history().messages().len() == 1,
        "pending control history was lost"
    );
    ensure!(
        state
            .approval()
            .is_some_and(|approval| { approval.tool_call.status() == ToolCallStatus::Pending }),
        "pending preview must not depend on Completed status"
    );
    println!(
        "workload=pending-preview-control old_lines=1 new_lines=1 retained_messages={} pending_approvals=1 activity=Idle",
        state.history().messages().len(),
    );
    let result = measure(
        "pending-preview-control",
        &state,
        &[
            "Permission Required",
            "benchmark_preview.rs",
            "│- const PREVIEW_OLD",
            "│+ const PREVIEW_NEW",
            "Allow Once",
        ],
    );
    // Keep the real permission response endpoint open for every measured draw;
    // no async runtime or agent connection is required to display a request.
    drop(black_box(receiver));
    result
}

fn draw(
    terminal: &mut Terminal<TestBackend>,
    state: &UiState,
    render_state: &mut render::RenderState,
) -> Result<()> {
    terminal.draw(|frame| render::draw(frame, black_box(state), render_state))?;
    black_box(terminal.backend().buffer());
    Ok(())
}

fn verify_visible(
    terminal: &Terminal<TestBackend>,
    workload: &str,
    controls: &[&str],
) -> Result<()> {
    let buffer = terminal.backend().buffer();
    let mut visible = String::with_capacity(buffer.content.len());
    for row in buffer.content.chunks(usize::from(buffer.area.width).max(1)) {
        for cell in row {
            visible.push_str(cell.symbol());
        }
        visible.push('\n');
    }
    for control in controls {
        ensure!(
            visible.contains(control),
            "{workload}: visible positive control {control:?} is missing; refusing an empty/fallback measurement\n{visible}"
        );
    }
    Ok(())
}

fn verify_styles(terminal: &Terminal<TestBackend>, state: &UiState) -> Result<()> {
    let theme = state.theme();
    let buffer = terminal.backend().buffer();
    for (control, color, modifier) in [
        ("You:", Some(theme.user), Modifier::BOLD),
        ("Kiro:", Some(theme.agent), Modifier::BOLD),
        (
            "VISIBLE_THOUGHT_CONTROL",
            Some(theme.muted),
            Modifier::ITALIC,
        ),
        (
            "INDEPENDENT_THOUGHT_CONTROL",
            Some(theme.muted),
            Modifier::ITALIC,
        ),
        ("bold_control", None, Modifier::BOLD),
        ("styled_control", None, Modifier::BOLD),
        (
            "const OLD_CONTROL",
            Some(theme.subdued_negative),
            Modifier::empty(),
        ),
        (
            "const NEW_CONTROL",
            Some(theme.subdued_positive),
            Modifier::empty(),
        ),
        (
            "const PREVIEW_OLD",
            Some(theme.subdued_negative),
            Modifier::empty(),
        ),
        (
            "const PREVIEW_NEW",
            Some(theme.subdued_positive),
            Modifier::empty(),
        ),
    ] {
        let cells = buffer
            .content
            .chunks(usize::from(buffer.area.width).max(1))
            .flat_map(|row| row.windows(control.len()))
            .find(|cells| {
                cells
                    .iter()
                    .zip(control.bytes())
                    .all(|(cell, byte)| cell.symbol().as_bytes() == [byte])
            });
        // Content positive controls decide which rows must be visible in this
        // window; validate exact semantic styles whenever those cells occur.
        if let Some(cells) = cells {
            for cell in cells {
                if let Some(expected) = color {
                    ensure!(
                        cell.fg == expected,
                        "wrong foreground for {control:?}: {:?} != {expected:?}",
                        cell.fg
                    );
                }
                ensure!(
                    cell.modifier.contains(modifier),
                    "wrong modifier for {control:?}"
                );
            }
        }
    }
    Ok(())
}

fn measure(workload: &str, state: &UiState, controls: &[&str]) -> Result<()> {
    let mut session = BenchSession::new(WIDTH, HEIGHT)?;
    session.measure_case(workload, state, controls, "cold", Duration::ZERO)
}

struct BenchSession {
    terminal: Terminal<TestBackend>,
    render_state: render::RenderState,
}

impl BenchSession {
    fn new(width: u16, height: u16) -> Result<Self> {
        Ok(Self {
            terminal: Terminal::new(TestBackend::new(width, height))?,
            render_state: render::RenderState::new(),
        })
    }

    fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        self.terminal.backend_mut().resize(width, height);
        self.terminal.autoresize()?;
        Ok(())
    }

    fn draw(&mut self, state: &UiState) -> Result<()> {
        draw(&mut self.terminal, state, &mut self.render_state)
    }

    fn measure_case(
        &mut self,
        workload: &str,
        state: &UiState,
        controls: &[&str],
        transition: &str,
        mutation: Duration,
    ) -> Result<()> {
        let start = Instant::now();
        self.draw(state)?;
        let update = start.elapsed();
        verify_visible(&self.terminal, workload, controls)?;
        verify_styles(&self.terminal, state)?;
        let mutation_ns = if mutation.is_zero() {
            "not_measured".to_owned()
        } else {
            mutation.as_nanos().to_string()
        };
        println!(
            "transition workload={workload} kind={transition} mutation_ns={} draw_ns={} retained_messages={}",
            mutation_ns,
            update.as_nanos(),
            state.history().messages().len(),
        );
        // Metrics enumerate retained descriptors/backing outside the frame timer.
        // owned_text_bytes is not allocator/RSS/Arc-capacity accounting.
        let metrics = self.render_state.layout_metrics();
        println!(
            "layout workload={workload} prepared_messages={} physical_rows={} runs={} source_slots={} owned_text_bytes={} storage_scope=selected_transcript_not_process_RSS",
            metrics.messages,
            metrics.rows,
            metrics.runs,
            metrics.source_slots,
            metrics.owned_text_bytes,
        );

        for _ in 0..WARMUP_DRAWS {
            self.draw(state)?;
        }
        verify_visible(&self.terminal, workload, controls)?;
        verify_styles(&self.terminal, state)?;
        let mut samples = [Duration::ZERO; SAMPLES];
        for elapsed in &mut samples {
            let start = Instant::now();
            for _ in 0..DRAWS_PER_SAMPLE {
                self.draw(state)?;
            }
            *elapsed = start.elapsed();
        }
        verify_visible(&self.terminal, workload, controls)?;
        verify_styles(&self.terminal, state)?;
        for (index, elapsed) in samples.iter().enumerate() {
            println!(
                "sample workload={workload} index={} draws={DRAWS_PER_SAMPLE} total_ns={} ns_per_frame={:.3}",
                index + 1,
                elapsed.as_nanos(),
                elapsed.as_nanos() as f64 / DRAWS_PER_SAMPLE as f64,
            );
        }
        samples.sort_unstable();
        let median = (samples[SAMPLES / 2 - 1].as_nanos() as f64
            + samples[SAMPLES / 2].as_nanos() as f64)
            / (2 * DRAWS_PER_SAMPLE) as f64;
        let p95_index = (95 * SAMPLES).div_ceil(100) - 1;
        let p95 = samples[p95_index].as_nanos() as f64 / DRAWS_PER_SAMPLE as f64;
        println!(
            "summary workload={workload} median_ns_per_frame={median:.3} p95_ns_per_frame={p95:.3} samples={SAMPLES} draws_per_sample={DRAWS_PER_SAMPLE} visible_controls=passed"
        );
        Ok(())
    }
}

fn agent(state: &mut UiState, text: String, streaming: bool) {
    assert!(
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text,
            is_streaming: streaming,
        }))
    );
}

fn completed_tool(state: &mut UiState, id: &str, kind: ToolKind, text: &str) {
    let input = match kind {
        ToolKind::Execute => serde_json::json!({"command": format!("printf {id}")}),
        _ => serde_json::json!({"path": format!("fixture_{id}.rs")}),
    };
    let mut call = ToolCall::new(
        ToolCallId::new(id),
        format!("Fixture {id}"),
        kind,
        ToolCallStatus::Completed,
        Some(input),
    );
    if kind == ToolKind::Write {
        call = call.with_content(vec![ToolCallContent::Diff {
            path: format!("fixture_{id}.rs"),
            old_text: Some("const OLD_CONTROL: u64 = 1;\n".into()),
            new_text: "const NEW_CONTROL: u64 = 2;\n".into(),
        }]);
    } else {
        call = call
            .with_content(vec![ToolCallContent::Text(text.into())])
            .with_raw_output(Some(serde_json::json!(text)));
    }
    assert!(state.apply_notification(&Notification::ToolCallStarted(call)));
    state.commit_streaming();
}

fn retained_fixture(size: usize) -> Result<UiState> {
    // The explicit limit is part of the fixture, not a production history cap.
    let mut state = UiState::new(size);
    state.set_terminal_size(WIDTH, HEIGHT);
    state.set_appearance(ThemeId::CyrilDark, ColorMode::TrueColor);
    completed_tool(
        &mut state,
        "older-tool",
        ToolKind::Execute,
        "OLDER_TOOL_ORIGINAL",
    );
    state.add_steer_echo("OLDER_RECEIPT_CONTROL");
    state.apply_notification(&Notification::SteeringQueued {
        message: None,
        message_id: Some("steer-older-receipt".into()),
    });
    for index in 2..size - 6 {
        match index % 9 {
            0 => state.add_user_message(&format!("user_{index:05} /workspace/unique_{index}.rs")),
            1 | 2 => agent(
                &mut state,
                format!("unique_markdown_{index:05} **bold** and *emphasis*\n\n- item_{index}\n"),
                false,
            ),
            3 => agent(
                &mut state,
                format!("```rust\nconst UNIQUE_{index:05}: u64 = {index};\n```\n"),
                false,
            ),
            4 => agent(
                &mut state,
                format!("| unique_{index:05} | value |\n|---|---|\n| source | {index} |\n"),
                false,
            ),
            5 => {
                state.apply_notification(&Notification::AgentThought(AgentThought {
                    text: format!("thought_{index:05} keeps chronological reasoning"),
                }));
                state.commit_streaming();
            }
            6 => completed_tool(
                &mut state,
                &format!("read-{index}"),
                ToolKind::Read,
                "read source",
            ),
            7 => completed_tool(
                &mut state,
                &format!("exec-{index}"),
                ToolKind::Execute,
                "execute source",
            ),
            _ => completed_tool(&mut state, &format!("write-{index}"), ToolKind::Write, ""),
        }
    }
    // Fill the area above the source controls with identical retained content,
    // so unique history cannot leak into the visible window at smaller sizes.
    state.add_system_message("VISIBLE_SUFFIX_PADDING\n".repeat(usize::from(HEIGHT)));
    state.add_user_message("VISIBLE_USER_CONTROL /workspace/control.rs");
    agent(
        &mut state,
        "VISIBLE_AGENT_CONTROL **bold_control**\n\n```rust\nfn visible_code_control() {}\n```\n\n| visible_table_control | value |\n|---|---|\n| retained | yes |\n".into(),
        false,
    );
    state.apply_notification(&Notification::AgentThought(AgentThought {
        text: "VISIBLE_THOUGHT_CONTROL".into(),
    }));
    state.commit_streaming();
    completed_tool(&mut state, "visible-write", ToolKind::Write, "");
    completed_tool(
        &mut state,
        "visible-execute",
        ToolKind::Execute,
        "VISIBLE_EXECUTE_CONTROL",
    );
    state.set_activity(cyril_ui::traits::Activity::Idle);
    ensure!(
        state.history().messages().len() == size,
        "fixture silently pruned: expected {size}"
    );
    let markdown_sources = state
        .history()
        .messages()
        .iter()
        .filter(|message| matches!(message.kind(), ChatMessageKind::AgentText(_)))
        .count();
    ensure!(
        size < 1_000 || markdown_sources > 256,
        "fixture did not exceed the Markdown cache"
    );
    println!(
        "fixture retained_messages={size} unique_markdown_sources={markdown_sources} retention_limit={size} suffix_messages=6 construction=outside_timers"
    );
    Ok(state)
}

const SUFFIX_CONTROLS: &[&str] = &[
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

fn run_long_histories() -> Result<()> {
    for size in [100, 1_000, 10_000, 50_000] {
        let mut state = retained_fixture(size)?;
        let mut session = BenchSession::new(WIDTH, HEIGHT)?;
        session.measure_case(
            &format!("history-{size}-follow"),
            &state,
            SUFFIX_CONTROLS,
            "cold",
            Duration::ZERO,
        )?;
        run_appearance_transitions(size, &mut state, &mut session)?;
        run_history_transitions(size, &mut state, &mut session)?;
    }
    run_giant_sources()
}

fn run_history_transitions(
    size: usize,
    state: &mut UiState,
    session: &mut BenchSession,
) -> Result<()> {
    state.chat_scroll_up(usize::MAX);
    session.measure_case(
        &format!("history-{size}-oldest"),
        state,
        &["OLDER_TOOL_ORIGINAL", "OLDER_RECEIPT_CONTROL"],
        "window-only",
        Duration::ZERO,
    )?;
    let start = Instant::now();
    state.apply_notification(&Notification::ToolCallUpdated(
        ToolCall::new(
            ToolCallId::new("older-tool"),
            String::new(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Text("OLDER_TOOL_CHANGED".into())])
        .with_raw_output(Some(serde_json::json!("OLDER_TOOL_CHANGED"))),
    ));
    let mutation = start.elapsed();
    session.measure_case(
        &format!("history-{size}-old-tool-edit"),
        state,
        &["OLDER_TOOL_CHANGED"],
        "incremental-update",
        mutation,
    )?;
    let start = Instant::now();
    agent(
        state,
        "[STEERING steer-older-receipt: RECEIPT_NOTE_CHANGED]".into(),
        true,
    );
    let mutation = start.elapsed();
    ensure!(
        state.streaming_text().is_empty(),
        "receipt unexpectedly changed live source"
    );
    session.measure_case(
        &format!("history-{size}-receipt-edit"),
        state,
        &["RECEIPT_NOTE_CHANGED"],
        "incremental-update",
        mutation,
    )?;
    state.chat_scroll_reset();

    let mut mutation = Duration::ZERO;
    let mut updates = Duration::ZERO;
    for delta in 0..1_000 {
        let text = format!(" live_delta_{delta:04}");
        let start = Instant::now();
        agent(state, text, true);
        mutation += start.elapsed();
        let start = Instant::now();
        session.draw(state)?;
        updates += start.elapsed();
    }
    ensure!(
        state.history().messages().len() == size,
        "live deltas altered committed retention"
    );
    verify_visible(&session.terminal, "live-deltas", &["live_delta_0999"])?;
    println!(
        "updates workload=history-{size}-1000-live-deltas notifications=1000 mutation_ns={} update_draw_ns={} mutation_and_update_excluded_from_warm=true",
        mutation.as_nanos(),
        updates.as_nanos()
    );
    session.measure_case(
        &format!("history-{size}-stable-live"),
        state,
        &["live_delta_0999"],
        "already-updated",
        Duration::ZERO,
    )?;
    let start = Instant::now();
    state.apply_notification(&Notification::AgentThought(AgentThought {
        text: "INDEPENDENT_THOUGHT_CONTROL".into(),
    }));
    let mutation = start.elapsed();
    session.measure_case(
        &format!("history-{size}-thought"),
        state,
        &["INDEPENDENT_THOUGHT_CONTROL"],
        "live-thought-update",
        mutation,
    )?;
    // Flush is an explicit mutation, not part of the following stable samples.
    let start = Instant::now();
    state.commit_streaming();
    state.add_user_message("APPEND_PRUNE_CONTROL");
    let mutation = start.elapsed();
    ensure!(
        state.history().messages().len() == size,
        "append failed to enforce prefix retention"
    );
    ensure!(!state.history().messages().iter().any(|message| matches!(message.kind(), ChatMessageKind::ToolCall(call) if call.id().as_str() == "older-tool")), "prefix prune retained evicted tool");
    state.set_activity(cyril_ui::traits::Activity::Idle);
    session.measure_case(
        &format!("history-{size}-append-prune"),
        state,
        &["APPEND_PRUNE_CONTROL"],
        "append-prefix-prune",
        mutation,
    )?;

    let child = SessionId::new(format!("benchmark-child-{size}"));
    state.apply_subagent_notification(
        &child,
        &Notification::AgentMessage(AgentMessage {
            text: "CHILD_COMMITTED_CONTROL".into(),
            is_streaming: false,
        }),
    );
    state.apply_subagent_notification(
        &child,
        &Notification::AgentMessage(AgentMessage {
            text: "CHILD_LIVE_CONTROL".into(),
            is_streaming: true,
        }),
    );
    state.chat_scroll_up(usize::MAX); // Child must ignore main browse position.
    ensure!(state.focus_subagent(child));
    session.measure_case(
        &format!("history-{size}-child-focus"),
        state,
        &[
            "[Esc] Back",
            "CHILD_COMMITTED_CONTROL",
            "CHILD_LIVE_CONTROL",
        ],
        "selected-transcript-cold",
        Duration::ZERO,
    )?;
    state.unfocus_subagent();
    state.chat_scroll_reset();
    session.measure_case(
        &format!("history-{size}-main-refocus"),
        state,
        &["APPEND_PRUNE_CONTROL"],
        "selected-transcript-cold",
        Duration::ZERO,
    )?;

    Ok(())
}

fn run_giant_sources() -> Result<()> {
    const ROWS: usize = 80_000;
    // Match the fixed scrollbar gutter without changing the outer backend geometry.
    let content_width = usize::from(WIDTH - 1);
    for unbroken in [false, true] {
        let mut source = String::with_capacity(ROWS * content_width);
        for row in 0..ROWS {
            if unbroken {
                let token = format!("unbroken_{row:05}_");
                source.push_str(&token);
                source.extend(std::iter::repeat_n('x', content_width - token.len()));
            } else {
                writeln!(
                    source,
                    "markdown_{row:05} **styled_control** source retained beyond cache and u16 rows.  "
                )?;
            }
        }
        ensure!(
            source.len() > 2 * 1_024 * 1_024,
            "giant fixture is not multi-megabyte"
        );
        ensure!(
            !unbroken || !source.contains('\n'),
            "giant line contains a line break"
        );
        let source_bytes = source.len();
        let mut state = new_state();
        agent(&mut state, source, false);
        state.set_activity(cyril_ui::traits::Activity::Idle);
        let mut session = BenchSession::new(WIDTH, HEIGHT)?;
        let kind = if unbroken {
            "giant-unbroken"
        } else {
            "giant-markdown"
        };
        session.measure_case(
            &format!("{kind}-bottom"),
            &state,
            &[if unbroken {
                "unbroken_79999_"
            } else {
                "markdown_79999"
            }],
            "cold-preparation",
            Duration::ZERO,
        )?;
        let total = session.render_state.layout_metrics().rows;
        ensure!(
            total > usize::from(u16::MAX),
            "fixture did not cross the old scroll ceiling"
        );
        ensure!(
            session.render_state.layout_metrics().messages == 1,
            "giant source was not retained as one message"
        );
        println!(
            "fixture workload={kind} source_bytes={source_bytes} logical_lines={} physical_rows={total}",
            if unbroken { 1 } else { ROWS }
        );
        // Put the selected source row at the window's bottom (clamped at top).
        // One agent-label row precedes source row zero. This source arithmetic
        // does not assume the input widget's height or duplicate root geometry.
        for (name, marker_row) in [("top", 0), ("middle", 40_000), ("deep-above-u16", 70_000)] {
            state.chat_scroll_reset();
            state.chat_scroll_up(total.saturating_sub(marker_row + 2));
            let marker = if unbroken {
                format!("unbroken_{marker_row:05}_")
            } else {
                format!("markdown_{marker_row:05}")
            };
            session.measure_case(
                &format!("{kind}-{name}"),
                &state,
                &[&marker],
                "window-only",
                Duration::ZERO,
            )?;
        }
    }
    Ok(())
}

fn run_appearance_transitions(
    size: usize,
    state: &mut UiState,
    session: &mut BenchSession,
) -> Result<()> {
    for (width, height, transition) in [
        (80, 48, "bulk-width"),
        (160, 48, "bulk-width"),
        (160, 12, "height-only"),
        (160, 48, "height-only"),
    ] {
        let start = Instant::now();
        state.set_terminal_size(width, height);
        session.resize(width, height)?;
        let mutation = start.elapsed();
        let controls = if height == HEIGHT {
            SUFFIX_CONTROLS
        } else {
            &["VISIBLE_EXECUTE_CONTROL"]
        };
        session.measure_case(
            &format!("history-{size}-{width}x{height}"),
            state,
            controls,
            transition,
            mutation,
        )?;
    }
    for (theme, color) in [
        (ThemeId::CyrilDark, ColorMode::None),
        (ThemeId::CyrilLight, ColorMode::None),
        (ThemeId::CyrilLight, ColorMode::TrueColor),
        (ThemeId::CyrilDark, ColorMode::TrueColor),
    ] {
        let start = Instant::now();
        state.set_appearance(theme, color);
        let mutation = start.elapsed();
        session.measure_case(
            &format!("history-{size}-{theme:?}-{color:?}"),
            state,
            SUFFIX_CONTROLS,
            "bulk-theme",
            mutation,
        )?;
    }
    Ok(())
}
