//! Full-root rendering measurements; no wall-clock acceptance threshold.
//!
//! Run: `cargo bench -p cyril-ui --bench conversation_render -- completed-write`.
//! Record the checked revision, CPU, `rustc -Vv`, and exact Cargo invocation with
//! the output. Optional `CYRIL_BENCH_*` labels attach those facts to the receipt.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::{Duration, Instant};

use anyhow::{Result, bail, ensure};
use cyril_core::types::{
    AgentMessage, Notification, PermissionOption, PermissionOptionId, PermissionOptionKind,
    PermissionRequest, SessionId, StopReason, ToolCall, ToolCallContent, ToolCallId,
    ToolCallStatus, ToolKind,
};
use cyril_ui::render;
use cyril_ui::state::UiState;
use cyril_ui::theme::{ColorMode, ThemeId};
use cyril_ui::traits::{ChatMessageKind, TuiState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const WIDTH: u16 = 160;
const HEIGHT: u16 = 48;
const WARMUP_DRAWS: usize = 100;
const SAMPLES: usize = 30;
const DRAWS_PER_SAMPLE: usize = 100;
const RETAINED_MESSAGES: usize = 2;
const USAGE: &str = "Usage: conversation_render completed-write [--bench]\n\
    Runs sparse 2,000-line and dense 20,000-line completed Writes, then a pending preview control.\n\
    --bench is accepted for Cargo's benchmark invocation; unknown modes/arguments are errors.";
const CODE_CONTROL: &str =
    "Write rendering fixture:\n```rust\nfn benchmark_control() -> bool { true }\n```\n";

#[derive(Clone, Copy)]
enum Mode {
    CompletedWrite,
    Help,
}

fn parse_mode(arguments: impl IntoIterator<Item = OsString>) -> Result<Mode> {
    let mut mode = None;
    let mut cargo_bench = false;
    for argument in arguments {
        match argument.to_str() {
            Some("completed-write") if mode.is_none() => mode = Some(Mode::CompletedWrite),
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
            report_environment();
            for workload in [WriteWorkload::Sparse, WriteWorkload::Dense] {
                run_completed_write(workload)?;
            }
            run_pending_preview()?;
        }
    }
    Ok(())
}

fn report_environment() {
    println!(
        "benchmark=conversation_render mode=completed-write package_version={} os={} arch={} family={} debug_assertions={}",
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
        "timing=Instant cold_scope=first_draw_per_fresh_state_and_terminal process_caches=shared_between_cases construction_and_validation=excluded warmup_draws={WARMUP_DRAWS} samples={SAMPLES} draws_per_sample={DRAWS_PER_SAMPLE}"
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
        state.messages().len() == RETAINED_MESSAGES,
        "fixture was evicted"
    );
    ensure!(
        state.active_tool_calls().is_empty(),
        "fixture is not historical"
    );
    let retained_writes = state
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
        state.messages().len(),
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
        state.messages().len() == 1,
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
        state.messages().len(),
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

fn draw(terminal: &mut Terminal<TestBackend>, state: &UiState) -> Result<()> {
    terminal.draw(|frame| render::draw(frame, black_box(state)))?;
    black_box(terminal.backend().buffer());
    Ok(())
}

fn verify_visible(
    terminal: &Terminal<TestBackend>,
    workload: &str,
    controls: &[&str],
) -> Result<()> {
    let buffer = terminal.backend().buffer();
    let mut visible = String::with_capacity(usize::from(WIDTH) * usize::from(HEIGHT));
    for row in buffer.content.chunks(usize::from(WIDTH)) {
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

fn measure(workload: &str, state: &UiState, controls: &[&str]) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT))?;
    let start = Instant::now();
    draw(&mut terminal, state)?;
    let cold = start.elapsed();
    verify_visible(&terminal, workload, controls)?;
    println!(
        "cold workload={workload} draws=1 total_ns={}",
        cold.as_nanos()
    );

    // The cold root draw initializes the real diff, markdown and syntax paths;
    // fixture construction, first-use caches and these draws are not samples.
    for _ in 0..WARMUP_DRAWS {
        draw(&mut terminal, state)?;
    }
    verify_visible(&terminal, workload, controls)?;
    let mut samples = [Duration::ZERO; SAMPLES];
    for elapsed in &mut samples {
        let start = Instant::now();
        for _ in 0..DRAWS_PER_SAMPLE {
            draw(&mut terminal, state)?;
        }
        *elapsed = start.elapsed();
    }
    verify_visible(&terminal, workload, controls)?;
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
        "summary workload={workload} median_ns_per_frame={median:.3} p95_ns_per_frame={p95:.3} samples={SAMPLES} draws_per_sample={DRAWS_PER_SAMPLE} visible_controls=passed retained_messages={}",
        state.messages().len(),
    );
    Ok(())
}
