use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use futures_util::{FutureExt, StreamExt};
use ratatui::DefaultTerminal;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::capture_forwarder::CaptureForwarder;
use crate::memory_runtime::{
    FirstPromptContextError, MemoryRuntimeHandle, ProjectBinding, ProjectMemory,
};
use cyril_core::commands::{
    CommandContext, CommandRegistry, CommandResult, CommandResultKind, MemoryCommandAction,
};
use cyril_core::protocol::bridge::{BridgeHandle, BridgeSender};
use cyril_core::session::SessionController;
use cyril_core::types::*;
use cyril_core::usage::{
    KiroSidecarKind, UsageEnrichmentHandle, UsageEnrichmentResult, UsageLog, UsageObserver,
    UsageSnapshotHandle, UsageSnapshotResult, UsageWrite, spawn_usage_enrichment_worker,
};
use cyril_core::workflow::WorkflowTracker;
use cyril_ui::state::{AutocompleteAction, UiState};
use cyril_ui::traits::{Activity, TuiState, approval_origin_label};

use cyril_core::types::code_panel::CodeCommandResponse;

/// Lines per mouse wheel tick (finer-grained than keyboard half-page scroll).
const MOUSE_SCROLL_LINES: usize = 3;
const MAX_ENRICHMENT_RETRIES: u8 = 1;

fn reserve_enrichment_retry(retryable: bool, attempts: &mut u8) -> bool {
    if !retryable || *attempts >= MAX_ENRICHMENT_RETRIES {
        return false;
    }
    *attempts += 1;
    true
}

/// Spawn the voice engine when the `voice` feature is enabled. This is the only
/// feature-gated site — everything downstream operates on the always-present
/// `Option<VoiceHandle>` and cyril-core voice types, so the `select!` arm and
/// command routing need no `#[cfg]`.
#[cfg(feature = "voice")]
fn spawn_voice_engine() -> Option<cyril_core::voice::VoiceHandle> {
    Some(cyril_voice::spawn_voice())
}

#[cfg(not(feature = "voice"))]
fn spawn_voice_engine() -> Option<cyril_core::voice::VoiceHandle> {
    None
}

pub struct App {
    bridge_sender: BridgeSender,
    notification_rx: mpsc::Receiver<RoutedNotification>,
    permission_rx: mpsc::Receiver<PermissionRequest>,
    source_rx: Option<mpsc::Receiver<SourceTurnEvent>>,
    bridge_completion_rx: Option<oneshot::Receiver<()>>,
    capture_forwarder: Option<CaptureForwarder>,
    ui_state: UiState,
    session: SessionController,
    commands: CommandRegistry,
    redraw_needed: bool,
    last_activity: Instant,
    /// The cwd kiro-cli was spawned in — used to resolve the active agent's
    /// workspace config (`<cwd>/.kiro/agents/`) when persisting trust grants.
    cwd: PathBuf,
    /// Canonical live usage observer and durable aggregate source.
    usage_observer: UsageObserver,
    usage_log: UsageLog,
    usage_enrichment: UsageEnrichmentHandle,
    usage_enrichment_rx: mpsc::UnboundedReceiver<UsageEnrichmentResult>,
    /// Requests snapshots off the event loop. The loop never computes one
    /// itself (cyril-nanu C1).
    usage_snapshot: UsageSnapshotHandle,
    usage_snapshot_rx: mpsc::UnboundedReceiver<UsageSnapshotResult>,
    agent_engine: AgentEngine,
    enrichment_requests: HashMap<UsageRecordId, (SessionId, KiroSidecarKind)>,
    failed_enrichments: BTreeSet<UsageRecordId>,
    enrichment_attempts: HashMap<UsageRecordId, u8>,
    /// Voice-input engine handle (ROADMAP CN2). `None` when the `voice` feature
    /// is off (or the engine could not start). The type lives in cyril-core so
    /// this field and its `select!` arm compile regardless of the feature.
    voice: Option<cyril_core::voice::VoiceHandle>,
    memory_runtime: Option<MemoryRuntimeHandle>,
    memory_status: MemoryStatusView,
    /// How the startup workspace resolved against project memory. Lesson
    /// commands and first-prompt injection need `Bound`; the other states
    /// carry the reason the user sees.
    project_binding: ProjectBinding,
    /// Results of memory work that ran off the event loop (`/memory`
    /// commands, first-prompt lesson lookups). The `select!` arm drains it;
    /// nothing awaits the companion inline.
    memory_task_tx: mpsc::UnboundedSender<MemoryTaskResult>,
    memory_task_rx: mpsc::UnboundedReceiver<MemoryTaskResult>,
    /// The session whose next non-empty prompt gets the project-lessons
    /// block prepended. Armed on `SessionCreated`, consumed by the first
    /// prompt, and re-armed only when the companion was still starting.
    first_prompt_lessons_pending: Option<SessionId>,
    /// Authoritative "is voice capturing?" intent. Flipped on each successful
    /// Start/Stop send (and cleared on engine `Error`). Toggling reads this —
    /// NOT the lagging `ui_state.voice_status()` projection — so rapid `/voice`
    /// presses (drained as a batch before the engine's Status echo arrives)
    /// alternate Start/Stop correctly instead of both sending `Start`. In V1a
    /// the engine only changes capture state in response to commands, so this
    /// optimistic model tracks it exactly; see the V1b note in `handle_voice_event`.
    voice_active: bool,
    /// One-shot `--prompt` text awaiting the initial session (cyril-0ffy).
    /// Set by `create_initial_session`, consumed (`take()`) by the first
    /// main-routed `SessionCreated` in `handle_notification` — so the agent
    /// receives exactly one `session/prompt` and a later `/new` never replays
    /// it. `None` for interactive startup.
    startup_prompt: Option<String>,
    /// Workspace-global workflow lifecycle state (cyril-6beh C12). Every
    /// `Notification::Workflow` frame is applied here — exactly once, by
    /// value — before any SessionController/UiState consumer sees it, and
    /// workflow frames are never forwarded onward.
    workflow_tracker: WorkflowTracker,
    /// Test-only dispatch counters (cyril-6beh slice 22): incremented at the
    /// actual tracker/session/UI call sites so App tests can prove a workflow
    /// frame branches before every other consumer and is consumed exactly
    /// once. Compiled out of production builds.
    #[cfg(test)]
    workflow_apply_calls: u64,
    #[cfg(test)]
    workflow_stream_apply_calls: u64,
    #[cfg(test)]
    subagent_ui_apply_calls: u64,
    #[cfg(test)]
    session_apply_calls: u64,
    #[cfg(test)]
    ui_apply_calls: u64,
}
/// Outcome of memory work that ran on a spawned task. Delivered back to the
/// event loop over `App::memory_task_rx`.
enum MemoryTaskResult {
    /// Rendered `/memory` command output, ready to show.
    CommandOutput(String),
    /// A fresh session's first prompt, held back while its query-aware
    /// context loads.
    FirstPromptContext {
        session_id: SessionId,
        content_blocks: Vec<String>,
        outcome: Result<Option<cyril_memory::PromptContext>, FirstPromptContextError>,
    },
}

// The persistence boundary: memory-domain enums become core view enums here
// and nowhere else. Each side keeps its own single-variant vocabulary so a
// future provenance is an additive change on both.
fn memory_lesson_view(lesson: &cyril_memory::LessonRecord) -> MemoryLessonView {
    let provenance = match lesson.provenance() {
        cyril_memory::LessonProvenance::UserExplicit => MemoryLessonProvenance::UserExplicit,
    };
    let trust = match lesson.trust() {
        cyril_memory::LessonTrust::Instruction => MemoryLessonTrust::Instruction,
    };
    let status = match lesson.status() {
        cyril_memory::LessonStatus::Active => MemoryLessonStatus::Active,
        cyril_memory::LessonStatus::Invalidated => MemoryLessonStatus::Invalidated,
    };
    MemoryLessonView::new(
        lesson.id().to_string(),
        lesson.content().to_owned(),
        cyril_core::types::MemoryLessonMetadataView::new(
            provenance,
            trust,
            status,
            lesson.supersedes_id().map(|id| id.to_string()),
            lesson.created_at_ms(),
            lesson.updated_at_ms(),
        ),
    )
}

fn memory_teach_view(
    result: &cyril_memory::TeachResponse,
    operation: MemoryTeachOperation,
) -> MemoryTeachView {
    MemoryTeachView::new(
        operation,
        memory_lesson_view(result.lesson()),
        result.created(),
    )
}
fn memory_source_turn_status(status: cyril_memory::SourceTurnStatus) -> MemorySourceTurnStatus {
    match status {
        cyril_memory::SourceTurnStatus::Incomplete => MemorySourceTurnStatus::Incomplete,
        cyril_memory::SourceTurnStatus::Finished(disposition) => match disposition {
            cyril_memory::SourceTurnDisposition::Completed => MemorySourceTurnStatus::Completed,
            cyril_memory::SourceTurnDisposition::Interrupted => MemorySourceTurnStatus::Interrupted,
            cyril_memory::SourceTurnDisposition::Failed => MemorySourceTurnStatus::Failed,
            cyril_memory::SourceTurnDisposition::Abandoned => MemorySourceTurnStatus::Abandoned,
            cyril_memory::SourceTurnDisposition::CaptureOverflow => {
                MemorySourceTurnStatus::CaptureOverflow
            }
        },
    }
}

fn memory_source_turn_summary_view(
    turn: &cyril_memory::SourceTurnSummary,
) -> MemorySourceTurnSummaryView {
    MemorySourceTurnSummaryView::new(
        turn.id().to_string(),
        turn.prompt_preview().to_owned(),
        turn.tool_count(),
        MemorySourceTurnSummaryMetadataView::new(
            turn.session_id().as_str().to_owned(),
            turn.bridge_turn_id(),
            memory_source_turn_status(turn.status()),
            turn.started_at_ms(),
            turn.finished_at_ms(),
        ),
    )
}

fn memory_bounded_text_view(value: &cyril_memory::BoundedText) -> MemoryBoundedTextView {
    MemoryBoundedTextView::new(value.text().to_owned(), value.truncated_chars())
}

fn memory_source_tool_view(tool: &cyril_memory::ToolSummary) -> MemorySourceToolView {
    MemorySourceToolView::new(
        tool.tool_id().as_str().to_owned(),
        memory_bounded_text_view(tool.name()),
        tool.status().to_owned(),
        memory_bounded_text_view(tool.input()),
        memory_bounded_text_view(tool.result()),
        tool.capture_truncated_chars(),
    )
}

fn memory_source_turn_view(turn: &cyril_memory::SourceTurnRecord) -> MemorySourceTurnView {
    MemorySourceTurnView::new(
        turn.id().to_string(),
        turn.prompt().text().to_owned(),
        turn.assistant().text().to_owned(),
        turn.tools().iter().map(memory_source_tool_view).collect(),
        turn.omitted_tool_count(),
        MemorySourceTurnMetadataView::new(
            turn.session_id().as_str().to_owned(),
            turn.bridge_turn_id(),
            memory_source_turn_status(turn.status()),
            turn.source_hash().map(hex::encode),
            turn.started_at_ms(),
            turn.finished_at_ms(),
            turn.next_sequence(),
        ),
    )
}

/// Execute one `/memory` lesson command against the bound project and render
/// the outcome. Runs on a spawned task so the companion round trip (a connect
/// and a request, each bounded by the runtime request timeout) never stalls
/// the event loop; the text comes back as `MemoryTaskResult::CommandOutput`.
async fn run_memory_action(memory: ProjectMemory, action: MemoryCommandAction) -> String {
    match action {
        MemoryCommandAction::Teach { text } => match cyril_memory::LessonText::new(&text) {
            Ok(text) => match memory.teach(text).await {
                Ok(result) => cyril_ui::memory_format::format_memory_teach(&memory_teach_view(
                    &result,
                    MemoryTeachOperation::Teach,
                )),
                Err(error) => format!("Memory error: {error}"),
            },
            Err(error) => format!("Memory lesson rejected: {error}"),
        },
        MemoryCommandAction::Replace { lesson_id, text } => {
            match (
                lesson_id.parse::<cyril_memory::LessonId>(),
                cyril_memory::LessonText::new(&text),
            ) {
                (Ok(lesson_id), Ok(text)) => match memory.replace(lesson_id, text).await {
                    Ok(result) => cyril_ui::memory_format::format_memory_teach(&memory_teach_view(
                        &result,
                        MemoryTeachOperation::Replace,
                    )),
                    Err(error) => format!("Memory error: {error}"),
                },
                (Err(error), _) => format!("Memory lesson ID rejected: {error}"),
                (_, Err(error)) => format!("Memory lesson rejected: {error}"),
            }
        }
        MemoryCommandAction::List => match memory.list().await {
            Ok(result) => cyril_ui::memory_format::format_memory_list(&MemoryLessonListView::new(
                result.lessons().iter().map(memory_lesson_view).collect(),
                result.omitted_count(),
                result.corrupt_count(),
            )),
            Err(error) => format!("Memory error: {error}"),
        },
        MemoryCommandAction::Inspect { lesson_id } => {
            match lesson_id.parse::<cyril_memory::LessonId>() {
                Ok(lesson_id) => match memory.inspect(lesson_id).await {
                    Ok(result) => {
                        cyril_ui::memory_format::format_memory_lesson(&memory_lesson_view(&result))
                    }
                    Err(error) => format!("Memory error: {error}"),
                },
                Err(error) => format!("Memory lesson ID rejected: {error}"),
            }
        }
        MemoryCommandAction::Turns => match memory.list_turns().await {
            Ok(result) => {
                cyril_ui::memory_format::format_memory_turn_list(&MemorySourceTurnListView::new(
                    result
                        .turns()
                        .iter()
                        .map(memory_source_turn_summary_view)
                        .collect(),
                    result.omitted_count(),
                    result.corrupt_count(),
                ))
            }
            Err(error) => format!("Memory error: {error}"),
        },
        MemoryCommandAction::InspectTurn { source_turn_id } => {
            match source_turn_id.parse::<cyril_memory::SourceTurnId>() {
                Ok(source_turn_id) => match memory.inspect_turn(source_turn_id).await {
                    Ok(result) => cyril_ui::memory_format::format_memory_turn(
                        &memory_source_turn_view(&result),
                    ),
                    Err(error) => format!("Memory error: {error}"),
                },
                Err(error) => format!("Memory source turn ID rejected: {error}"),
            }
        }
    }
}

/// Everything the App needs to record and display usage.
///
/// Bundled because the three always travel together and are always built in
/// the same place. Passing them separately pushed `App::new` past clippy's
/// argument limit, and the limit was making a fair point: this is one concern
/// — the usage log, the worker that reads it, and the channel its results
/// arrive on — not three unrelated parameters.
pub struct UsageWiring {
    pub log: UsageLog,
    pub snapshot: UsageSnapshotHandle,
    pub snapshot_rx: mpsc::UnboundedReceiver<UsageSnapshotResult>,
}

impl App {
    /// Build the app from the UI config.
    ///
    /// Takes `&UiConfig` rather than loose scalars on purpose (cyril-nd4h): the
    /// destructure below is exhaustive, so a new `UiConfig` field cannot be
    /// added without the compiler demanding a decision about consuming it here.
    /// Two fields shipped serialized and documented for months while nothing
    /// read them; this seam is what stops that recurring.
    pub fn new(
        bridge: BridgeHandle,
        ui: &config::UiConfig,
        cwd: PathBuf,
        hooks: cyril_core::commands::HooksCommandSource,
        workflows: cyril_core::commands::WorkflowCommandSource,
        usage: UsageWiring,
        agent_engine: AgentEngine,
    ) -> Self {
        let UsageWiring {
            log: usage_log,
            snapshot: usage_snapshot,
            snapshot_rx: usage_snapshot_rx,
        } = usage;
        // EXHAUSTIVE ON PURPOSE -- no `..`. Adding a UiConfig field must fail
        // compilation here rather than join the ranks of the silently ignored.
        let &config::UiConfig {
            max_messages,
            mouse_capture,
        } = ui;
        let (bridge_sender, notification_rx, permission_rx, source_rx, bridge_completion_rx) =
            bridge.split();
        let (usage_enrichment, usage_enrichment_rx) = spawn_usage_enrichment_worker();
        let (memory_task_tx, memory_task_rx) = mpsc::unbounded_channel();
        let commands = CommandRegistry::with_builtins_and_usage(
            hooks,
            workflows,
            cyril_core::commands::UsageAccountCommandSource::resolve(agent_engine),
        );
        let info: Vec<(String, Option<String>)> = commands
            .all_commands()
            .iter()
            .map(|c| {
                let desc = c.description();
                (
                    c.name().to_string(),
                    Some(desc.to_string()).filter(|s| !s.is_empty()),
                )
            })
            .collect();
        let mut ui_state = UiState::new(max_messages);
        ui_state.set_command_info(info);
        // This is the ONE read of the configured mouse mode (cyril-nd4h).
        // main.rs derives the terminal's startup mode from `mouse_captured()`
        // rather than reading the config a second time, so the flag and the
        // terminal cannot disagree and Ctrl+M can never start out inverted.
        ui_state.set_mouse_captured(mouse_capture);
        Self {
            bridge_sender,
            notification_rx,
            permission_rx,
            source_rx: Some(source_rx),
            bridge_completion_rx: Some(bridge_completion_rx),
            capture_forwarder: None,
            ui_state,
            session: SessionController::new(),
            commands,
            redraw_needed: true,
            last_activity: Instant::now(),
            cwd,
            usage_observer: UsageObserver::new(),
            enrichment_requests: HashMap::new(),
            failed_enrichments: BTreeSet::new(),
            enrichment_attempts: HashMap::new(),
            usage_log,
            usage_enrichment,
            usage_enrichment_rx,
            usage_snapshot,
            usage_snapshot_rx,
            agent_engine,
            voice: spawn_voice_engine(),
            memory_runtime: None,
            memory_status: MemoryStatusView::default(),
            project_binding: ProjectBinding::Disabled,
            memory_task_tx,
            memory_task_rx,
            first_prompt_lessons_pending: None,
            voice_active: false,
            startup_prompt: None,
            workflow_tracker: WorkflowTracker::new(),
            #[cfg(test)]
            workflow_apply_calls: 0,
            #[cfg(test)]
            workflow_stream_apply_calls: 0,
            #[cfg(test)]
            subagent_ui_apply_calls: 0,
            #[cfg(test)]
            session_apply_calls: 0,
            #[cfg(test)]
            ui_apply_calls: 0,
        }
    }

    fn begin_usage_turn(&mut self, session_id: &SessionId) -> bool {
        let timestamp_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis().min(u128::from(u64::MAX)) as u64,
            Err(error) => {
                tracing::error!(error = %error, "system clock predates Unix epoch; usage turn not recorded");
                self.ui_state
                    .add_system_message("Usage recording failed: system clock is invalid.".into());
                return false;
            }
        };
        let context = TurnUsageContext::new(
            session_id.clone(),
            self.cwd.to_string_lossy(),
            self.session.current_model(),
            UsageAgentType::Main,
        );
        let sidecar_kind = match self.agent_engine {
            AgentEngine::V2 => cyril_core::usage::KiroSidecarKind::V2,
            AgentEngine::Kas => cyril_core::usage::KiroSidecarKind::Kas,
        };
        match self.usage_observer.begin_turn(
            context,
            Instant::now(),
            timestamp_ms,
            Some(sidecar_kind),
        ) {
            Ok(()) => true,
            Err(error) => {
                tracing::error!(session_id = %session_id, error = %error, "usage turn start failed");
                self.ui_state
                    .add_system_message(format!("Usage recording failed: {error}"));
                false
            }
        }
    }

    async fn dispatch_deferred_command(&mut self, deferred: BridgeCommand) {
        // SendPrompt triggers a real turn → mark session Busy. Session-management
        // commands do not. See `/code` (busy) versus `/rewind` (not busy).
        if let BridgeCommand::SendPrompt { session_id, prompt } = deferred {
            if let Err(error) = self
                .send_prompt(session_id, prompt.original_blocks().to_vec())
                .await
            {
                tracing::warn!(error = %error, "failed to send deferred prompt");
                self.ui_state.set_activity(Activity::Idle);
                self.ui_state
                    .add_system_message("Failed to dispatch follow-up command to agent.".into());
            }
            return;
        }
        if let Err(error) = self.bridge_sender.send(deferred).await {
            tracing::warn!(error = %error, "failed to send deferred bridge command");
            self.ui_state.set_activity(Activity::Idle);
            self.ui_state
                .add_system_message("Failed to dispatch follow-up command to agent.".into());
        }
    }

    /// The one seam every prompt leaves through — typed submit, the startup
    /// `--prompt`, and `/code` follow-ups all land here — so first-prompt
    /// project lessons (cyril-ezgo) are prepended in exactly one place.
    ///
    /// A fresh session's first non-empty prompt is held while its lessons
    /// load on a spawned task (bounded by
    /// [`crate::memory_runtime::FIRST_PROMPT_CONTEXT_TIMEOUT`]); the event
    /// loop keeps servicing input and agent traffic, and the prompt goes out
    /// from `handle_memory_task_result`. Every other prompt is sent directly.
    async fn send_prompt(
        &mut self,
        session_id: SessionId,
        content_blocks: Vec<String>,
    ) -> cyril_core::Result<()> {
        if self.first_prompt_lessons_pending.as_ref() == Some(&session_id)
            && let Some(query) = content_blocks.first().cloned()
            && let Some(memory) = self.project_binding.memory()
        {
            // Consumed now — the result handler re-arms it only when the
            // companion had not finished starting, so a second Enter in the
            // window is a plain prompt (it will be routed as a steer while
            // the session is Busy).
            self.first_prompt_lessons_pending = None;
            self.session.set_status(SessionStatus::Busy);
            let memory = memory.clone();
            let results = self.memory_task_tx.clone();
            tokio::spawn(async move {
                let outcome = memory.first_prompt_context(query).await;
                if results
                    .send(MemoryTaskResult::FirstPromptContext {
                        session_id,
                        content_blocks,
                        outcome,
                    })
                    .is_err()
                {
                    tracing::debug!("first-prompt context dropped: app is gone");
                }
            });
            return Ok(());
        }
        self.dispatch_prompt(session_id, PromptEnvelope::original(content_blocks))
            .await
    }

    /// Send a prompt to the bridge now, with usage bookkeeping. Busy is set
    /// only once the bridge accepted it.
    async fn dispatch_prompt(
        &mut self,
        session_id: SessionId,
        prompt: PromptEnvelope,
    ) -> cyril_core::Result<()> {
        let usage_started = self.begin_usage_turn(&session_id);
        if let Err(error) = self
            .bridge_sender
            .send(BridgeCommand::SendPrompt {
                session_id: session_id.clone(),
                prompt,
            })
            .await
        {
            if usage_started {
                self.usage_observer.abort_turn(&session_id);
            }
            return Err(error);
        }
        self.session.set_status(SessionStatus::Busy);
        Ok(())
    }

    /// Apply the outcome of memory work that ran off the event loop.
    async fn handle_memory_task_result(&mut self, result: MemoryTaskResult) {
        match result {
            MemoryTaskResult::CommandOutput(text) => {
                self.ui_state.add_system_message(text);
            }
            MemoryTaskResult::FirstPromptContext {
                session_id,
                content_blocks,
                outcome,
            } => {
                let prepared_context = match outcome {
                    Ok(Some(context)) => Some(context.text().to_owned()),
                    Ok(None) => None,
                    Err(error) => {
                        tracing::warn!(
                            session_id = %session_id,
                            error = %error,
                            "first-prompt context unavailable; prompt sent without it"
                        );
                        if error.retry_on_next_prompt()
                            && self.session.id() == Some(&session_id)
                            && self.first_prompt_lessons_pending.is_none()
                        {
                            self.first_prompt_lessons_pending = Some(session_id.clone());
                        }
                        None
                    }
                };
                let prompt = PromptEnvelope::prepared(content_blocks, prepared_context);
                if let Err(error) = self.dispatch_prompt(session_id, prompt).await {
                    tracing::warn!(error = %error, "failed to send prompt after lesson lookup");
                    // Busy was taken optimistically when the lookup started.
                    self.session.set_status(SessionStatus::Active);
                    self.ui_state.set_activity(Activity::Idle);
                    self.ui_state
                        .add_system_message("Failed to send prompt to agent.".into());
                }
            }
        }
        self.redraw_needed = true;
    }
    /// Whether mouse capture should be active, per `ui.mouse_capture`.
    ///
    /// `main.rs` calls this to decide whether to issue `EnableMouseCapture` at
    /// startup, instead of reading the config itself — one read, two consumers
    /// (cyril-nd4h claim C3). `ui_state` stays private.
    pub fn mouse_captured(&self) -> bool {
        self.ui_state.mouse_captured()
    }

    /// Correct the mouse-capture flag when the terminal refuses the mode.
    ///
    /// Startup asks the terminal for capture only when configured; if that
    /// request fails, the flag has to follow, or `UiState` claims a mode the
    /// terminal is not in and the first `Ctrl+M` press flips to the state
    /// already in effect — the inverted-toggle failure again, arriving by a
    /// different route. This is the same rollback the `Ctrl+M` handler already
    /// performs when `execute!` fails.
    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.ui_state.set_mouse_captured(captured);
    }
    pub(crate) fn set_memory_runtime(
        &mut self,
        memory_runtime: MemoryRuntimeHandle,
        project_binding: ProjectBinding,
    ) {
        match memory_runtime.status() {
            crate::memory_runtime::MemoryRuntimeStatus::Failed(failure)
            | crate::memory_runtime::MemoryRuntimeStatus::Degraded(failure) => {
                tracing::warn!(reason = failure.message(), "memory runtime unavailable");
            }
            crate::memory_runtime::MemoryRuntimeStatus::Disabled(_)
            | crate::memory_runtime::MemoryRuntimeStatus::Starting
            | crate::memory_runtime::MemoryRuntimeStatus::Ready(_) => {}
        }
        let capture_memory = project_binding.memory().cloned();
        self.project_binding = project_binding;
        let status = self.with_project_binding(memory_runtime.status_view());
        self.ui_state.set_memory_status(status.clone());
        self.memory_status = status;
        if let Some(source_rx) = self.source_rx.take() {
            self.capture_forwarder = Some(match capture_memory {
                Some(memory) => CaptureForwarder::spawn(source_rx, memory),
                None => CaptureForwarder::discard(source_rx),
            });
        }
        self.memory_runtime = Some(memory_runtime);
    }

    /// Every memory status the App publishes carries the project axis, so
    /// `/memory status` can say "ready, but this workspace is unbound".
    fn with_project_binding(&self, status: MemoryStatusView) -> MemoryStatusView {
        status.with_project(self.project_binding.status_view())
    }

    /// Stop the companion gracefully. Idempotent; `run` calls it on quit and
    /// `main` calls it on every other exit path so an error return never
    /// falls through to the abort-only `Drop`.
    pub(crate) async fn shutdown_memory_runtime(&mut self) {
        if let Some(completion_rx) = self.bridge_completion_rx.take()
            && tokio::time::timeout(Duration::from_secs(2), completion_rx)
                .await
                .is_err()
        {
            tracing::warn!("bridge completion timed out before capture drain");
        }
        if let Some(forwarder) = self.capture_forwarder.take() {
            forwarder.drain().await;
        }
        if let Some(mut memory_runtime) = self.memory_runtime.take() {
            memory_runtime.shutdown().await;
        }
    }

    /// Kick off the initial session. `oneshot_prompt` is the parsed `--prompt`
    /// value (cyril-0ffy): held until the session is ready and then submitted
    /// exactly once via the deferred-command path in `handle_notification`.
    /// `None` (interactive startup) submits nothing. The parameter — rather
    /// than a setter — makes `main()` decide about `cli.prompt` at the call
    /// site; it cannot silently go back to being parsed-but-unread.
    pub async fn create_initial_session(&mut self, cwd: PathBuf, oneshot_prompt: Option<String>) {
        self.startup_prompt = match oneshot_prompt {
            Some(text) if text.is_empty() => {
                // Mirror `submit_input`'s empty-input early return: an empty
                // prompt would be a pointless turn, but dropping user intent
                // silently is worse than saying so.
                tracing::warn!("--prompt is empty; nothing will be submitted");
                None
            }
            other => other,
        };
        self.ui_state
            .add_system_message("Connecting to agent...".into());

        // Load file completer for @-file autocomplete
        let completer = cyril_ui::file_completer::FileCompleter::load(&cwd).await;
        self.ui_state.set_file_completer(completer);

        if let Err(e) = self
            .bridge_sender
            .send(BridgeCommand::NewSession { cwd })
            .await
        {
            self.ui_state
                .add_system_message(format!("Failed to create session: {e}"));
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> cyril_core::Result<()> {
        let mut event_stream = EventStream::new();
        let mut redraw_interval = tokio::time::interval(Self::redraw_duration(Activity::Idle));
        redraw_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Initial draw
        terminal
            .draw(|frame| cyril_ui::render::draw(frame, &self.ui_state))
            .map_err(|e| {
                cyril_core::Error::with_source(
                    cyril_core::ErrorKind::Transport {
                        detail: "initial draw failed".into(),
                    },
                    e,
                )
            })?;

        loop {
            tokio::select! {
                biased;

                // Priority 1: Terminal input
                Some(event) = event_stream.next() => {
                    match event {
                        Ok(event) => {
                            self.handle_terminal_event_batch(event, || {
                                match event_stream.next().now_or_never().flatten() {
                                    Some(Ok(event)) => Some(event),
                                    _ => None,
                                }
                            })
                            .await?;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "terminal event error");
                        }
                    }
                }

                // Priority 2: Notifications from bridge
                Some(notification) = self.notification_rx.recv() => {
                    for deferred in self.handle_notification(notification) {
                        self.dispatch_deferred_command(deferred).await;
                    }
                }
                Some(result) = self.usage_enrichment_rx.recv() => {
                    self.handle_usage_enrichment(result);
                }
                // Snapshots computed off this loop (cyril-nanu). The App holds
                // the handle, so this closes only when the worker thread dies.
                Some(result) = self.usage_snapshot_rx.recv() => {
                    self.handle_usage_snapshot(result);
                }
                // Memory work that ran off-loop (`/memory` commands, first-prompt
                // lesson lookups). The App holds the sender, so this never closes.
                Some(result) = self.memory_task_rx.recv() => {
                    self.handle_memory_task_result(result).await;
                }


                // Priority 3: Permission requests from bridge
                Some(request) = self.permission_rx.recv() => {
                    self.ui_state.show_approval(request);
                    self.redraw_needed = true;
                }

                // Priority 4: Voice engine events (CN2). Resolves to `pending`
                // (never fires) when the voice feature is off — `voice` is None.
                voice_event = Self::next_voice_event(&mut self.voice) => {
                    match voice_event {
                        Some(ev) => self.handle_voice_event(ev),
                        // Channel closed: stop polling instead of busy-looping.
                        None => self.voice = None,
                    }
                }

                memory_status = Self::next_memory_status(&mut self.memory_runtime) => {
                    match memory_status {
                        Some(status) => {
                            let status = self.with_project_binding(status);
                            self.memory_status = status.clone();
                            if self.ui_state.set_memory_status(status) {
                                self.redraw_needed = true;
                            }
                        }
                        None => self.memory_runtime = None,
                    }
                }

                // Priority 5: Redraw tick
                _ = redraw_interval.tick() => {
                    // Flush stream buffer on tick
                    if self.ui_state.flush_stream_buffer() {
                        self.redraw_needed = true;
                    }

                    // During busy states, redraw every tick so the activity
                    // spinner animates and the elapsed timer increments.
                    if !matches!(
                        self.ui_state.activity(),
                        Activity::Idle | Activity::Ready
                    ) {
                        self.redraw_needed = true;
                    }

                    // Deep idle detection
                    if self.last_activity.elapsed() > Duration::from_secs(30) {
                        self.ui_state.set_deep_idle(true);
                    }
                }
            }

            // Adaptive frame rate — subagent, workflow, and voice activity
            // all count beside the main session (the voice meter animates
            // while listening; a workflow step streams as a peer session).
            let new_duration = Self::redraw_duration(self.effective_activity());
            redraw_interval = tokio::time::interval(new_duration);
            redraw_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            // Conditional redraw
            if self.redraw_needed {
                terminal
                    .draw(|frame| cyril_ui::render::draw(frame, &self.ui_state))
                    .map_err(|e| {
                        cyril_core::Error::with_source(
                            cyril_core::ErrorKind::Transport {
                                detail: "draw failed".into(),
                            },
                            e,
                        )
                    })?;
                self.redraw_needed = false;
            }

            if self.ui_state.should_quit() {
                if let Err(e) = self.bridge_sender.send(BridgeCommand::Shutdown).await {
                    tracing::warn!(error = %e, "failed to send shutdown to bridge");
                }
                self.shutdown_memory_runtime().await;
                break;
            }
        }

        Ok(())
    }

    fn redraw_duration(activity: Activity) -> Duration {
        match activity {
            Activity::Streaming | Activity::ToolRunning => Duration::from_millis(50),
            Activity::Waiting | Activity::Sending => Duration::from_millis(100),
            Activity::Ready => Duration::from_millis(250),
            Activity::Idle => Duration::from_secs(1),
        }
    }

    #[cfg(test)]
    fn record_workflow_apply(&mut self) {
        self.workflow_apply_calls += 1;
    }

    #[cfg(not(test))]
    fn record_workflow_apply(&mut self) {}

    #[cfg(test)]
    fn record_subagent_ui_apply(&mut self) {
        self.subagent_ui_apply_calls += 1;
    }

    #[cfg(not(test))]
    fn record_subagent_ui_apply(&mut self) {}

    #[cfg(test)]
    fn record_workflow_stream_apply(&mut self) {
        self.workflow_stream_apply_calls += 1;
    }

    #[cfg(not(test))]
    fn record_workflow_stream_apply(&mut self) {}

    /// Late-claim sweep (cyril-jxfu C5): after workflow state changes, any
    /// optimistic subagent stream whose session a claim now names moves to
    /// the workflow store, history intact. Sweeping on state-change rather
    /// than per event kind covers every claim carrier uniformly — double-emit
    /// re-emission, resume-path first emission, and snapshot-borne node
    /// state. Main's own id can never appear among subagent stream keys (the
    /// classifier never routes main frames there), so no main-guard is
    /// needed.
    fn reparent_claimed_subagent_streams(&mut self) {
        let claimed: Vec<SessionId> = self
            .ui_state
            .subagent_ui()
            .streams()
            .keys()
            .filter(|sid| self.workflow_tracker.session_owner(sid).is_some())
            .cloned()
            .collect();
        for sid in claimed {
            if self.ui_state.claim_stream_for_workflow(&sid) {
                tracing::debug!(
                    session_id = sid.as_str(),
                    "late claim: re-parented optimistic subagent stream to the workflow store"
                );
                self.redraw_needed = true;
            }
        }
    }

    /// Effective activity for the adaptive frame rate: subagent, workflow,
    /// and voice activity all hold the fast tick alongside the main session
    /// (cyril-jxfu C7 — a streaming workflow step must animate even when the
    /// main session is idle, e.g. after attaching to a foreign run).
    fn effective_activity(&self) -> Activity {
        if self.ui_state.any_subagent_active()
            || self.ui_state.any_workflow_active()
            || self.ui_state.any_voice_active()
        {
            Activity::Streaming
        } else {
            self.ui_state.activity()
        }
    }

    #[cfg(test)]
    fn record_session_apply(&mut self) {
        self.session_apply_calls += 1;
    }

    #[cfg(not(test))]
    fn record_session_apply(&mut self) {}

    #[cfg(test)]
    fn record_ui_apply(&mut self) {
        self.ui_apply_calls += 1;
    }

    #[cfg(not(test))]
    fn record_ui_apply(&mut self) {}

    /// Asks the worker for a fresh snapshot. Never computes one here: at
    /// 100,000 rows that is ~700 ms of event loop, and this path fires per
    /// turn, per context sample and per sidecar enrichment (cyril-nanu C1).
    fn refresh_usage_panel_from_log(&mut self) {
        if !self.ui_state.has_usage_panel() {
            return;
        }
        self.request_usage_snapshot();
    }

    /// Sends a snapshot request and reflects the outcome in the panel.
    ///
    /// A failed send means the worker is gone; the panel says so rather than
    /// waiting for a result that can never arrive (cyril-nanu C9).
    fn request_usage_snapshot(&mut self) {
        if self.usage_snapshot.request() {
            self.ui_state.mark_usage_panel_refreshing();
        } else {
            self.ui_state
                .mark_usage_panel_failed("usage snapshot worker is unavailable".to_owned());
        }
        self.redraw_needed = true;
    }

    /// Applies a snapshot that finished off the event loop.
    ///
    /// Re-checks that a panel is open: the guard in
    /// `refresh_usage_panel_from_log` ran when the request was *sent*, and the
    /// operator may have closed the panel since (cyril-nanu C5). A result that
    /// lands with no panel open is discarded and starts nothing.
    fn handle_usage_snapshot(&mut self, result: UsageSnapshotResult) {
        if !self.ui_state.has_usage_panel() {
            return;
        }
        match result {
            UsageSnapshotResult::Ready(snapshot) => {
                self.ui_state.refresh_usage_panel(*snapshot);
            }
            UsageSnapshotResult::Failed(reason) => {
                tracing::warn!(error = %reason, "usage panel refresh failed");
                self.ui_state.mark_usage_panel_failed(reason);
            }
        }
        self.redraw_needed = true;
    }

    fn handle_usage_enrichment(&mut self, result: UsageEnrichmentResult) {
        match result {
            UsageEnrichmentResult::Enriched(enrichment) => {
                self.enrichment_requests.remove(&enrichment.record_id);
                self.failed_enrichments.remove(&enrichment.record_id);
                self.enrichment_attempts.remove(&enrichment.record_id);
                if let Err(error) = self.usage_log.enrich_record(
                    enrichment.record_id,
                    enrichment.billed_model_id.as_deref(),
                    &enrichment.tools,
                ) {
                    tracing::warn!(
                        record_id = enrichment.record_id.get(),
                        error = %error,
                        "usage sidecar enrichment could not update the durable record"
                    );
                } else {
                    self.refresh_usage_panel_from_log();
                }
            }
            UsageEnrichmentResult::Failed {
                record_id,
                message,
                retryable,
            } => {
                let attempts = self.enrichment_attempts.entry(record_id).or_default();
                if reserve_enrichment_retry(retryable, attempts) {
                    self.failed_enrichments.insert(record_id);
                } else {
                    if let Some((session_id, kind)) = self.enrichment_requests.remove(&record_id) {
                        self.usage_enrichment.abandon(record_id, session_id, kind);
                    }
                    self.failed_enrichments.remove(&record_id);
                    self.enrichment_attempts.remove(&record_id);
                }
                tracing::warn!(
                    record_id = record_id.get(),
                    error = message,
                    retryable,
                    "usage sidecar enrichment unavailable; retaining live tool data"
                );
            }
        }
    }

    fn handle_notification(&mut self, routed: RoutedNotification) -> Vec<BridgeCommand> {
        let usage_only = matches!(
            &routed.notification,
            Notification::UsageSessionStarted { .. } | Notification::TurnUsageCaptured(_)
        );
        if let Notification::UsageSessionStarted { session_id, origin } = &routed.notification {
            let kind = match self.agent_engine {
                AgentEngine::V2 => cyril_core::usage::KiroSidecarKind::V2,
                AgentEngine::Kas => cyril_core::usage::KiroSidecarKind::Kas,
            };
            self.usage_enrichment
                .session_started(session_id.clone(), kind, *origin);
        }
        if let Some(write) = self.usage_observer.apply(&routed, Instant::now()) {
            match write {
                UsageWrite::Turn {
                    record,
                    sidecar_kind,
                } => match self.usage_log.append(&record) {
                    Ok(record_id) => {
                        self.refresh_usage_panel_from_log();
                        if let Some(kind) = sidecar_kind {
                            self.enrichment_requests
                                .insert(record_id, (record.context().session_id().clone(), kind));
                            self.enrichment_attempts.insert(record_id, 0);
                            self.usage_enrichment.enrich(
                                record_id,
                                record.context().session_id().clone(),
                                kind,
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(error = %error, "persist usage turn failed");
                        self.ui_state
                            .add_system_message(format!("Usage recording failed: {error}"));
                    }
                },
                UsageWrite::Context { sample, compaction } => {
                    if let Err(error) = self.usage_log.record_context(&sample, compaction.as_ref())
                    {
                        tracing::error!(error = %error, "persist usage context failed");
                        self.ui_state
                            .add_system_message(format!("Usage recording failed: {error}"));
                    } else {
                        self.refresh_usage_panel_from_log();
                    }
                }
            }
        }
        let RoutedNotification {
            session_id,
            notification,
            // cyril-a71q: the bridge has already mediated ownership by the time a
            // notification reaches the App -- a stale or absorbed completion never
            // gets forwarded. The App therefore has no ownership decision left to
            // make and deliberately ignores the stamp. Bound explicitly rather
            // than `..` so a rename breaks loudly here.
            turn: _,
        } = routed;

        if usage_only {
            return Vec::new();
        }

        // Workflow lifecycle frames (cyril-6beh C12/C14): workspace-global
        // state owned by this App's tracker. Branch before EVERY existing
        // SessionController/UiState consumer, consume the boxed event exactly
        // once by value, and never forward the frame onward. A state error
        // leaves the tracker atomic (see WorkflowStateError) and is
        // warning-only: dispatch continues and later frames still apply.
        if let Notification::Workflow(event) = notification {
            self.record_workflow_apply();
            // Capture the frame's diagnostic identity before the tracker
            // consumes the event by value.
            let event_kind = event.method_name();
            let workflow_id = event.workflow_id().as_str().to_owned();
            match self.workflow_tracker.apply_event(*event) {
                // A state change may carry a session claim (node_start
                // re-emit, resume first-emit, or snapshot-borne node state),
                // so the late-claim sweep runs on every applied change
                // (cyril-jxfu C5). Tracker state itself still renders
                // nothing — redraw wiring for the run view is cyril-zd8u.
                Ok(changed) => {
                    if changed {
                        self.reparent_claimed_subagent_streams();
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        workflow_id = %workflow_id,
                        event_kind,
                        error_kind = error.error_kind(),
                        error = %error,
                        "workflow state application failed",
                    );
                }
            }
            return Vec::new();
        }

        // Fetched run snapshots (cyril-0qe6 C4/C5): same exactly-once
        // ownership as lifecycle frames — the tracker consumes the boxed
        // snapshot by value and the notification is never forwarded. The
        // companion WorkflowCommand display outcome arrives separately and
        // rides normal routing. A rejected snapshot (terminal-conflict, bad
        // node paths) leaves the tracker unchanged and is warning-only.
        if let Notification::WorkflowSnapshot(snapshot) = notification {
            let workflow_id = snapshot.workflow_id().as_str().to_owned();
            match self.workflow_tracker.apply_snapshot(*snapshot) {
                Ok(changed) => {
                    if changed {
                        // Snapshot-borne node state can carry session claims
                        // (cyril-jxfu C5) — same sweep as lifecycle frames.
                        self.reparent_claimed_subagent_streams();
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        workflow_id = %workflow_id,
                        error_kind = error.error_kind(),
                        error = %error,
                        "workflow snapshot application failed",
                    );
                }
            }
            return Vec::new();
        }

        // Tracker-level notifications (list_update, inbox) are global:
        // apply them regardless of session_id. Returns false for unrelated variants.
        let tracker_changed = self
            .ui_state
            .apply_subagent_tracker_notification(&notification);

        // SubagentListUpdated also informs SubagentUiState so it can mark terminated streams.
        if let Notification::SubagentListUpdated { ref subagents, .. } = notification {
            self.ui_state.apply_subagent_list_update(subagents);
            self.redraw_needed = true;
        }

        // Route session-scoped notifications. `classify_notification_route` is
        // total over its four inputs, so this match has no routing decision of
        // its own to make — cyril-tglp was exactly such a decision (an extra
        // `&& self.session.id().is_some()`) leaking back into the caller and
        // re-admitting an unattributable frame to the main pipeline.
        if let Some(ref sid) = session_id {
            let tracked = self.ui_state.subagent_tracker().is_subagent(sid);
            let workflow_owned = self.workflow_tracker.session_owner(sid).is_some();
            match classify_notification_route(Some(sid), self.session.id(), tracked, workflow_owned)
            {
                NotificationRoute::Workflow => {
                    self.record_workflow_stream_apply();
                    self.ui_state
                        .apply_workflow_notification(sid, &notification);
                    self.redraw_needed = true;
                    return Vec::new();
                }
                NotificationRoute::Subagent => {
                    if !tracked {
                        // Not main, but no SubagentListUpdated has named it yet.
                        // Route optimistically — the stream is created on first
                        // contact and the list_update reconciles it later.
                        tracing::debug!(
                            session_id = sid.as_str(),
                            "notification for unknown session, routing to subagent stream"
                        );
                    }
                    self.record_subagent_ui_apply();
                    self.ui_state
                        .apply_subagent_notification(sid, &notification);
                    self.redraw_needed = true;
                    return Vec::new();
                }
                NotificationRoute::Drop => {
                    // `warn!`, not `debug!`: no shipped engine produces this
                    // ordering, so a line here means the wire contract moved.
                    tracing::warn!(
                        session_id = sid.as_str(),
                        "scoped notification arrived before any main session exists; \
                         unattributable, dropping (cyril-tglp)"
                    );
                    return Vec::new();
                }
                NotificationRoute::Main => {}
            }
        }

        self.record_session_apply();
        let session_changed = self.session.apply_notification(&notification);
        self.record_ui_apply();
        let ui_changed = self.ui_state.apply_notification(&notification);

        // Register agent commands when they arrive
        if let Notification::CommandsUpdated {
            commands: ref cmds,
            prompts: ref prompt_list,
        } = notification
        {
            self.commands.register_agent_commands(cmds);
            // Update autocomplete with all command info (name + description)
            let mut info: Vec<(String, Option<String>)> = self
                .commands
                .all_commands()
                .iter()
                .map(|cmd| {
                    let desc = cmd.description();
                    (
                        cmd.name().to_string(),
                        Some(desc.to_string()).filter(|s| !s.is_empty()),
                    )
                })
                .collect();
            for prompt in prompt_list {
                info.push((
                    prompt.name().to_string(),
                    prompt
                        .description()
                        .map(str::to_string)
                        .filter(|s| !s.is_empty()),
                ));
            }
            self.ui_state.set_command_info(info);

            // Optimistic code intelligence detection: if .kiro/settings/lsp.json
            // exists in the working directory, assume code intelligence is active
            // until the first /code response confirms or denies it.
            if std::path::Path::new(".kiro/settings/lsp.json").exists() {
                self.ui_state.set_code_intelligence_active(true);
            }
        }

        // Handle clear command result
        if let Notification::AgentMessage(ref msg) = notification
            && !msg.is_streaming
            && msg.text == "__clear__"
        {
            self.ui_state.clear_messages();
        }

        // KAS pushed a new hook registry (cyril-gk17). `SessionController` has
        // already recorded it for `/hooks enable|disable` name resolution, so
        // the App's only remaining job is the view — and only when the panel is
        // already open. This notification arrives unprompted on any hook-file
        // edit, so it must never pop an overlay open by itself.
        if let Notification::HooksChanged { ref hooks } = notification
            && self.ui_state.refresh_hooks_panel(hooks.clone())
        {
            self.redraw_needed = true;
        }

        // Handle command options received — open picker or show message
        if let Notification::CommandOptionsReceived {
            ref command,
            ref options,
        } = notification
        {
            if options.is_empty() {
                self.ui_state
                    .add_system_message(format!("No {command} options available."));
            } else {
                self.ui_state.show_picker(command.clone(), options.clone());
            }
            self.redraw_needed = true;
        }

        // Handle MCP OAuth request — display URL for the user to copy
        if let Notification::McpOAuthRequest {
            ref server_name,
            ref url,
        } = notification
        {
            self.ui_state.add_system_message(format!(
                "MCP server '{server_name}' requires authentication. Open in browser: {url}"
            ));
            self.redraw_needed = true;
        }

        // Handle command execution response. The `hooks` and `code` commands
        // are special-cased; all other commands fall through to the generic
        // command-output path. See `dispatch_command_executed` for the rules.
        let mut deferred_commands: Vec<BridgeCommand> = Vec::new();

        // A fresh session's first prompt carries the project lessons
        // (cyril-ezgo). Armed here — the one place a session becomes current —
        // and consumed by `send_prompt`. A resumed session counts as fresh
        // for this process: whatever an earlier process injected is not
        // visible here, and lessons may have been taught since.
        if let Notification::SessionCreated { session_id, .. } = &notification {
            self.first_prompt_lessons_pending = Some(session_id.clone());
        }

        // One-shot `--prompt` (cyril-0ffy): the initial session is ready, so
        // submit the startup prompt. Deferred through the same bridge-command
        // path as `/code` follow-ups — the run loop sends it, marks the session
        // Busy on success, and surfaces an advisory on failure — so the event
        // loop never blocks and the agent receives exactly one `session/prompt`
        // (`take()` empties the slot; a later `/new` finds it `None`).
        if matches!(notification, Notification::SessionCreated { .. })
            && self.startup_prompt.is_some()
        {
            match self.session.id().cloned() {
                Some(session_id) => {
                    if let Some(text) = self.startup_prompt.take() {
                        // UX parity with a typed submit (`submit_input`'s idle
                        // path): the transcript shows the prompt and the
                        // toolbar shows Sending.
                        self.ui_state.add_user_message(&text);
                        self.ui_state.set_activity(Activity::Sending);
                        deferred_commands.push(BridgeCommand::SendPrompt {
                            session_id,
                            prompt: PromptEnvelope::original(vec![text]),
                        });
                    }
                }
                None => {
                    // Unreachable in practice: SessionController sets its id
                    // unconditionally when applying SessionCreated. If that
                    // invariant ever breaks, the prompt stays pending (not
                    // taken) and submits on the next SessionCreated — log so
                    // the broken invariant is visible rather than silent.
                    tracing::warn!(
                        "SessionCreated applied but no session id is known; \
                         one-shot --prompt held for the next session"
                    );
                }
            }
        }

        if let Notification::CommandExecuted {
            ref command,
            ref response,
        } = notification
        {
            if command == "code" {
                deferred_commands.extend(dispatch_code_command(
                    response,
                    &self.session,
                    &mut self.ui_state,
                ));
            } else if command == "rewind" {
                // `/rewind` orchestration: when the agent signals
                // `switchSession: true` in the response, fire the
                // load+terminate pair to transition to the new session.
                // No new ACP method needed — the bridge already has
                // LoadSession and TerminateSession primitives.
                deferred_commands.extend(dispatch_rewind_command(
                    response,
                    &self.session,
                    &mut self.ui_state,
                ));
                dispatch_command_executed(command, response, &mut self.ui_state);
            } else {
                dispatch_command_executed(command, response, &mut self.ui_state);

                // WORKAROUND(Kiro v1.28.0): Kiro doesn't send ConfigOptionUpdate for
                // model changes (QRK-004), so we extract the model from the /model
                // command response. When Kiro sends proper ConfigOptionUpdate
                // notifications, this block becomes dead code — remove it and rely
                // on the ConfigOptionsUpdated handler in UiState.apply_notification().
                if command == "model"
                    && let Some(model_id) = response
                        .get("data")
                        .and_then(|d| d.get("model"))
                        .and_then(|m| m.get("id"))
                        .and_then(|id| id.as_str())
                {
                    self.ui_state.set_current_model(Some(model_id.to_string()));
                }
            }

            self.redraw_needed = true;
        }

        self.redraw_needed = self.redraw_needed || session_changed || ui_changed || tracker_changed;
        deferred_commands
    }

    /// Handle one terminal event plus buffered followers. An active approval is
    /// an input-batch boundary: its terminal key can synchronously promote the
    /// next request, which must be drawn before another key can act on it.
    async fn handle_terminal_event_batch(
        &mut self,
        event: Event,
        mut next_buffered: impl FnMut() -> Option<Event>,
    ) -> cyril_core::Result<()> {
        let approval_was_active = self.ui_state.has_approval();
        self.handle_terminal_event(event).await?;
        if approval_was_active {
            return Ok(());
        }
        while let Some(event) = next_buffered() {
            self.handle_terminal_event(event).await?;
        }
        Ok(())
    }

    async fn handle_terminal_event(&mut self, event: Event) -> cyril_core::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Mouse(mouse) => {
                // Respect modal overlay priority — don't scroll chat when
                // an overlay is consuming input.
                if !self.ui_state.has_approval()
                    && !self.ui_state.has_picker()
                    && !self.ui_state.has_hooks_panel()
                    && !self.ui_state.has_code_panel()
                    && !self.ui_state.has_usage_panel()
                    && self.ui_state.subagent_ui().focused_session_id().is_none()
                {
                    // Mouse wheel uses a fixed 3-line step; keyboard
                    // PgUp/PgDn uses half-page for coarser navigation.
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            self.ui_state.chat_scroll_up(MOUSE_SCROLL_LINES);
                            self.redraw_needed = true;
                        }
                        MouseEventKind::ScrollDown => {
                            self.ui_state.chat_scroll_down(MOUSE_SCROLL_LINES);
                            self.redraw_needed = true;
                        }
                        _ => {}
                    }
                }
            }
            Event::Resize(w, h) => {
                self.ui_state.set_terminal_size(w, h);
                self.redraw_needed = true;
            }
            Event::Paste(text) => {
                if !self.ui_state.has_usage_panel() {
                    self.ui_state.insert_text(&text);
                    self.redraw_needed = true;
                }
            }
            _ => {}
        }
        self.last_activity = Instant::now();
        self.ui_state.set_deep_idle(false);
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        // Layer 1: Global shortcuts
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c'))
            | (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                self.ui_state.request_quit();
                return Ok(());
            }
            (KeyModifiers::CONTROL, KeyCode::Char('m')) => {
                self.ui_state.toggle_mouse_capture();
                let result = if self.ui_state.mouse_captured() {
                    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture,)
                } else {
                    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture,)
                };
                if let Err(e) = result {
                    tracing::warn!(error = %e, "failed to toggle mouse capture");
                    self.ui_state.toggle_mouse_capture(); // roll back
                }
                self.redraw_needed = true;
                return Ok(());
            }
            _ => {}
        }

        // Layer 2: Modal overlays
        if self.ui_state.has_approval() {
            self.handle_approval_key(key);
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_picker() {
            self.handle_picker_key(key).await?;
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_hooks_panel() {
            self.handle_hooks_panel_key(key);
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_code_panel() {
            self.handle_code_panel_key(key).await?;
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_usage_panel() {
            dispatch_usage_panel_key(key, &mut self.ui_state);
            self.redraw_needed = true;
            return Ok(());
        }

        // Layer 3: Autocomplete (if active — consumes relevant keys)
        match self.ui_state.handle_autocomplete_key(key) {
            AutocompleteAction::Consumed | AutocompleteAction::Accepted => {
                self.redraw_needed = true;
                return Ok(());
            }
            AutocompleteAction::AcceptedAndSubmit => {
                self.submit_input().await?;
                self.redraw_needed = true;
                return Ok(());
            }
            AutocompleteAction::NotActive => {} // Fall through to Layer 4
        }

        // Layer 4: Normal input
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input().await?;
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // If drilled into a subagent stream, Esc exits the drill-in first.
                if self.ui_state.subagent_ui().focused_session_id().is_some() {
                    self.ui_state.unfocus_subagent();
                } else if matches!(self.session.status(), SessionStatus::Busy) {
                    self.bridge_sender
                        .send(BridgeCommand::CancelRequest)
                        .await?;
                    // Stalled-turn escalation (cyril-14ou): if the stall chip
                    // is up, record that this cancel went out — the engine may
                    // not honor a cancel mid-stall (cyril-w9oi is the second
                    // tier), and the chip wording should say so. No-op when
                    // no stall is displayed.
                    self.ui_state.mark_stall_cancel_sent();
                }
            }
            _ => {
                // Only scroll the main chat when not drilled into a subagent.
                let scroll_consumed = self.ui_state.subagent_ui().focused_session_id().is_none()
                    && dispatch_chat_scroll_key(key, &mut self.ui_state);
                if !scroll_consumed {
                    self.ui_state.handle_input_key(key);
                }
            }
        }

        self.redraw_needed = true;
        Ok(())
    }

    fn handle_approval_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.ui_state.approval_select_prev(),
            KeyCode::Down => self.ui_state.approval_select_next(),
            KeyCode::Enter => {
                // A confirmed trust tier carries its approval origin. Only a
                // valid main-session id may write the active agent's durable
                // config; an empty wire id is retained for provenance but is
                // never authority.
                if let Some((origin, trust)) = self.ui_state.approval_confirm() {
                    if !origin.as_str().is_empty() && self.session.id() == Some(&origin) {
                        self.persist_trust_grant(&trust);
                    } else {
                        let origin = approval_origin_label(&origin);
                        self.ui_state.add_system_message(format!(
                            "Trust remains session-scoped for {origin}; it was not saved to the \
                             main agent's config."
                        ));
                    }
                }
            }
            KeyCode::Esc => self.ui_state.approval_cancel(),
            _ => {}
        }
    }

    /// Persist a granted trust tier to the active agent's config file so it
    /// survives across sessions. The session-scoped grant has already been sent;
    /// this write is non-fatal. Built-in agents and agents with no on-disk config
    /// are intentionally skipped (logged at debug); a genuine write/parse failure
    /// is surfaced to the user, since they explicitly asked to "always allow".
    fn persist_trust_grant(&mut self, trust: &cyril_core::types::TrustOption) {
        use cyril_core::kiro_agent_config::{TrustPersistError, persist_trust_grant};

        // Own the agent name so the immutable session borrow ends before we may
        // need `&mut self.ui_state` to report a failure below.
        let Some(agent) = self
            .session
            .current_mode_id()
            .map(|m| m.as_str().to_string())
        else {
            tracing::debug!("no active agent identity; trust grant not persisted");
            return;
        };
        match persist_trust_grant(&agent, &self.cwd, &trust.setting_key, &trust.patterns) {
            Ok(path) => {
                tracing::info!(path = %path.display(), "persisted trust grant across sessions")
            }
            Err(e @ (TrustPersistError::BuiltinAgent(_) | TrustPersistError::NoConfig(_))) => {
                // Expected for the default/built-in agents and ad-hoc agents
                // without a config file — session-scoped trust still applies.
                tracing::debug!(reason = %e, "trust grant not persisted");
            }
            Err(e) => {
                // A genuine persistence failure (write/parse/serialize/invalid
                // config). Don't let it vanish into the log — the user must learn
                // the grant won't survive the session.
                tracing::warn!(error = %e, "failed to persist trust grant");
                self.ui_state.add_system_message(format!(
                    "Trust applied for this session, but saving it across sessions failed: {e}"
                ));
            }
        }
    }

    /// Handle key input while the `/hooks` panel overlay is visible.
    /// Esc closes; Up/Down and PgUp/PgDn scroll.
    fn handle_hooks_panel_key(&mut self, key: KeyEvent) {
        dispatch_hooks_panel_key(key, &mut self.ui_state);
    }

    /// Handle key input while the `/code` panel overlay is visible.
    /// Esc closes; `r` refreshes by re-executing the `/code` command.
    async fn handle_code_panel_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        match key.code {
            KeyCode::Esc => self.ui_state.close_code_panel(),
            KeyCode::Char('r') => {
                if let Some(id) = self.session.id().cloned() {
                    self.bridge_sender
                        .send(BridgeCommand::ExecuteCommand {
                            command: "code".into(),
                            session_id: id,
                            args: serde_json::json!({}),
                        })
                        .await?;
                } else {
                    tracing::debug!("code panel refresh requested but no active session");
                    self.ui_state
                        .add_system_message("No active session — cannot refresh.".into());
                    self.ui_state.close_code_panel();
                }
            }
            _ => {} // Consume all other keys
        }
        Ok(())
    }

    async fn handle_picker_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        match key.code {
            KeyCode::Up => self.ui_state.picker_select_prev(),
            KeyCode::Down => self.ui_state.picker_select_next(),
            KeyCode::Enter => {
                if let Some((command_name, value)) = self.ui_state.picker_confirm()
                    && let Some(session_id) = self.session.id()
                {
                    self.bridge_sender
                        .send(BridgeCommand::ExecuteCommand {
                            command: command_name,
                            session_id: session_id.clone(),
                            args: serde_json::json!({"value": value}),
                        })
                        .await?;
                }
            }
            KeyCode::Esc => self.ui_state.picker_cancel(),
            KeyCode::Char(c) => self.ui_state.picker_type_char(c),
            KeyCode::Backspace => self.ui_state.picker_backspace(),
            _ => {}
        }
        Ok(())
    }

    /// Start a `/memory` lesson command. Returns immediately: the companion
    /// round trip runs on a spawned task and its rendered output arrives via
    /// `memory_task_rx` (the event loop must never block on a command).
    fn handle_memory_action(&mut self, action: MemoryCommandAction) {
        let Some(memory) = self.project_binding.memory().cloned() else {
            let message = self
                .project_binding
                .unavailable_message()
                .unwrap_or_else(|| "Memory is unavailable for this project.".to_owned());
            self.ui_state.add_system_message(message);
            return;
        };
        let results = self.memory_task_tx.clone();
        tokio::spawn(async move {
            let rendered = run_memory_action(memory, action).await;
            if results
                .send(MemoryTaskResult::CommandOutput(rendered))
                .is_err()
            {
                tracing::debug!("memory command output dropped: app is gone");
            }
        });
    }

    async fn submit_input(&mut self) -> cyril_core::Result<()> {
        let text = self.ui_state.take_input();
        if text.is_empty() {
            return Ok(());
        }

        self.last_activity = Instant::now();

        // Try as slash command
        if let Some((cmd, args)) = self.commands.parse(&text) {
            let ctx = CommandContext {
                workspace: &self.cwd,
                session: &self.session,
                bridge: &self.bridge_sender,
                subagent_tracker: Some(self.ui_state.subagent_tracker()),
                workflow_tracker: Some(&self.workflow_tracker),
                memory_status: Some(&self.memory_status),
            };
            let command_name = cmd.name().to_string();
            let args = args.to_string();
            match cmd.execute(&ctx, &args).await {
                // /steer needs the async steer path (echo + SteerSession); route it
                // through the same dispatch_steer as Enter-while-busy.
                Ok(CommandResult {
                    kind: CommandResultKind::Steer { text },
                }) => {
                    return dispatch_steer(
                        &mut self.ui_state,
                        &self.session,
                        &self.bridge_sender,
                        text,
                    )
                    .await;
                }
                // /steer clear needs the async bridge path too (cyril-vgcm C11).
                Ok(CommandResult {
                    kind: CommandResultKind::ClearSteer,
                }) => {
                    return dispatch_clear_steer(
                        &mut self.ui_state,
                        &self.session,
                        &self.bridge_sender,
                    )
                    .await;
                }
                Ok(CommandResult {
                    kind: CommandResultKind::MemoryAction(action),
                }) => {
                    self.handle_memory_action(action);
                    return Ok(());
                }
                Ok(result) => self.handle_command_result(result),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        command = %command_name,
                        "slash command execution failed"
                    );
                    self.ui_state
                        .add_system_message(format!("Command error: {e}"));
                }
            }
            return Ok(());
        }

        // Route by session state (K1b, cyril-bm1j): a busy turn steers instead of
        // firing a second SendPrompt the bridge would reject — the cyril-2vcc fix.
        // Prompt/NoSession fall through to the existing block (which handles the
        // no-session advisory itself).
        if classify_submit(self.session.status(), self.session.id().is_some()) == SubmitRoute::Steer
        {
            return dispatch_steer(&mut self.ui_state, &self.session, &self.bridge_sender, text)
                .await;
        }

        // Send as prompt (idle path, unchanged)
        let session_id = match self.session.id() {
            Some(id) => id.clone(),
            None => {
                self.ui_state
                    .add_system_message("No active session. Use /new to create one.".into());
                return Ok(());
            }
        };

        self.ui_state.add_user_message(&text);
        self.session.set_status(SessionStatus::Busy);
        self.ui_state.set_activity(Activity::Sending);

        let mut content_blocks = vec![text.clone()];

        if let Some(completer) = self.ui_state.file_completer() {
            let root = completer.root().to_path_buf();
            let known = completer.known_files();
            for path in cyril_ui::file_completer::parse_file_references(&text, known) {
                match cyril_ui::file_completer::read_file(&root, &path) {
                    Ok(contents) => {
                        content_blocks.push(format!("<file path=\"{path}\">\n{contents}\n</file>"));
                        tracing::info!("Attached @-referenced file: {path}");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read @-referenced file {path}: {e}");
                        self.ui_state
                            .add_system_message(format!("Could not attach @{path}: {e}"));
                    }
                }
            }
        }

        self.send_prompt(session_id, content_blocks).await
    }

    fn handle_command_result(&mut self, result: CommandResult) {
        match result.kind {
            CommandResultKind::SystemMessage(text) => {
                if text == "__clear__" {
                    self.ui_state.clear_messages();
                } else {
                    self.ui_state.add_system_message(text);
                }
            }
            CommandResultKind::NotACommand(_text) => {
                // Should not happen since we already checked parse()
            }
            CommandResultKind::ShowPicker { title, options } => {
                self.ui_state.show_picker(title, options);
            }
            CommandResultKind::Dispatched => {
                // Already sent via bridge
            }
            CommandResultKind::Steer { .. } => {
                // Routed in submit_input before reaching here (needs async
                // dispatch_steer). Reaching this arm is a routing bug.
                tracing::error!("Steer result reached handle_command_result — routing bug");
            }
            CommandResultKind::ClearSteer => {
                // Routed in submit_input before reaching here (needs async
                // dispatch_clear_steer) — same split as Steer above.
                tracing::error!("ClearSteer result reached handle_command_result — routing bug");
            }
            CommandResultKind::ToggleVoice => {
                self.toggle_voice();
            }
            CommandResultKind::MemoryStatus(status) => {
                let rendered = cyril_ui::memory_format::format_memory_status(&status);
                self.ui_state
                    .add_command_output("memory".to_owned(), rendered);
            }
            CommandResultKind::MemoryAction(_) => {
                tracing::error!("MemoryAction reached synchronous result routing");
            }
            CommandResultKind::ShowUsage {
                account_query_started,
            } => {
                // The panel opens on this frame and fills in when the snapshot
                // lands (cyril-nanu D2). Computing it here cost ~700 ms of
                // frozen terminal at 100,000 rows.
                self.ui_state.open_usage_panel();
                self.request_usage_snapshot();
                if account_query_started {
                    self.ui_state.mark_usage_account_loading();
                    if let Err(error) = self
                        .bridge_sender
                        .try_send(BridgeCommand::QueryUsageAccount)
                    {
                        self.ui_state
                            .apply_notification(&Notification::UsageAccountQueryFailed {
                                message: error.to_string(),
                            });
                    }
                }
                let retry_ids = std::mem::take(&mut self.failed_enrichments);
                for record_id in retry_ids {
                    if let Some((session_id, kind)) =
                        self.enrichment_requests.get(&record_id).cloned()
                    {
                        self.usage_enrichment.enrich(record_id, session_id, kind);
                    }
                }
            }
            CommandResultKind::Quit => {
                self.ui_state.request_quit();
            }
        }
        self.redraw_needed = true;
    }

    /// Await the next event from the voice engine, or never resolve when voice
    /// is disabled (the handle is `None`). Lets the `select!` arm stay cfg-free.
    async fn next_voice_event(
        voice: &mut Option<cyril_core::voice::VoiceHandle>,
    ) -> Option<VoiceEvent> {
        match voice {
            Some(handle) => handle.recv_event().await,
            None => std::future::pending().await,
        }
    }
    async fn next_memory_status(
        memory_runtime: &mut Option<MemoryRuntimeHandle>,
    ) -> Option<MemoryStatusView> {
        match memory_runtime {
            Some(runtime) => runtime.changed().await,
            None => std::future::pending().await,
        }
    }

    /// Apply a voice engine event to UI state (ROADMAP CN2).
    fn handle_voice_event(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Level(level) => self.ui_state.set_voice_level(level),
            VoiceEvent::Status(status) => self.ui_state.set_voice_status(status),
            // The payoff: a finished transcript drops into the input buffer.
            VoiceEvent::Transcript(text) => self.ui_state.insert_text(&text),
            VoiceEvent::Error(msg) => {
                // The engine bailed → it is no longer capturing. Clear intent so
                // the next /voice starts fresh.
                self.voice_active = false;
                self.ui_state.set_voice_status(VoiceStatus::Idle);
                self.ui_state
                    .add_system_message(format!("Voice error: {msg}"));
            }
        }
        // Note (V1b): when the engine gains engine-initiated stops (silence
        // timeout), do NOT naively reconcile `voice_active` from a `Status(Idle)`
        // event — a stale Idle from a completed Stop can arrive after a newer
        // Start and wedge the toggle. Tag commands/events with a generation, or
        // emit a distinct auto-stopped event, and reconcile on that.
        self.redraw_needed = true;
    }

    /// Toggle voice capture (the `/voice` command). Decides Start vs Stop from
    /// the authoritative `voice_active` intent (not the lagging UI projection),
    /// and reports gracefully if voice isn't compiled in or is backpressured.
    /// `redraw_needed` is set by the caller (`handle_command_result`).
    fn toggle_voice(&mut self) {
        let Some(handle) = self.voice.as_ref() else {
            // `voice` is None for two distinct reasons: the feature was never
            // compiled in, or the engine thread exited at runtime (the select!
            // arm cleared the handle). Report them differently — `cfg!` keeps
            // this a single code path with no `#[cfg]` block.
            let detail = if cfg!(feature = "voice") {
                "Voice engine is unavailable — it stopped unexpectedly."
            } else {
                "Voice input isn't compiled in — rebuild with `--features voice`."
            };
            self.ui_state.add_system_message(detail.into());
            return;
        };
        let cmd = if self.voice_active {
            VoiceCommand::Stop
        } else {
            VoiceCommand::Start
        };
        match handle.try_send_command(cmd) {
            // Flip intent only on a successful send so it never drifts from
            // what the engine was actually told.
            Ok(()) => self.voice_active = !self.voice_active,
            Err(e) => {
                tracing::warn!(error = %e, "failed to send voice command");
                let detail = match e {
                    cyril_core::voice::VoiceError::Busy => "Voice engine busy — try again.",
                    cyril_core::voice::VoiceError::ChannelClosed => "Voice subsystem unavailable.",
                };
                self.ui_state.add_system_message(detail.into());
            }
        }
    }
}

/// Where a session-scoped notification belongs (cyril-a71q C7, cyril-tglp).
///
/// Extracted as a pure function so the routing rule is testable without building
/// an `App` — the same shape as `classify_submit`. The rule decides whether a
/// notification may touch MAIN state at all, which is what makes "only the owned
/// release mutates main" checkable rather than asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationRoute {
    /// Global (no scope) or the main session: apply to `SessionController`/`UiState`.
    Main,
    /// A different session: apply to that subagent's stream, main untouched.
    Subagent,
    /// A workflow step's peer session (cyril-jxfu): a `node_start` or
    /// snapshot has claimed this id, so its frames belong to the workflow
    /// stream store — never to `SubagentUiState` or the crew panel, which no
    /// engine ever names workflow steps into.
    Workflow,
    /// Unattributable — discard. Scoped to a session that nothing has yet
    /// identified, while no main session exists to compare it against. See the
    /// drop-vs-buffer rationale on `classify_notification_route`.
    Drop,
}

/// Classify a session-scoped notification. Total over its four inputs so the
/// caller keeps no routing decision of its own: cyril-tglp was precisely such a
/// leftover decision, an `&& self.session.id().is_some()` in `handle_notification`
/// that re-admitted an unattributable frame to the main pipeline while this
/// function had already classified it as not-main.
///
/// `tracked_subagent` means "a `kiro.dev/subagent/list_update` has already named
/// this session id as a subagent". It is load-bearing only when `main` is
/// unknown — once main is known, "scoped and not main" is decidable without it.
///
/// `workflow_owned` means "a workflow claim (`node_start` or snapshot node
/// state) has named this session id" (cyril-jxfu). It outranks everything but
/// main itself: a workflow claim on the MAIN session is a wire anomaly, and
/// protecting main-pipeline continuity wins there (cyril-a71q C7); it beats
/// `tracked_subagent` because ownership is a positive per-id claim while no
/// shipped engine ever lists a workflow step in a `list_update`; and it makes
/// a pre-main frame attributable, so the Drop arm never fires for it.
fn classify_notification_route(
    scope: Option<&SessionId>,
    main: Option<&SessionId>,
    tracked_subagent: bool,
    workflow_owned: bool,
) -> NotificationRoute {
    match (scope, main) {
        // Unscoped -> global lifecycle event, nothing to compare against.
        (None, _) => NotificationRoute::Main,
        // Scoped and it IS the main session.
        (Some(s), Some(m)) if s == m => NotificationRoute::Main,
        // Scoped, not main, and a workflow claim names it -> the workflow
        // stream store, regardless of trackedness or whether main is known.
        (Some(_), _) if workflow_owned => NotificationRoute::Workflow,
        // Scoped, main is known, and it is NOT main -> foreign. Whether the
        // subagent is already tracked only decides which stream receives it, not
        // whether main is spared; both paths spare main.
        (Some(_), Some(_)) => NotificationRoute::Subagent,
        // Scoped while no main session id is known yet, but a list_update has
        // already proven this id is a subagent. Attributable, so it gets its
        // stream. Collapsing this to Main silently rerouted a tracked subagent's
        // frames into main state when they arrived before SessionCreated.
        (Some(_), None) if tracked_subagent => NotificationRoute::Subagent,
        // Scoped, no main session, and nothing has named the id: genuinely
        // UNATTRIBUTABLE. It may be a subagent whose list_update is still in
        // flight, or the main session's own first frame racing SessionCreated —
        // and the two are indistinguishable from here.
        //
        // DROP, not buffer, and not "guess subagent" (cyril-tglp):
        //  - Guessing subagent keys a stream by an id that may turn out to BE
        //    main, leaving a phantom stream that the crew panel (which reads the
        //    tracker, not the streams) never renders.
        //  - Buffering needs a bounded queue plus a replay trigger on
        //    SessionCreated: new state and new lifecycle logic inside a
        //    deliberately thin orchestrator, to recover frames from an ordering
        //    no shipped engine produces — both scoped producers, `ToolCallChunk`
        //    and `MetadataUpdated`, follow session creation.
        //  - Dropping forfeits at most one currently-unreachable frame and says
        //    so at `warn!`, so an engine that ever does produce this ordering
        //    surfaces as a log line rather than as corrupted main state.
        (Some(_), None) => NotificationRoute::Drop,
    }
}

/// Where a non-empty, non-command Enter submit should go (ROADMAP K1b, cyril-bm1j).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitRoute {
    /// Busy turn in flight → steer it mid-turn instead of starting a 2nd prompt.
    Steer,
    /// Idle/other state → send as a normal prompt (unchanged pre-K1b behavior).
    Prompt,
    /// No active session → advisory; nothing to prompt or steer.
    NoSession,
}

/// Classify a non-empty, non-command Enter submit. Pure decision (the CI-testable
/// seam behind `submit_input`): only `Busy` steers; everything else with a session
/// prompts; no session is advisory. `has_session` is checked first — you cannot
/// steer or prompt a session that does not exist.
///
/// Precondition (sanity-hint, caller-guaranteed): called only for non-empty,
/// non-command text — `submit_input` early-returns on empty and dispatches slash
/// commands before reaching here. The function ignores text content, so a
/// violation still yields a correct route; no runtime enforcement is needed.
fn classify_submit(status: &SessionStatus, has_session: bool) -> SubmitRoute {
    if !has_session {
        SubmitRoute::NoSession
    } else if matches!(status, SessionStatus::Busy) {
        SubmitRoute::Steer
    } else {
        SubmitRoute::Prompt
    }
}

/// Whether a steer can be delivered, or why not (ROADMAP K1b, cyril-bm1j).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SteerGate {
    Send,
    AdvisoryUnsupported,
    AdvisoryNoSession,
}

/// Decide whether a steer should be sent. `has_session` is checked BEFORE
/// `unsupported` — the message a user sees for "no session" must win over
/// "unsupported", and a steer needs a session id regardless. Pure (CI-testable).
fn steer_gate(unsupported: bool, has_session: bool) -> SteerGate {
    if !has_session {
        SteerGate::AdvisoryNoSession
    } else if unsupported {
        SteerGate::AdvisoryUnsupported
    } else {
        SteerGate::Send
    }
}

/// Dispatch a queue-steer: the single path shared by Enter-while-busy and the
/// `/steer` command (ROADMAP K1b, cyril-bm1j). Applies `steer_gate`, emits
/// `SteerSession`, and adds the optimistic echo only once the send succeeds
/// (cyril-7n1l) — or an advisory when it can't.
/// Gating on `steering_unsupported()` is the keystone that keeps the optimistic
/// echo reconcilable: the bridge drops a steer on a known-unsupported session
/// silently (no notification), so we must not add a `Queued` echo that would
/// then never resolve.
///
/// Precondition (sanity-hint, caller-guaranteed): `text` is non-empty —
/// `submit_input` early-returns on empty input and `/steer` returns usage for an
/// empty arg. An empty steer would be a backend no-op, not wrong cyril output, so
/// a `debug_assert!` suffices (no release-time refusal needed).
async fn dispatch_steer(
    ui: &mut UiState,
    session: &SessionController,
    bridge: &BridgeSender,
    text: String,
) -> cyril_core::Result<()> {
    debug_assert!(
        !text.is_empty(),
        "dispatch_steer callers guarantee non-empty text"
    );
    match steer_gate(session.steering_unsupported(), session.id().is_some()) {
        SteerGate::Send => {
            // id() is Some — steer_gate just checked has_session.
            let Some(session_id) = session.id().cloned() else {
                return Ok(());
            };
            // Send BEFORE committing the optimistic echo (cyril-7n1l): a failed
            // send means the steer never reached the backend, so no notification
            // will ever drain the chip/echo — an echo added first would be a
            // permanent phantom.
            bridge
                .send(BridgeCommand::SteerSession {
                    session_id,
                    message: text.clone(),
                })
                .await?;
            ui.add_steer_echo(&text);
        }
        SteerGate::AdvisoryUnsupported => ui.add_system_message(
            "Steering isn't supported by this backend (needs kiro-cli 2.7.0+).".into(),
        ),
        SteerGate::AdvisoryNoSession => {
            ui.add_system_message("No active session — nothing to steer.".into())
        }
    }
    Ok(())
}

/// Dispatch a queue-clear: the `/steer clear` path (cyril-vgcm C11). Reuses
/// `steer_gate` — clear's gating is definitionally identical (a session that
/// can't steer has nothing queued to clear; the pre-send skip in the bridge
/// mirrors this). Emits `ClearSteering` with ZERO optimistic mutation (D4):
/// chips flip only when the `SteeringCleared` broadcast lands — the broadcast
/// is the truth, and a local pre-drain would desync from an id-scoped or
/// failed clear. Advisory system messages only for the no-session /
/// steer-unsupported gates; success is silent (matches steer's echo-driven
/// philosophy). No text precondition — there is no payload.
async fn dispatch_clear_steer(
    ui: &mut UiState,
    session: &SessionController,
    bridge: &BridgeSender,
) -> cyril_core::Result<()> {
    match steer_gate(session.steering_unsupported(), session.id().is_some()) {
        SteerGate::Send => {
            // id() is Some — steer_gate just checked has_session.
            let Some(session_id) = session.id().cloned() else {
                return Ok(());
            };
            bridge
                .send(BridgeCommand::ClearSteering { session_id })
                .await?;
        }
        SteerGate::AdvisoryUnsupported => ui.add_system_message(
            "Steering isn't supported by this backend (needs kiro-cli 2.7.0+).".into(),
        ),
        SteerGate::AdvisoryNoSession => {
            ui.add_system_message("No active session — no queued steers to clear.".into())
        }
    }
    Ok(())
}

/// Produce a concise one-line summary from a (possibly multi-line) tool description.
///
/// Tool descriptions frequently begin with a leading newline and hard-wrap their
/// opening sentence across physical lines (e.g. the `subagent` tool's first line ends
/// mid-sentence at "Each stage runs as a"). Taking the first physical line therefore
/// truncates mid-sentence. Instead, take the first paragraph (up to a blank line),
/// collapse its internal whitespace, and return its first sentence.
fn summarize_description(desc: &str) -> String {
    // First paragraph: everything up to the first blank line.
    let first_para = desc.trim().split("\n\n").next().unwrap_or("").trim();
    // Collapse hard-wrapped newlines and runs of whitespace into single spaces.
    let collapsed = first_para.split_whitespace().collect::<Vec<_>>().join(" ");
    // Prefer the first sentence — the earliest sentence terminator followed by a
    // space — to keep rows short. Fall back to the whole collapsed paragraph when
    // there is no sentence boundary. `..=idx` is byte-safe: every terminator is
    // ASCII, so `idx` lands on a char boundary.
    let boundary = [". ", "? ", "! "]
        .into_iter()
        .filter_map(|term| collapsed.find(term))
        .min();
    match boundary {
        Some(idx) => collapsed[..=idx].to_string(),
        None => collapsed,
    }
}

/// Append per-file context items (indented) under a context-breakdown category.
///
/// Kiro's `/context` response nests an `items` array under categories like
/// `contextFiles`/`sessionFiles`, each item carrying `name`, `tokens`,
/// `percent`, `matched`, and an optional `auto_included` flag. Items are
/// rendered largest-first so the heaviest contributors surface at the top.
/// Categories without an `items` array (e.g. `tools`) are left untouched.
fn append_context_items(out: &mut String, category: &serde_json::Value) {
    let Some(items) = category.get("items").and_then(|i| i.as_array()) else {
        return;
    };

    // Sort by token count descending without mutating the source array.
    let mut sorted: Vec<&serde_json::Value> = items.iter().collect();
    sorted.sort_by(|a, b| {
        let at = a.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let bt = b.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        bt.cmp(&at)
    });

    for item in sorted {
        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let tokens = item.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let pct = item.get("percent").and_then(|p| p.as_f64()).unwrap_or(0.0);
        // Optional flags: surface only when they tell the user something useful.
        let auto = item
            .get("auto_included")
            .and_then(|a| a.as_bool())
            .unwrap_or(false);
        let matched = item
            .get("matched")
            .and_then(|m| m.as_bool())
            .unwrap_or(true);
        let mut tags = String::new();
        if auto {
            tags.push_str(" (auto)");
        }
        if !matched {
            tags.push_str(" (unmatched)");
        }
        out.push_str(&format!("    {name} — {tokens} tokens ({pct:.1}%){tags}\n"));
    }
}

/// Format a `kiro.dev/commands/execute` response for display as a system message.
///
/// The response shape is `{"success": bool, "message": "...", "data": {...}}`.
/// This handles tools lists, context breakdowns, usage breakdowns, and generic messages
/// as a priority cascade.
fn format_command_response(command: &str, response: &serde_json::Value) -> String {
    let message = response
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let data = response.get("data");

    // If there's tool data, format as a list
    if let Some(tools) = data.and_then(|d| d.get("tools")).and_then(|t| t.as_array()) {
        let mut out = format!("{message}\n\n");
        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let source = tool.get("source").and_then(|s| s.as_str()).unwrap_or("");
            let desc = summarize_description(
                tool.get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or(""),
            );
            let source_tag = if !source.is_empty() && source != "built-in" {
                format!(" ({source})")
            } else {
                String::new()
            };
            out.push_str(&format!("  {name} — {desc}{source_tag}\n"));
        }
        return out;
    }

    // If there's a context breakdown, format it
    if let Some(breakdown) = data.and_then(|d| d.get("breakdown")) {
        let pct = data
            .and_then(|d| d.get("contextUsagePercentage"))
            .and_then(|p| p.as_f64())
            .unwrap_or(0.0);
        let model = data
            .and_then(|d| d.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        let mut out = format!("Context: {pct:.1}% used (model: {model})\n\n");
        let categories = [
            ("contextFiles", "Context files"),
            ("tools", "Tools"),
            ("yourPrompts", "Your prompts"),
            ("kiroResponses", "Kiro responses"),
            ("sessionFiles", "Session files"),
        ];
        for (key, label) in &categories {
            if let Some(cat) = breakdown.get(*key) {
                let tokens = cat.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                let cat_pct = cat.get("percent").and_then(|p| p.as_f64()).unwrap_or(0.0);
                if tokens > 0 {
                    out.push_str(&format!("  {label}: {tokens} tokens ({cat_pct:.1}%)\n"));
                    append_context_items(&mut out, cat);
                }
            }
        }
        return out;
    }

    // If there's usage breakdown data
    if let Some(breakdowns) = data
        .and_then(|d| d.get("usageBreakdowns"))
        .and_then(|u| u.as_array())
    {
        let plan = data
            .and_then(|d| d.get("planName"))
            .and_then(|p| p.as_str())
            .unwrap_or("Unknown");
        let mut out = format!("Plan: {plan}\n\n");
        for bd in breakdowns {
            let name = bd
                .get("displayName")
                .and_then(|n| n.as_str())
                .unwrap_or("?");
            let used = bd.get("used").and_then(|u| u.as_f64()).unwrap_or(0.0);
            let limit = bd.get("limit").and_then(|l| l.as_f64()).unwrap_or(0.0);
            let pct = bd.get("percentage").and_then(|p| p.as_u64()).unwrap_or(0);
            out.push_str(&format!("  {name}: {used:.0} / {limit:.0} ({pct}%)\n"));
        }
        return out;
    }

    // For well-formatted messages, just use them
    if !message.is_empty() {
        return message.to_string();
    }

    // Fallback
    let success = response
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(true);
    if success {
        format!("/{command}: done.")
    } else {
        format!("/{command}: command failed.")
    }
}

/// Parse a `/hooks` response body into a list of `HookInfo`.
///
/// Expects the Kiro wire shape `{data: {hooks: [{trigger, command, matcher?}, ...]}}`.
/// Deserializes the whole `data.hooks` array as a typed `Vec<HookInfo>` in one
/// shot: if any entry is structurally malformed (missing `trigger`, missing
/// `command`, wrong types), the whole response is rejected rather than
/// silently dropping individual entries.
///
/// Returns `None` on any of these conditions, so the caller falls back to
/// `format_command_response` and the user still sees the raw response
/// instead of a silently empty panel:
///
/// - `data` field absent → `debug` log
/// - `data.hooks` field absent → `debug` log
/// - structural deserialization failure → `warn` log
/// - any entry has an empty `trigger` or `command` → `warn` log (display
///   defect — would render as a blank row)
///
/// Uses `Deserialize::deserialize` directly on `&Value` (which implements
/// `Deserializer`) to avoid the deep clone of the hooks array that
/// `serde_json::from_value` would require.
fn parse_hooks_response(response: &serde_json::Value) -> Option<Vec<cyril_core::types::HookInfo>> {
    let Some(data) = response.get("data") else {
        tracing::debug!("/hooks response has no `data` field — falling back");
        return None;
    };
    let Some(hooks_value) = data.get("hooks") else {
        tracing::debug!("/hooks response has no `data.hooks` field — falling back");
        return None;
    };
    let hooks = match Vec::<cyril_core::types::HookInfo>::deserialize(hooks_value) {
        Ok(hooks) => hooks,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "malformed /hooks response, falling back to generic command output"
            );
            return None;
        }
    };
    if hooks
        .iter()
        .any(|h| h.trigger.is_empty() || h.command.is_empty())
    {
        tracing::warn!(
            "/hooks response contained a hook with an empty trigger or command — falling back"
        );
        return None;
    }
    Some(hooks)
}

/// Dispatch a `CommandExecuted` response to the UI.
///
/// For `command == "hooks"` with a successful response (`success: true` or
/// absent), parses the hooks and opens the overlay panel. For any other
/// command, or for hooks responses that are structurally invalid or report
/// `success: false`, falls through to `format_command_response` so the
/// backend's `message` field surfaces as a normal command-output line.
///
/// Extracted as a free function so it can be tested directly without
/// constructing a full `App`. Model-specific workarounds (see
/// `App::handle_notification`) stay at the caller site because they mutate
/// session state, not UI state.
fn dispatch_command_executed(
    command: &str,
    response: &serde_json::Value,
    ui_state: &mut cyril_ui::state::UiState,
) {
    let handled_as_panel = command == "hooks" && is_success_response(response) && {
        match parse_hooks_response(response) {
            Some(hooks) => {
                ui_state.show_hooks_panel(hooks);
                true
            }
            None => false,
        }
    };

    if !handled_as_panel {
        let text = format_command_response(command, response);
        ui_state.add_command_output(command.to_string(), text);
    }
}

/// Handle a `/code` command response.
///
/// If the response reports `success: false`, falls through to generic command
/// output (matching the `hooks` pattern). Otherwise routes by response shape:
/// - Panel: shows overlay and, if `Initialized`, marks code intelligence active.
/// - Prompt: validates session, pre-populates chat, sets Busy, returns a
///   deferred `SendPrompt` command (deferred because `handle_notification` is
///   sync and cannot `.await` the bridge send).
/// - Unknown: falls through to generic formatting.
fn dispatch_code_command(
    response: &serde_json::Value,
    session: &cyril_core::session::SessionController,
    ui_state: &mut cyril_ui::state::UiState,
) -> Vec<BridgeCommand> {
    if !is_success_response(response) {
        let text = format_command_response("code", response);
        ui_state.add_command_output("code".to_string(), text);
        return Vec::new();
    }

    match CodeCommandResponse::from_json(response) {
        CodeCommandResponse::Panel(data) => {
            ui_state.set_code_intelligence_active(data.status == LspStatus::Initialized);
            ui_state.show_code_panel(data);
            Vec::new()
        }
        CodeCommandResponse::Prompt { text, label } => {
            let session_id = match session.id().cloned() {
                Some(id) => id,
                None => {
                    tracing::warn!("/code prompt response arrived with no active session");
                    ui_state.add_system_message(
                        "/code: received prompt but no active session — try again.".into(),
                    );
                    return Vec::new();
                }
            };
            // cyril-8ej2: a /code prompt injected mid-turn would hit the bridge's
            // one-turn guard (bridge.rs rejects a 2nd SendPrompt while a turn is in
            // flight), so the SendPrompt would be dropped AFTER we'd committed a
            // UserText + set Sending — a commit-without-send desync. Advise and drop
            // instead, committing nothing (mirrors the no-session branch above). This
            // is a runtime check by necessity: a debug_assert would compile out and
            // silently re-open the desync in release. Scope is Busy only, matching
            // classify_submit — other statuses carry no in-flight prompt_task. debug!
            // (not warn!) because a busy mid-turn injection is expected, not anomalous.
            if matches!(session.status(), SessionStatus::Busy) {
                tracing::debug!("/code prompt dropped: a turn is already in progress");
                ui_state.add_system_message(
                    "/code: agent is busy — prompt not sent. Try again after the current turn."
                        .into(),
                );
                return Vec::new();
            }
            let display = label.as_deref().unwrap_or("Code Intelligence");
            ui_state.add_system_message(format!("/code: {display}"));
            ui_state.add_user_message(&text);
            ui_state.set_activity(Activity::Sending);

            vec![BridgeCommand::SendPrompt {
                session_id,
                prompt: PromptEnvelope::original(vec![text]),
            }]
        }
        CodeCommandResponse::Unknown(ref value) => {
            let text = format_command_response("code", value);
            ui_state.add_command_output("code".to_string(), text);
            Vec::new()
        }
    }
}

/// Dispatch a `/rewind` command response.
///
/// When the agent selects a new session (response carries
/// `data.switchSession: true` plus `data.sessionId`), emit the
/// `session/load` + `session/terminate` pair that client-orchestrates the
/// "fork" — Kiro doesn't have a `session/fork` method; the rewind primitive
/// is `commands/execute rewind {value: "<idx>"}` returning a new session id
/// that the client must explicitly load and switch from. See
/// `docs/cyril-acp-coverage-vs-2.4.1.md` for the wire trace.
///
/// Returns an empty vec for the no-args panel-data response (the panel is
/// rendered via `dispatch_command_executed`) and on any error case.
fn dispatch_rewind_command(
    response: &serde_json::Value,
    session: &cyril_core::session::SessionController,
    ui_state: &mut cyril_ui::state::UiState,
) -> Vec<BridgeCommand> {
    if !is_success_response(response) {
        return Vec::new();
    }
    let data = match response.get("data") {
        Some(d) => d,
        None => return Vec::new(),
    };
    let switch = data
        .get("switchSession")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !switch {
        // Panel-data response (no selection yet). The `turns` payload is
        // rendered by the generic command-output path.
        return Vec::new();
    }
    let new_session_id = match data
        .get("sessionId")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(id) => SessionId::new(id),
        None => {
            tracing::warn!("/rewind response had switchSession:true but no sessionId — skipping");
            return Vec::new();
        }
    };
    let old_session_id = match session.id().cloned() {
        Some(id) => id,
        None => {
            tracing::warn!(
                "/rewind switchSession response arrived with no active session — skipping"
            );
            return Vec::new();
        }
    };
    ui_state.add_system_message(format!(
        "/rewind: switched to new session {} (old session {} will be terminated)",
        new_session_id.as_str(),
        old_session_id.as_str()
    ));
    vec![
        BridgeCommand::LoadSession {
            session_id: new_session_id,
        },
        BridgeCommand::TerminateSession {
            session_id: old_session_id,
        },
    ]
}

/// Dispatch a key press while the `/hooks` panel is visible.
///
/// Extracted as a free function so the full key-map can be unit-tested
/// without constructing an `App`. Esc hides the panel; arrow keys scroll
/// one line; page keys scroll ten lines; other keys are no-ops.
fn dispatch_hooks_panel_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState) {
    match key.code {
        KeyCode::Esc => ui_state.hide_hooks_panel(),
        KeyCode::Up => ui_state.hooks_panel_scroll_up(1),
        KeyCode::Down => ui_state.hooks_panel_scroll_down(1),
        KeyCode::PageUp => ui_state.hooks_panel_scroll_up(10),
        KeyCode::PageDown => ui_state.hooks_panel_scroll_down(10),
        _ => {}
    }
}

fn dispatch_usage_panel_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState) {
    match key.code {
        KeyCode::Esc => ui_state.hide_usage_panel(),
        KeyCode::Tab | KeyCode::Right => ui_state.usage_panel_next_page(),
        KeyCode::BackTab | KeyCode::Left => ui_state.usage_panel_previous_page(),
        KeyCode::Up => ui_state.usage_panel_scroll_up(1),
        KeyCode::Down => ui_state.usage_panel_scroll_down(1),
        KeyCode::PageUp => ui_state.usage_panel_scroll_up(10),
        KeyCode::PageDown => ui_state.usage_panel_scroll_down(10),
        _ => {}
    }
}

/// Handle PageUp/PageDown for main chat scrolling.
/// Returns `true` if the key was consumed.
fn dispatch_chat_scroll_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState) -> bool {
    let (_, h) = ui_state.terminal_size();
    let half_page = ((h as usize) / 2).max(1);
    match key.code {
        KeyCode::PageUp => {
            ui_state.chat_scroll_up(half_page);
            true
        }
        KeyCode::PageDown => {
            ui_state.chat_scroll_down(half_page);
            true
        }
        _ => false,
    }
}

/// Returns `true` if the response either has no `success` field (legacy or
/// optional) or has `success: true`. `success: false` reports a backend
/// error and should never be swallowed by panel-style handlers.
fn is_success_response(response: &serde_json::Value) -> bool {
    response
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use cyril_ui::traits::ChatMessageKind;

    fn test_usage_log() -> UsageLog {
        UsageLog::open_in_memory().expect("in-memory usage log")
    }

    /// A live snapshot request handle with no worker behind it.
    ///
    /// The request receiver is leaked on purpose so the sender stays open and
    /// `request()` succeeds: a dropped receiver makes every request fail, which
    /// silently routes every test through the worker-unavailable path instead
    /// of the one it means to exercise. That is exactly what an earlier version
    /// of this helper did.
    fn live_snapshot_handle() -> UsageSnapshotHandle {
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        std::mem::forget(request_rx);
        UsageSnapshotHandle::for_channel(request_tx)
    }

    /// A snapshot handle whose worker is gone: every `request()` fails.
    fn dead_snapshot_handle() -> UsageSnapshotHandle {
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        drop(request_rx);
        UsageSnapshotHandle::for_channel(request_tx)
    }

    /// A result receiver nothing sends to. The sender is leaked so the channel
    /// never closes and the App's `select!` arm simply stays pending.
    fn idle_snapshot_rx() -> mpsc::UnboundedReceiver<UsageSnapshotResult> {
        let (result_tx, result_rx) = mpsc::unbounded_channel();
        std::mem::forget(result_tx);
        result_rx
    }

    #[test]
    fn enrichment_retry_budget_allows_one_transient_retry_only() {
        let mut transient_attempts = 0;
        assert!(reserve_enrichment_retry(true, &mut transient_attempts));
        assert_eq!(transient_attempts, 1);
        assert!(!reserve_enrichment_retry(true, &mut transient_attempts));
        assert_eq!(transient_attempts, 1);

        let mut terminal_attempts = 0;
        assert!(!reserve_enrichment_retry(false, &mut terminal_attempts));
        assert_eq!(terminal_attempts, 0);
    }

    // ── cyril-nd4h: ui.mouse_capture is actually honored (claims C1, C2) ──────
    //
    // STRESS FIXTURE: all three reachable shapes -- explicit false, explicit
    // true, and absent (default). One-sided fences are the bug class here: a
    // test that only checks `false` passes under an implementation hardcoded to
    // `false`, and a test that only checks the default passes under the
    // hardcoded `true` this ticket removes. The `false` case is the sentinel --
    // it fails against pre-fix code, where App::new called
    // `set_mouse_captured(true)` unconditionally.
    fn app_with_mouse_capture(mouse_capture: bool) -> App {
        App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig {
                mouse_capture,
                ..config::UiConfig::default()
            },
            PathBuf::from("/tmp"),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        )
    }

    #[test]
    fn mouse_capture_false_is_honored() {
        assert!(
            !app_with_mouse_capture(false).mouse_captured(),
            "ui.mouse_capture = false must reach App's startup state; before \
             cyril-nd4h this was hardcoded true and the setting did nothing"
        );
    }

    #[test]
    fn mouse_capture_true_is_honored() {
        assert!(app_with_mouse_capture(true).mouse_captured());
    }

    #[test]
    fn mouse_capture_absent_defaults_to_enabled() {
        // The default must stay ON: a user who never wrote the key keeps the
        // behavior they had before this ticket existed.
        let app = App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            PathBuf::from("/tmp"),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        assert!(app.mouse_captured());
    }

    // cyril-a71q slice 9 / claim C7: notification routing truth table.
    //
    // STRESS FIXTURE. The claim is "only the owned release mutates MAIN state".
    // Stale and absorbed completions never reach the App at all -- the bridge
    // drops them (fenced there by slices 4, 5 and 7). What reaches the App and
    // must still spare main is the FOREIGN-scoped terminal, and this is the rule
    // that spares it.
    #[test]
    fn classify_notification_route_truth_table() {
        let main = SessionId::new("sess_main");
        let foreign = SessionId::new("sess_foreign");

        // ── workflow_owned = false: every pre-jxfu row, byte-identical ──────
        //
        // Unscoped -> global lifecycle event; nothing to compare against.
        // Trackedness is irrelevant here, so both settings are asserted.
        for tracked in [false, true] {
            assert_eq!(
                classify_notification_route(None, Some(&main), tracked, false),
                NotificationRoute::Main
            );
            // Scoped to the main session -> main.
            assert_eq!(
                classify_notification_route(Some(&main), Some(&main), tracked, false),
                NotificationRoute::Main
            );
            // THE CLAIM: a foreign session's terminal must not reach main state.
            assert_eq!(
                classify_notification_route(Some(&foreign), Some(&main), tracked, false),
                NotificationRoute::Subagent,
                "a foreign terminal must never touch main -- the cross-session split-brain"
            );
        }
        // Scoped while no main session is known, but a list_update has already
        // named it: attributable, so it still reaches its own stream.
        assert_eq!(
            classify_notification_route(Some(&foreign), None, true, false),
            NotificationRoute::Subagent,
            "no main session yet must not mean 'main' -- that reroutes a tracked \
             subagent's frames into main state"
        );
        // cyril-tglp: same, but nothing has named the id. Unattributable ->
        // dropped. Returning Main here is the defect; returning Subagent would
        // key a stream by an id that may yet turn out to BE main.
        assert_eq!(
            classify_notification_route(Some(&foreign), None, false, false),
            NotificationRoute::Drop,
            "an unidentified scope with no main session is unattributable, not main"
        );
        // Adversarial: equal ids that are distinct objects still compare as main.
        assert_eq!(
            classify_notification_route(
                Some(&SessionId::new("sess_main")),
                Some(&main),
                false,
                false
            ),
            NotificationRoute::Main,
            "identity is by value, not by pointer"
        );

        // ── workflow_owned = true: the cyril-jxfu rows (C1/C2) ──────────────
        //
        // Unscoped frames stay global no matter what claims exist.
        for tracked in [false, true] {
            assert_eq!(
                classify_notification_route(None, Some(&main), tracked, true),
                NotificationRoute::Main,
                "an unscoped frame is global even while workflow runs exist"
            );
            assert_eq!(
                classify_notification_route(None, None, tracked, true),
                NotificationRoute::Main,
                "an unscoped frame is global even pre-session"
            );
            // C1: a claimed foreign session routes Workflow with main known,
            // whether or not a list_update also (anomalously) tracked it —
            // ownership is the more specific claim and wins the collision.
            assert_eq!(
                classify_notification_route(Some(&foreign), Some(&main), tracked, true),
                NotificationRoute::Workflow,
                "C1: a workflow-owned foreign session belongs to the workflow \
                 store, not the subagent stream (tracked={tracked})"
            );
            // C1: attributable WITHOUT a main session — the claim itself is
            // the attribution, so the Drop arm must never fire here.
            assert_eq!(
                classify_notification_route(Some(&foreign), None, tracked, true),
                NotificationRoute::Workflow,
                "C1: a workflow claim makes a pre-main frame attributable \
                 (tracked={tracked})"
            );
            // C2: a workflow claim on the MAIN session is a wire anomaly, and
            // main-pipeline continuity outranks it. A build that tests
            // ownership before scope==main fails exactly this row.
            assert_eq!(
                classify_notification_route(Some(&main), Some(&main), tracked, true),
                NotificationRoute::Main,
                "C2: main outranks a workflow claim on the main session itself \
                 (tracked={tracked})"
            );
        }
        // AC2 completeness: the pre-existing value-equality adversarial row,
        // extended across the new input like every other combination — main
        // identity is by value under a workflow claim too.
        assert_eq!(
            classify_notification_route(
                Some(&SessionId::new("sess_main")),
                Some(&main),
                false,
                true
            ),
            NotificationRoute::Main,
            "C2: value-equal main identity must hold under a workflow claim"
        );
    }

    // ── cyril-tglp: a scoped frame that predates the main session ────────────
    //
    // STRESS FIXTURE. The bug class is "a routing guard that admits a frame to
    // MAIN state by accident". A one-sided test is the trap here: asserting
    // only that the pre-session frame is dropped passes against an
    // implementation that drops *every* scoped frame, and asserting only the
    // post-session cases passes against the pre-fix code. So all three
    // reachable shapes are exercised, and only the first one is the sentinel
    // (it fails against pre-fix code, where `&& self.session.id().is_some()`
    // let an untracked pre-session frame fall through to main).
    fn test_app() -> App {
        App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            PathBuf::from("/tmp"),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        )
    }

    /// Like `test_app`, but keeps the command receiver so a test can assert
    /// which `BridgeCommand`s a key dispatched.
    fn test_app_with_command_rx() -> (App, tokio::sync::mpsc::Receiver<BridgeCommand>) {
        test_app_with_engine_and_command_rx(AgentEngine::V2)
    }

    fn test_app_with_engine_and_command_rx(
        engine: AgentEngine,
    ) -> (App, tokio::sync::mpsc::Receiver<BridgeCommand>) {
        let (handle, rx) = BridgeHandle::for_tests_with_command_rx();
        (
            App::new(
                handle,
                &config::UiConfig::default(),
                PathBuf::from("/tmp"),
                cyril_core::commands::HooksCommandSource::Agent,
                cyril_core::commands::WorkflowCommandSource::None,
                UsageWiring {
                    log: test_usage_log(),
                    snapshot: live_snapshot_handle(),
                    snapshot_rx: idle_snapshot_rx(),
                },
                engine,
            ),
            rx,
        )
    }

    fn app_with_snapshot_handle(handle: UsageSnapshotHandle) -> App {
        App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            PathBuf::from("/tmp"),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: handle,
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        )
    }

    /// An App whose snapshot requests succeed and whose results the test
    /// delivers by hand.
    fn app_for_tests() -> App {
        app_with_snapshot_handle(live_snapshot_handle())
    }

    /// An App whose snapshot worker is gone.
    fn app_for_tests_without_snapshot_worker() -> App {
        app_with_snapshot_handle(dead_snapshot_handle())
    }

    #[tokio::test]
    async fn memory_failure_does_not_block_initial_session_dispatch() {
        let root = tempfile::tempdir().expect("root");
        let config_path = root.path().join("config.toml");
        std::fs::write(
            &config_path,
            "[memory]\nenabled = false\nunknown_memory_field = true\n",
        )
        .expect("config");
        let report = cyril_memory::load_config_report(&config_path);
        let memory_runtime =
            crate::memory_runtime::MemoryRuntimeHandle::start(report.memory().clone());
        assert!(matches!(
            memory_runtime.status(),
            crate::memory_runtime::MemoryRuntimeStatus::Failed(_)
        ));

        let (mut app, mut commands) = test_app_with_command_rx();
        app.set_memory_runtime(memory_runtime, ProjectBinding::Disabled);
        app.create_initial_session(root.path().to_path_buf(), None)
            .await;
        let command = tokio::time::timeout(Duration::from_secs(1), commands.recv())
            .await
            .expect("command deadline")
            .expect("bridge command");
        assert!(matches!(command, BridgeCommand::NewSession { .. }));
    }
    #[test]
    fn typed_memory_result_formats_through_ui_command_output() {
        let mut app = test_app();
        let status = MemoryStatusView::ready(
            "instance",
            1,
            cyril_core::types::MemoryStoreVersions::new(1, 1),
        );
        app.handle_command_result(CommandResult::memory_status(status));
        let message = app.ui_state.messages().last().expect("command output");
        match message.kind() {
            cyril_ui::traits::ChatMessageKind::CommandOutput { command, text } => {
                assert_eq!(command, "memory");
                assert!(text.contains("Memory: ready"));
                assert!(text.contains("memory 1, knowledge 1"));
            }
            other => panic!("expected memory command output, got {other:?}"),
        }
    }

    fn establish_main_session(app: &mut App, session_id: &SessionId) {
        app.handle_notification(RoutedNotification::global(
            Notification::UsageSessionStarted {
                session_id: session_id.clone(),
                origin: SessionOrigin::Fresh,
            },
        ));
        app.handle_notification(RoutedNotification::global(Notification::SessionCreated {
            session_id: session_id.clone(),
            current_mode: None,
            current_model: Some("openai-codex/gpt-5.6-luna".into()),
            available_modes: Vec::new(),
            available_models: Vec::new(),
        }));
    }

    #[tokio::test]
    async fn usage_modal_command_and_key_priority() {
        let (mut app, mut rx) = test_app_with_command_rx();
        assert!(
            app.session.id().is_none(),
            "fixture must have no active session"
        );
        app.ui_state.insert_text("/usage");
        app.submit_input().await.expect("execute local /usage");
        assert!(app.ui_state.has_usage_panel());
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        app.ui_state.insert_text("draft");
        app.handle_key(key(KeyCode::Char('x')))
            .await
            .expect("modal consumes character");
        assert_eq!(app.ui_state.input_text(), "draft");
        app.handle_terminal_event(Event::Paste("paste".into()))
            .await
            .expect("modal consumes paste");
        assert_eq!(app.ui_state.input_text(), "draft");

        app.handle_key(key(KeyCode::Tab)).await.expect("next page");
        assert_eq!(
            app.ui_state.usage_panel().map(|panel| panel.page),
            Some(cyril_ui::traits::UsagePage::Costs)
        );
        app.handle_key(key(KeyCode::BackTab))
            .await
            .expect("previous page");
        assert_eq!(
            app.ui_state.usage_panel().map(|panel| panel.page),
            Some(cyril_ui::traits::UsagePage::Overview)
        );
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("close modal");
        assert!(!app.ui_state.has_usage_panel());
    }

    #[tokio::test]
    async fn usage_account_query_order_and_state_matrix() {
        let (mut app, mut rx) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
        establish_main_session(&mut app, &SessionId::new("sess_kas"));
        app.ui_state.insert_text("/usage");
        app.submit_input().await.expect("open KAS usage");
        assert!(app.ui_state.has_usage_panel(), "local snapshot opens first");
        assert!(matches!(
            app.ui_state
                .usage_panel()
                .map(|panel| &panel.account_status),
            Some(cyril_ui::traits::UsageAccountStatus::Loading)
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(BridgeCommand::QueryUsageAccount)
        ));
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        app.handle_notification(RoutedNotification::global(
            Notification::UsageAccountUpdated {
                account: UsageAccount {
                    plan_name: "KIRO PRO MAX".to_owned(),
                    billing_cycle_reset: "2026-09-01".to_owned(),
                    overages_enabled: false,
                    is_enterprise: false,
                    overage_capable: false,
                    usage_breakdowns: Vec::new(),
                    bonus_credits: Vec::new(),
                    add_on_credits: Vec::new(),
                },
                fetched_at_ms: Some(42),
            },
        ));
        assert!(matches!(
            app.ui_state
                .usage_panel()
                .map(|panel| &panel.account_status),
            Some(cyril_ui::traits::UsageAccountStatus::Fresh)
        ));
        app.handle_notification(RoutedNotification::global(
            Notification::UsageAccountQueryFailed {
                message: "offline".to_owned(),
            },
        ));
        assert!(matches!(
            app.ui_state
                .usage_panel()
                .map(|panel| &panel.account_status),
            Some(cyril_ui::traits::UsageAccountStatus::Stale(message))
                if message == "offline"
        ));

        let (mut v2, mut v2_rx) = test_app_with_engine_and_command_rx(AgentEngine::V2);
        establish_main_session(&mut v2, &SessionId::new("v2-session"));
        v2.ui_state.insert_text("/usage");
        v2.submit_input().await.expect("open v2 usage");
        assert!(v2.ui_state.has_usage_panel());
        assert!(matches!(
            v2_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        let (mut dead, dead_rx) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
        drop(dead_rx);
        establish_main_session(&mut dead, &SessionId::new("sess_dead"));
        dead.ui_state.insert_text("/usage");
        dead.submit_input()
            .await
            .expect("local usage must open after bridge death");
        assert!(dead.ui_state.has_usage_panel());
        assert!(matches!(
            dead.ui_state
                .usage_panel()
                .map(|panel| &panel.account_status),
            Some(cyril_ui::traits::UsageAccountStatus::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn open_usage_panel_refreshes_on_turn_and_context_writes() {
        let session_id = SessionId::new("s-refresh");
        let (mut app, mut command_rx) = test_app_with_command_rx();
        establish_main_session(&mut app, &session_id);
        app.ui_state.insert_text("hello");
        app.submit_input().await.expect("send prompt");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(BridgeCommand::SendPrompt { .. })
        ));
        app.ui_state.open_usage_panel();
        // Land a first snapshot so the panel holds values and is Idle. Without
        // this it sits in Computing, where a refresh request deliberately does
        // NOT overwrite the status — nothing has been computed to refresh yet.
        let first = app.usage_log.snapshot().expect("initial usage snapshot");
        app.handle_usage_snapshot(UsageSnapshotResult::Ready(Box::new(first)));
        app.redraw_needed = false;

        app.handle_notification(RoutedNotification::scoped(
            session_id.clone(),
            Notification::TurnUsageCaptured(TokenUsage::new(10, 4, 6, None, None, None)),
        ));
        app.handle_notification(RoutedNotification::scoped(
            session_id.clone(),
            Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn,
            },
        ));
        // cyril-nanu: the write still lands in the log synchronously, but the
        // panel is now REFRESHED off the loop — so the observable here is the
        // request and the marker, not an updated snapshot on this frame.
        assert_eq!(
            app.usage_log
                .snapshot()
                .expect("log readable")
                .overview
                .requests,
            1,
            "the turn is persisted synchronously; only the panel refresh moved"
        );
        assert!(
            matches!(
                app.ui_state.usage_panel().map(|panel| &panel.refresh),
                Some(cyril_ui::traits::UsageRefreshStatus::Refreshing)
            ),
            "a turn write marks the panel as refreshing"
        );
        assert!(app.redraw_needed);

        app.redraw_needed = false;
        app.handle_notification(RoutedNotification::scoped(
            session_id,
            Notification::ContextBreakdownUpdated {
                usage_percentage: 42.0,
                breakdown: None,
            },
        ));
        assert_eq!(
            app.usage_log
                .snapshot()
                .expect("log readable")
                .context
                .latest
                .as_ref()
                .map(|sample| sample.percentage),
            Some(42.0),
            "the context sample is persisted synchronously"
        );
        assert!(
            matches!(
                app.ui_state.usage_panel().map(|panel| &panel.refresh),
                Some(cyril_ui::traits::UsageRefreshStatus::Refreshing)
            ),
            "a context sample marks the panel as refreshing"
        );
        assert!(app.redraw_needed);
    }

    #[tokio::test]
    async fn all_prompt_paths_start_and_failed_send_aborts_usage() {
        let session_id = SessionId::new("s-main");

        // Direct typed prompt.
        let (mut direct, mut direct_rx) = test_app_with_command_rx();
        establish_main_session(&mut direct, &session_id);
        direct.ui_state.insert_text("hello");
        direct.submit_input().await.expect("send direct prompt");
        assert!(matches!(
            direct_rx.try_recv(),
            Ok(BridgeCommand::SendPrompt { .. })
        ));
        direct.handle_notification(RoutedNotification::scoped(
            session_id.clone(),
            Notification::TurnUsageCaptured(TokenUsage::new(10, 4, 6, None, None, None)),
        ));
        direct.handle_notification(RoutedNotification::scoped(
            session_id.clone(),
            Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn,
            },
        ));
        assert_eq!(
            direct
                .usage_log
                .snapshot()
                .expect("direct usage snapshot")
                .overview
                .requests,
            1
        );

        // One-shot/deferred prompt.
        let (mut deferred, mut deferred_rx) = test_app_with_command_rx();
        deferred.startup_prompt = Some("startup".into());
        deferred.handle_notification(RoutedNotification::global(
            Notification::UsageSessionStarted {
                session_id: session_id.clone(),
                origin: SessionOrigin::Fresh,
            },
        ));
        let commands = deferred.handle_notification(RoutedNotification::global(
            Notification::SessionCreated {
                session_id: session_id.clone(),
                current_mode: None,
                current_model: None,
                available_modes: Vec::new(),
                available_models: Vec::new(),
            },
        ));
        assert_eq!(commands.len(), 1);
        deferred
            .dispatch_deferred_command(commands.into_iter().next().expect("one deferred prompt"))
            .await;
        assert!(matches!(
            deferred_rx.try_recv(),
            Ok(BridgeCommand::SendPrompt { .. })
        ));
        deferred.handle_notification(RoutedNotification::scoped(
            session_id.clone(),
            Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn,
            },
        ));
        assert_eq!(
            deferred
                .usage_log
                .snapshot()
                .expect("deferred usage snapshot")
                .overview
                .requests,
            1,
            "usage-less completed prompts are still recent records"
        );

        // Failed send must remove the observer pending turn.
        let (handle, command_rx) = BridgeHandle::for_tests_with_command_rx();
        drop(command_rx);
        let mut failed = App::new(
            handle,
            &config::UiConfig::default(),
            PathBuf::from("/tmp"),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        establish_main_session(&mut failed, &session_id);
        failed.ui_state.insert_text("will fail");
        assert!(failed.submit_input().await.is_err());
        assert!(
            failed.begin_usage_turn(&session_id),
            "failed send must abort its pending usage turn"
        );
        assert!(failed.usage_observer.abort_turn(&session_id));
    }

    #[test]
    #[ignore = "reference-workstation prompt coordination budget"]
    fn usage_prompt_coordination_budget_reference() {
        let session_id = SessionId::new("budget-session");
        let mut app = test_app();
        establish_main_session(&mut app, &session_id);
        let started = Instant::now();
        for _ in 0..10_000 {
            assert!(app.begin_usage_turn(&session_id));
            assert!(app.usage_observer.abort_turn(&session_id));
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed <= Duration::from_secs(10),
            "10,000 prompt coordination cycles exceeded 1ms/event: {elapsed:?}"
        );
    }
    /// cyril-14ou C9 (plumbing half; the live half — engine honors the cancel
    /// — passed at design time, archived in .cyril-14ou/findings.md). Four
    /// arms: Esc during a stalled busy turn sends CancelRequest AND escalates
    /// the chip; Esc while busy-but-not-stalled cancels without touching stall
    /// state; Esc while the approval overlay owns input does neither (the
    /// key-layer priority holds). Buggy implementations these fail under:
    /// unconditional cancel-sent marking, marking wired before the busy guard,
    /// Esc bypassing the overlay layer.
    #[tokio::test]
    async fn esc_marks_cancel_sent_during_stall() {
        use cyril_ui::traits::Activity;
        let stalled = Notification::TurnStalled {
            quiet: std::time::Duration::from_secs(31),
        };

        // Arm 1: stalled busy turn — cancel goes out, chip escalates.
        let (mut app, mut rx) = test_app_with_command_rx();
        app.session.set_status(SessionStatus::Busy);
        app.ui_state.set_activity(Activity::Streaming);
        assert!(app.ui_state.apply_notification(&stalled));
        app.handle_key(key(KeyCode::Esc)).await.expect("esc");
        assert!(
            matches!(rx.try_recv(), Ok(BridgeCommand::CancelRequest)),
            "Esc while busy must dispatch CancelRequest"
        );
        assert_eq!(
            app.ui_state.stall().map(|s| s.cancel_sent),
            Some(true),
            "stall chip must escalate to cancel-sent"
        );

        // Arm 2: busy but not stalled — cancel only, stall stays absent.
        let (mut app, mut rx) = test_app_with_command_rx();
        app.session.set_status(SessionStatus::Busy);
        app.ui_state.set_activity(Activity::Streaming);
        app.handle_key(key(KeyCode::Esc)).await.expect("esc");
        assert!(matches!(rx.try_recv(), Ok(BridgeCommand::CancelRequest)));
        assert!(app.ui_state.stall().is_none(), "no chip to escalate");

        // Arm 3: approval overlay owns input — no cancel, no escalation.
        let (mut app, mut rx) = test_app_with_command_rx();
        app.session.set_status(SessionStatus::Busy);
        app.ui_state.set_activity(Activity::Streaming);
        assert!(app.ui_state.apply_notification(&stalled));
        let (request, _responder_rx) = trust_request("main");
        app.ui_state.show_approval(request);
        app.handle_key(key(KeyCode::Esc)).await.expect("esc");
        assert!(
            rx.try_recv().is_err(),
            "the overlay consumed Esc; no CancelRequest may go out"
        );
        assert_eq!(
            app.ui_state.stall().map(|s| s.cancel_sent),
            Some(false),
            "no escalation while the overlay owns input"
        );

        // Arm 4: not busy — no cancel goes out, so nothing may escalate even
        // with a (stale) chip up. Catches marking wired outside the busy guard.
        let (mut app, mut rx) = test_app_with_command_rx();
        app.ui_state.set_activity(Activity::Streaming);
        assert!(app.ui_state.apply_notification(&stalled));
        app.handle_key(key(KeyCode::Esc)).await.expect("esc");
        assert!(rx.try_recv().is_err(), "not busy: no CancelRequest");
        assert_eq!(
            app.ui_state.stall().map(|s| s.cancel_sent),
            Some(false),
            "no cancel sent means no escalation"
        );
    }

    fn trust_request(
        session_id: &str,
    ) -> (
        PermissionRequest,
        tokio::sync::oneshot::Receiver<PermissionResponse>,
    ) {
        let (responder, receiver) = tokio::sync::oneshot::channel();
        (
            PermissionRequest {
                session_id: SessionId::new(session_id),
                tool_call: ToolCall::new(
                    ToolCallId::new(format!("tool-{session_id}")),
                    "echo safe".into(),
                    ToolKind::Execute,
                    ToolCallStatus::Pending,
                    None,
                ),
                message: "Allow command?".into(),
                options: vec![PermissionOption {
                    id: PermissionOptionId::new("always"),
                    label: "Always".into(),
                    kind: PermissionOptionKind::AllowAlways,
                    is_destructive: false,
                }],
                trust_options: vec![TrustOption {
                    label: "Full command".into(),
                    display: "echo safe".into(),
                    setting_key: "allowedCommands".into(),
                    patterns: vec!["echo safe".into()],
                }],
                responder,
            },
            receiver,
        )
    }
    #[tokio::test]
    async fn buffered_input_stops_before_promoted_approval() {
        let mut app = test_app();
        let (mut first, mut first_response) = trust_request("first");
        let (mut second, mut second_response) = trust_request("second");
        first.trust_options.clear();
        second.trust_options.clear();
        app.ui_state.show_approval(first);
        app.ui_state.show_approval(second);

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let mut buffered = Some(Event::Key(enter));
        app.handle_terminal_event_batch(Event::Key(enter), || buffered.take())
            .await
            .expect("approval input batch");

        assert!(buffered.is_some(), "buffered input must wait for redraw");
        assert!(matches!(
            first_response.try_recv(),
            Ok(PermissionResponse::Selected { .. })
        ));
        assert!(matches!(
            second_response.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        assert_eq!(
            app.ui_state
                .approval()
                .expect("second approval promoted")
                .session_id,
            SessionId::new("second")
        );
    }

    #[test]
    fn foreign_approval_trust_is_not_persisted_to_main_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".kiro").join("agents");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("myagent.json");
        let original = br#"{"unrelated":{"keep":true}}"#;
        std::fs::write(&config_path, original).unwrap();

        let mut app = App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            tmp.path().to_path_buf(),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        let main_id = SessionId::new("main-session");
        app.session
            .set_session(main_id.clone(), SessionStatus::Active);
        app.session.apply_notification(&Notification::ModeChanged {
            mode_id: ModeId::new("myagent"),
        });

        let (foreign, foreign_response) = trust_request("peer-session");
        let (main, main_response) = trust_request(main_id.as_str());
        app.ui_state.show_approval(foreign);
        app.ui_state.show_approval(main);

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        app.handle_approval_key(enter);
        app.handle_approval_key(enter);
        assert!(matches!(
            foreign_response.blocking_recv(),
            Ok(PermissionResponse::Selected {
                trust_option: Some(label),
                ..
            }) if label == "Full command"
        ));
        assert_eq!(
            std::fs::read(&config_path).unwrap(),
            original,
            "foreign approval must not write the main agent config"
        );
        assert!(app.ui_state.messages().iter().any(|message| {
            matches!(
                message.kind(),
                ChatMessageKind::System(text)
                    if text.contains("session-scoped") && text.contains("peer-session")
            )
        }));

        app.handle_approval_key(enter);
        app.handle_approval_key(enter);
        assert!(matches!(
            main_response.blocking_recv(),
            Ok(PermissionResponse::Selected { .. })
        ));
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(persisted["unrelated"]["keep"], true);
        assert_eq!(
            persisted["toolsSettings"]["execute_bash"]["allowedCommands"],
            serde_json::json!(["echo safe"])
        );
    }

    #[test]
    fn pre_main_approval_trust_is_session_scoped() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".kiro").join("agents");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("myagent.json");
        let original = b"{}";
        std::fs::write(&config_path, original).unwrap();

        let mut app = App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            tmp.path().to_path_buf(),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        app.session.apply_notification(&Notification::ModeChanged {
            mode_id: ModeId::new("myagent"),
        });
        let (request, response) = trust_request("early-session");
        app.ui_state.show_approval(request);

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        app.handle_approval_key(enter);
        app.handle_approval_key(enter);

        assert!(matches!(
            response.blocking_recv(),
            Ok(PermissionResponse::Selected { .. })
        ));
        assert_eq!(std::fs::read(&config_path).unwrap(), original);
        assert!(app.ui_state.messages().iter().any(|message| {
            matches!(
                message.kind(),
                ChatMessageKind::System(text)
                    if text.contains("session-scoped") && text.contains("early-session")
            )
        }));
    }

    #[test]
    fn empty_origin_cannot_authorize_main_config_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".kiro").join("agents");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("myagent.json");
        let original = b"{}";
        std::fs::write(&config_path, original).unwrap();

        let mut app = App::new(
            BridgeHandle::for_tests(),
            &config::UiConfig::default(),
            tmp.path().to_path_buf(),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        app.session
            .set_session(SessionId::new(""), SessionStatus::Active);
        app.session.apply_notification(&Notification::ModeChanged {
            mode_id: ModeId::new("myagent"),
        });
        let (request, response) = trust_request("");
        app.ui_state.show_approval(request);

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        app.handle_approval_key(enter);
        app.handle_approval_key(enter);

        assert!(matches!(
            response.blocking_recv(),
            Ok(PermissionResponse::Selected { .. })
        ));
        assert_eq!(std::fs::read(&config_path).unwrap(), original);
        assert!(app.ui_state.messages().iter().any(|message| {
            matches!(
                message.kind(),
                ChatMessageKind::System(text)
                    if text.contains("session-scoped") && text.contains("unknown session")
            )
        }));
    }

    fn metadata_frame(sid: &SessionId) -> Notification {
        Notification::MetadataUpdated {
            refusal: None,
            context_usage: Some(ContextUsage::new(75.0)),
            metering: None,
            tokens: None,
            duration_ms: None,
            effort: EffortUpdate::Unchanged,
            session_id: Some(sid.clone()),
        }
    }

    #[test]
    fn pre_session_scoped_frame_spares_main() {
        let foreign = SessionId::new("sess_foreign");
        let mut app = test_app();
        assert!(
            app.session.id().is_none(),
            "precondition: no main session has been created yet"
        );

        let deferred = app.handle_notification(RoutedNotification::scoped(
            foreign.clone(),
            metadata_frame(&foreign),
        ));

        assert!(deferred.is_empty(), "a dropped frame defers no commands");
        assert!(
            app.session.context_usage().is_none(),
            "a scoped frame arriving before SessionCreated must not mutate the \
             main SessionController -- it is not attributable to main"
        );
        assert!(
            app.ui_state.context_usage().is_none(),
            "...nor the main UiState: the toolbar would show a foreign \
             session's context usage as if it were the user's"
        );
        assert!(
            !app.ui_state.subagent_ui().streams().contains_key(&foreign),
            "and it must not be guessed into a subagent stream either -- the id \
             may yet turn out to BE main, which would key a phantom stream by \
             the main session id"
        );
    }

    #[test]
    fn pre_session_tracked_subagent_frame_still_streams() {
        let tracked = SessionId::new("sess_tracked");
        let mut app = test_app();

        // list_update is global — it lands before any main session exists.
        app.handle_notification(RoutedNotification::global(
            Notification::SubagentListUpdated {
                subagents: vec![SubagentInfo::new(
                    tracked.clone(),
                    "reviewer",
                    "semantic_reviewer",
                    "review the diff",
                    SubagentStatus::Working { message: None },
                )],
                pending_stages: Vec::new(),
            },
        ));
        assert!(app.session.id().is_none(), "precondition: still no main");

        app.handle_notification(RoutedNotification::scoped(
            tracked.clone(),
            Notification::AgentMessage(AgentMessage {
                text: "subagent output".into(),
                is_streaming: true,
            }),
        ));

        // The cyril-a71q behavior this fix must NOT overcorrect away: a
        // list_update has already proven this id is a subagent, so its frames
        // are attributable even with no main session.
        let stream = app
            .ui_state
            .subagent_ui()
            .streams()
            .get(&tracked)
            .expect("a tracked subagent's pre-session frames still reach its stream");
        assert_eq!(stream.streaming_text(), "subagent output");
        assert!(
            app.ui_state.streaming_text().is_empty(),
            "and main is still spared"
        );
    }

    #[test]
    fn post_session_untracked_frame_still_streams_optimistically() {
        let main = SessionId::new("sess_main");
        let foreign = SessionId::new("sess_foreign");
        let mut app = test_app();

        app.handle_notification(RoutedNotification::global(Notification::SessionCreated {
            session_id: main.clone(),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        }));

        app.handle_notification(RoutedNotification::scoped(
            foreign.clone(),
            Notification::AgentMessage(AgentMessage {
                text: "racing list_update".into(),
                is_streaming: true,
            }),
        ));

        // Once main is known, "scoped and not main" is decidable: the frame is
        // definitively foreign, so the optimistic stream stays.
        let stream = app
            .ui_state
            .subagent_ui()
            .streams()
            .get(&foreign)
            .expect("an untracked frame with main known still streams optimistically");
        assert_eq!(stream.streaming_text(), "racing list_update");
        assert!(app.ui_state.streaming_text().is_empty());
    }

    #[test]
    fn post_session_main_scoped_frame_updates_main() {
        let main = SessionId::new("sess_main");
        let mut app = test_app();

        app.handle_notification(RoutedNotification::global(Notification::SessionCreated {
            session_id: main.clone(),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        }));

        let deferred = app.handle_notification(RoutedNotification::scoped(
            main.clone(),
            metadata_frame(&main),
        ));

        assert!(deferred.is_empty(), "a metadata frame defers no commands");
        assert_eq!(
            app.session.context_usage().map(ContextUsage::percentage),
            Some(75.0),
            "a main-scoped frame must update the main SessionController"
        );
        assert_eq!(
            app.ui_state.context_usage(),
            Some(75.0),
            "a main-scoped frame must update the main UiState"
        );
        assert!(
            !app.ui_state.subagent_ui().streams().contains_key(&main),
            "the main session id must not create a subagent stream"
        );
    }

    // ── cyril-0ffy: the one-shot `--prompt` submits after session ready ──────
    //
    // STRESS FIXTURE: both reachable startup shapes plus the replay edge. A
    // startup carrying `--prompt TEXT` must defer exactly one SendPrompt with
    // TEXT once the initial session is ready, and never again on a later
    // SessionCreated (`/new`). A bare startup must defer nothing. The first
    // test is the sentinel: it fails against pre-fix code, where `Cli::prompt`
    // was parsed and then dropped on the floor — startup had no seam that
    // could accept it, so the process sat idle.

    fn session_created_frame(sid: &SessionId) -> RoutedNotification {
        RoutedNotification::global(Notification::SessionCreated {
            session_id: sid.clone(),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        })
    }

    /// The original `SendPrompt` payloads among `deferred`.
    fn sent_prompts(deferred: &[BridgeCommand]) -> Vec<(&SessionId, &[String])> {
        deferred
            .iter()
            .filter_map(|cmd| match cmd {
                BridgeCommand::SendPrompt { session_id, prompt } => {
                    Some((session_id, prompt.original_blocks()))
                }
                _ => None,
            })
            .collect()
    }

    /// Drive one off-loop memory result the way the `select!` arm would.
    async fn drain_one_memory_result(app: &mut App) {
        let result = tokio::time::timeout(Duration::from_secs(5), app.memory_task_rx.recv())
            .await
            .expect("memory task result within bound")
            .expect("memory task result");
        app.handle_memory_task_result(result).await;
    }

    async fn recv_prompt(
        commands: &mut tokio::sync::mpsc::Receiver<BridgeCommand>,
    ) -> (SessionId, Vec<String>, Vec<String>) {
        match tokio::time::timeout(Duration::from_secs(5), commands.recv())
            .await
            .expect("bridge command within bound")
            .expect("bridge command")
        {
            BridgeCommand::SendPrompt { session_id, prompt } => {
                let original = prompt.original_blocks().to_vec();
                (session_id, original, prompt.into_wire_blocks())
            }
            other => panic!("expected SendPrompt, got {other:?}"),
        }
    }

    fn last_system_message(app: &App) -> String {
        app.ui_state
            .messages()
            .iter()
            .rev()
            .find_map(|message| match message.kind() {
                ChatMessageKind::System(text) => Some(text.clone()),
                _ => None,
            })
            .expect("a system message")
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn c5_first_prompt_is_ordered_exactly_once_and_source_clean() {
        let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let memory = runtime.bind(workspace.path());
        memory
            .teach(cyril_memory::LessonText::new("prefer boring Rust").expect("lesson"))
            .await
            .expect("teach");

        let main = SessionId::new("sess_interactive");
        let (mut app, mut commands) = test_app_with_command_rx();
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Bound(memory),
        );
        assert!(
            app.handle_notification(session_created_frame(&main))
                .is_empty()
        );
        let original = "Original Ω prompt with attachment semantics";
        app.ui_state.insert_text(original);
        app.submit_input().await.expect("submit original");

        // The lesson lookup runs off the event loop: nothing has reached the
        // bridge yet, and the session already refuses a second prompt.
        assert!(commands.try_recv().is_err());
        assert!(matches!(app.session.status(), SessionStatus::Busy));
        drain_one_memory_result(&mut app).await;

        let (session_id, original_blocks, content_blocks) = recv_prompt(&mut commands).await;
        assert_eq!(session_id, main);
        assert_eq!(
            original_blocks,
            [original.to_owned()],
            "C5 source prompt polluted"
        );
        assert_eq!(content_blocks.len(), 2, "{content_blocks:?}");
        assert!(content_blocks[0].starts_with("<CYRIL_LESSONS"));
        assert!(content_blocks[0].contains("- prefer boring Rust"));
        assert_eq!(content_blocks[1], original);
        // The transcript shows the user's text only; the block is wire-only.
        assert!(app.ui_state.messages().iter().any(
            |message| matches!(message.kind(), ChatMessageKind::UserText(text) if text == original)
        ));
        assert!(!app.ui_state.messages().iter().any(|message| {
            matches!(
                message.kind(),
                ChatMessageKind::UserText(text) if text.contains("<CYRIL_LESSONS")
            )
        }));

        // Exactly once per session: the next prompt goes out untouched and
        // synchronously.
        app.session.set_status(SessionStatus::Active);
        app.ui_state.insert_text("second prompt");
        app.submit_input().await.expect("submit second");
        let (_, original_blocks, content_blocks) = recv_prompt(&mut commands).await;
        assert_eq!(original_blocks, ["second prompt".to_owned()]);
        assert_eq!(content_blocks, ["second prompt".to_owned()]);
        runtime.shutdown().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn c7_turn_inspection_survives_ui_retention_and_is_scoped() {
        let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let foreign_workspace = tempfile::tempdir().expect("foreign workspace");
        let memory = runtime.bind(workspace.path());
        let foreign = runtime.bind(foreign_workspace.path());
        let source_turn_id = cyril_memory::SourceTurnId::from_bytes([0x17; 16]);
        let session_id = cyril_memory::SourceSessionId::new("source-session").expect("session id");
        let events = vec![
            cyril_memory::SourceTurnEvent::new(
                session_id.clone(),
                source_turn_id,
                0,
                cyril_memory::SourceTurnEventKind::Started {
                    bridge_turn_id: 42,
                    started_at_ms: 1_000,
                    block_count: 1,
                },
            )
            .expect("started"),
            cyril_memory::SourceTurnEvent::new(
                session_id.clone(),
                source_turn_id,
                1,
                cyril_memory::SourceTurnEventKind::PromptFragment {
                    block_index: 0,
                    fragment_index: 0,
                    text: "distinctive retained decision".to_owned(),
                    is_last: true,
                },
            )
            .expect("prompt"),
            cyril_memory::SourceTurnEvent::new(
                session_id.clone(),
                source_turn_id,
                2,
                cyril_memory::SourceTurnEventKind::AssistantFragment {
                    fragment_index: 0,
                    text: "use the boring implementation".to_owned(),
                },
            )
            .expect("assistant"),
            cyril_memory::SourceTurnEvent::new(
                session_id,
                source_turn_id,
                3,
                cyril_memory::SourceTurnEventKind::Finished {
                    disposition: cyril_memory::SourceTurnDisposition::Completed,
                    finished_at_ms: 2_000,
                },
            )
            .expect("finished"),
        ];
        memory
            .capture_batch(cyril_memory::CaptureBatch::new(events).expect("capture batch"))
            .await
            .expect("capture");

        let (mut app, _commands) = test_app_with_command_rx();
        for index in 0..200 {
            app.ui_state
                .add_system_message(format!("unrelated retained UI row {index}"));
        }
        let listed = run_memory_action(memory.clone(), MemoryCommandAction::Turns).await;
        assert!(listed.contains(&source_turn_id.to_string()), "C7 {listed}");
        let inspected = run_memory_action(
            memory,
            MemoryCommandAction::InspectTurn {
                source_turn_id: source_turn_id.to_string(),
            },
        )
        .await;
        assert!(
            inspected.contains("Prompt:\ndistinctive retained decision")
                && inspected.contains("Session: source-session")
                && inspected.contains("Bridge turn: 42"),
            "C7 {inspected}"
        );
        let foreign_result = run_memory_action(
            foreign,
            MemoryCommandAction::InspectTurn {
                source_turn_id: source_turn_id.to_string(),
            },
        )
        .await;
        assert!(
            foreign_result.starts_with("Memory error:"),
            "C7 foreign scope leaked: {foreign_result}"
        );
        runtime.shutdown().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn first_prompt_lessons_wait_for_a_starting_companion() {
        let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let memory = runtime.bind(workspace.path());
        memory
            .teach(cyril_memory::LessonText::new("prefer boring Rust").expect("lesson"))
            .await
            .expect("teach");
        runtime.set_starting();

        let main = SessionId::new("sess_cold");
        let (mut app, mut commands) = test_app_with_command_rx();
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Bound(memory),
        );
        app.handle_notification(session_created_frame(&main));

        // Cold machine: the companion is still starting when the first prompt
        // is typed. The prompt goes out without lessons, and the session
        // stays eligible.
        app.ui_state.insert_text("first");
        app.submit_input().await.expect("submit first");
        drain_one_memory_result(&mut app).await;
        let (_, original_blocks, content_blocks) = recv_prompt(&mut commands).await;
        assert_eq!(original_blocks, ["first".to_owned()]);
        assert_eq!(content_blocks, ["first".to_owned()]);
        assert_eq!(app.first_prompt_lessons_pending.as_ref(), Some(&main));

        runtime.set_ready();
        app.session.set_status(SessionStatus::Active);
        app.ui_state.insert_text("second");
        app.submit_input().await.expect("submit second");
        drain_one_memory_result(&mut app).await;
        let (_, original_blocks, content_blocks) = recv_prompt(&mut commands).await;
        assert_eq!(original_blocks, ["second".to_owned()]);
        assert_eq!(content_blocks.len(), 2, "{content_blocks:?}");
        assert!(content_blocks[0].contains("- prefer boring Rust"));
        assert_eq!(content_blocks[1], "second");
        assert!(app.first_prompt_lessons_pending.is_none());

        app.session.set_status(SessionStatus::Active);
        app.ui_state.insert_text("third");
        app.submit_input().await.expect("submit third");
        let (_, original_blocks, content_blocks) = recv_prompt(&mut commands).await;
        assert_eq!(original_blocks, ["third".to_owned()]);
        assert_eq!(content_blocks, ["third".to_owned()]);
        runtime.shutdown().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn memory_commands_run_off_the_event_loop() {
        let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let memory = runtime.bind(workspace.path());
        let (mut app, _commands) = test_app_with_command_rx();
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Bound(memory),
        );
        let before = app.ui_state.messages().len();
        app.ui_state.insert_text("/memory list");
        app.submit_input().await.expect("submit list");
        // The command returned without touching the transcript: the round
        // trip is in flight on a task, not awaited on the loop.
        assert_eq!(app.ui_state.messages().len(), before);
        drain_one_memory_result(&mut app).await;
        assert_eq!(last_system_message(&app), "No active project lessons.");

        app.ui_state.insert_text("/memory teach prefer boring Rust");
        app.submit_input().await.expect("submit teach");
        drain_one_memory_result(&mut app).await;
        assert!(last_system_message(&app).starts_with("Lesson created:"));

        app.ui_state.insert_text("/memory list");
        app.submit_input().await.expect("submit list again");
        drain_one_memory_result(&mut app).await;
        assert!(last_system_message(&app).contains("prefer boring Rust"));
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn lesson_commands_report_why_the_project_is_unbound() {
        let (mut app, _commands) = test_app_with_command_rx();
        let reason = "Git metadata file /work/proj/.git is invalid";
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Unbound {
                reason: reason.to_owned(),
            },
        );
        app.ui_state.insert_text("/memory teach prefer boring Rust");
        app.submit_input().await.expect("submit teach");
        assert_eq!(
            last_system_message(&app),
            format!("Memory is unavailable for this project: {reason}")
        );
        app.ui_state.insert_text("/memory status");
        app.submit_input().await.expect("submit status");
        let status = match app
            .ui_state
            .messages()
            .last()
            .expect("status output")
            .kind()
        {
            ChatMessageKind::CommandOutput { text, .. } => text.clone(),
            other => panic!("expected command output, got {other:?}"),
        };
        assert!(
            status.contains(&format!("Project: unbound — {reason}")),
            "{status}"
        );

        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Disabled,
        );
        app.ui_state.insert_text("/memory list");
        app.submit_input().await.expect("submit list");
        assert!(
            last_system_message(&app).starts_with("Memory is disabled."),
            "{}",
            last_system_message(&app)
        );
    }

    #[tokio::test]
    async fn memory_runtime_shutdown_is_explicit_and_idempotent() {
        let (mut app, _commands) = test_app_with_command_rx();
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            ProjectBinding::Disabled,
        );
        assert!(app.memory_runtime.is_some());
        app.shutdown_memory_runtime().await;
        assert!(app.memory_runtime.is_none());
        app.shutdown_memory_runtime().await;
        assert!(app.memory_runtime.is_none());
    }

    #[tokio::test]
    async fn oneshot_prompt_submitted_after_session_ready() {
        let main = SessionId::new("sess_main");
        let mut app = test_app();
        // Startup the way main() wires it: session creation carries the parsed
        // `--prompt` value. The test handle has no bridge behind it, so the
        // NewSession send fails harmlessly; SessionCreated is then fed by hand
        // exactly as the notification loop would deliver it.
        app.create_initial_session(
            PathBuf::from("/tmp"),
            Some("run the requested smoke".into()),
        )
        .await;

        let deferred = app.handle_notification(session_created_frame(&main));

        let prompts = sent_prompts(&deferred);
        assert_eq!(
            prompts.len(),
            1,
            "exactly one SendPrompt once the initial session is ready"
        );
        assert_eq!(
            prompts[0].0, &main,
            "the one-shot targets the session just created"
        );
        assert_eq!(
            prompts[0].1,
            ["run the requested smoke".to_string()],
            "the one-shot carries the --prompt text as its only content block"
        );
        // UX parity with a typed submit: the text appears as a user message.
        assert!(
            app.ui_state.messages().iter().any(
                |m| matches!(m.kind(), ChatMessageKind::UserText(t) if t == "run the requested smoke")
            ),
            "the one-shot prompt must show in the transcript like a typed prompt"
        );

        // A later session (e.g. `/new`) must NOT replay the one-shot.
        let second = SessionId::new("sess_second");
        let deferred = app.handle_notification(session_created_frame(&second));
        assert!(
            sent_prompts(&deferred).is_empty(),
            "the one-shot submits exactly once — a later SessionCreated must not replay it"
        );
    }

    #[tokio::test]
    async fn startup_without_prompt_submits_nothing() {
        let main = SessionId::new("sess_main");
        let mut app = test_app();
        app.create_initial_session(PathBuf::from("/tmp"), None)
            .await;

        let deferred = app.handle_notification(session_created_frame(&main));

        assert!(
            sent_prompts(&deferred).is_empty(),
            "interactive startup (no --prompt) must submit nothing"
        );
    }

    // cyril-bm1j Slice 9 / claims C1, C2: submit routing truth table.
    #[test]
    fn classify_submit_truth_table() {
        // C1: busy + session -> Steer.
        assert_eq!(
            classify_submit(&SessionStatus::Busy, true),
            SubmitRoute::Steer
        );
        // C2: idle (Active) + session -> Prompt.
        assert_eq!(
            classify_submit(&SessionStatus::Active, true),
            SubmitRoute::Prompt
        );
        // No session -> NoSession.
        assert_eq!(
            classify_submit(&SessionStatus::Disconnected, false),
            SubmitRoute::NoSession
        );
        // Adversarial: busy but no session -> NoSession (no-session beats busy).
        assert_eq!(
            classify_submit(&SessionStatus::Busy, false),
            SubmitRoute::NoSession
        );
        // Only Busy steers — other present-session states prompt (unchanged path).
        assert_eq!(
            classify_submit(&SessionStatus::Compacting, true),
            SubmitRoute::Prompt
        );
        assert_eq!(
            classify_submit(&SessionStatus::Initializing, true),
            SubmitRoute::Prompt
        );
        // Error is the only data-carrying SessionStatus variant; the design lists
        // (Error,true)->Prompt. A future broadening of the steer predicate must not
        // silently route an errored session's Enter to a steer it can't accept.
        assert_eq!(
            classify_submit(
                &SessionStatus::Error {
                    message: "boom".into()
                },
                true
            ),
            SubmitRoute::Prompt
        );
    }

    // cyril-bm1j Slice 10 / claim C7: steer gate truth table.
    #[test]
    fn steer_gate_truth_table() {
        assert_eq!(steer_gate(false, true), SteerGate::Send);
        assert_eq!(steer_gate(true, true), SteerGate::AdvisoryUnsupported);
        assert_eq!(steer_gate(false, false), SteerGate::AdvisoryNoSession);
        // Adversarial: unsupported AND no session -> NoSession wins (checked first).
        assert_eq!(steer_gate(true, false), SteerGate::AdvisoryNoSession);
    }

    // cyril-bm1j Slice 11 / claims C1+C3+C7 integration + cyril-2vcc regression.
    #[tokio::test]
    async fn dispatch_steer_busy_sends_steer_and_echoes() {
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);

        dispatch_steer(&mut ui, &session, &bridge, "halt".into())
            .await
            .unwrap();

        // cyril-2vcc: a busy submit emits SteerSession, NOT a second SendPrompt
        // (which the bridge would reject -> the message would be lost).
        match rx.try_recv() {
            Ok(BridgeCommand::SteerSession { message, .. }) => assert_eq!(message, "halt"),
            other => panic!("expected SteerSession{{halt}}, got {other:?}"),
        }
        // Optimistic Queued echo present immediately.
        assert!(
            ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::SteerEcho {
                    text,
                    status: cyril_ui::traits::SteerEchoStatus::Queued,
                    ..
                } if text == "halt"
            )),
            "expected a Queued steer echo for 'halt'"
        );
    }

    // cyril-bm1j Slice 11 / claim C7 keystone: unsupported -> no send, no echo.
    #[tokio::test]
    async fn dispatch_steer_unsupported_sends_nothing_no_echo() {
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        session.apply_notification(&Notification::SteeringUnsupported {
            message: "steering requires kiro-cli 2.7.0+".into(),
        });

        dispatch_steer(&mut ui, &session, &bridge, "halt".into())
            .await
            .unwrap();

        // Keystone: nothing sent on a known-unsupported session, so no optimistic
        // echo can ever get stuck (the bridge drops such steers silently).
        assert!(
            rx.try_recv().is_err(),
            "unsupported session must not send a SteerSession"
        );
        assert!(
            !ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::SteerEcho {
                    status: cyril_ui::traits::SteerEchoStatus::Queued,
                    ..
                }
            )),
            "no Queued echo on an unsupported session"
        );
        assert!(
            ui.messages()
                .iter()
                .any(|m| matches!(m.kind(), cyril_ui::traits::ChatMessageKind::System(_))),
            "an advisory system message is shown instead"
        );
    }

    // cyril-vgcm C11: dispatch_clear_steer gate matrix + zero optimistic
    // mutation (D4). Bug classes: optimistic pre-drain (chips flipped before
    // the broadcast — desyncs from an id-scoped or failed clear), success
    // chatter (a system message on the silent-success path), and a divergent
    // gate (clear gating must equal steer_gate's for all cells).
    #[tokio::test]
    async fn dispatch_clear_steer_gates_and_never_mutates() {
        use cyril_ui::traits::{ChatMessageKind, SteerEchoStatus};

        // Cell 1: no session -> advisory, nothing sent.
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let session = SessionController::new();
        dispatch_clear_steer(&mut ui, &session, &bridge)
            .await
            .unwrap();
        assert!(rx.try_recv().is_err(), "no session: nothing on the bridge");
        assert!(
            ui.messages()
                .iter()
                .any(|m| matches!(m.kind(), ChatMessageKind::System(s) if s.contains("No active session"))),
            "no-session advisory shown"
        );

        // Cell 2: steering-unsupported session -> advisory, nothing sent.
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        session.apply_notification(&Notification::SteeringUnsupported {
            message: "steering requires kiro-cli 2.7.0+".into(),
        });
        dispatch_clear_steer(&mut ui, &session, &bridge)
            .await
            .unwrap();
        assert!(rx.try_recv().is_err(), "unsupported: nothing on the bridge");

        // Cell 3: healthy session with queued chips -> ClearSteering sent,
        // chips AND counter untouched, NO system message (silent success).
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        ui.add_steer_echo("queued one");
        ui.add_steer_echo("queued two");
        let msgs_before = ui.messages().len();
        dispatch_clear_steer(&mut ui, &session, &bridge)
            .await
            .unwrap();
        match rx.try_recv() {
            Ok(BridgeCommand::ClearSteering { session_id }) => {
                assert_eq!(session_id.as_str(), "sess_1");
            }
            other => panic!("expected ClearSteering, got {other:?}"),
        }
        assert_eq!(ui.steering_queued(), 2, "no optimistic counter drain");
        assert_eq!(
            ui.messages()
                .iter()
                .filter(|m| matches!(
                    m.kind(),
                    ChatMessageKind::SteerEcho {
                        status: SteerEchoStatus::Queued,
                        ..
                    }
                ))
                .count(),
            2,
            "no optimistic chip flip — the broadcast is the truth (D4)"
        );
        assert_eq!(
            ui.messages().len(),
            msgs_before,
            "silent success: no message added on dispatch"
        );
    }

    // cyril-7n1l: a failed bridge send must leave NO phantom optimistic steer
    // state — the steer never reached the backend, so no SteeringConsumed/
    // Cleared/Unsupported notification will ever drain the chip or the echo.
    #[tokio::test]
    async fn failed_steer_send_leaves_no_phantom_chip() {
        let (tx, rx) = mpsc::channel(8);
        drop(rx); // Bridge thread gone — every send fails with BridgeClosed.
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);

        let result = dispatch_steer(&mut ui, &session, &bridge, "halt".into()).await;

        assert!(
            result.is_err(),
            "a closed bridge channel must still propagate the error"
        );
        assert_eq!(
            ui.steering_queued(),
            0,
            "failed send must not leave an optimistic steer chip"
        );
        assert!(
            !ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::SteerEcho {
                    status: cyril_ui::traits::SteerEchoStatus::Queued,
                    ..
                }
            )),
            "failed send must not leave an unresolved Queued echo"
        );
    }

    #[test]
    fn format_response_tools_list() {
        let response = serde_json::json!({
            "success": true,
            "message": "Available tools:",
            "data": {
                "tools": [
                    {"name": "read", "description": "Read a file.\nMore details", "source": "built-in"},
                    {"name": "fetch", "description": "Fetch a URL", "source": "mcp-server"}
                ]
            }
        });
        let result = format_command_response("tools", &response);
        assert!(result.contains("Available tools:"));
        // First sentence only; the trailing line is dropped.
        assert!(result.contains("  read — Read a file.\n"));
        assert!(result.contains("  fetch — Fetch a URL (mcp-server)\n"));
    }

    #[test]
    fn summarize_description_joins_hard_wrapped_first_sentence() {
        // Mirrors the real `subagent` tool description, whose opening sentence is
        // hard-wrapped across physical lines and preceded by a leading newline.
        let desc = "\nSpawn and coordinate multiple AI agents in a pipeline (DAG). Each stage runs as a\npersistent session. Stages with no depends_on start immediately in parallel.\n\nMODES:\n- blocking";
        // Must not cut off mid-sentence at "Each stage runs as a".
        assert_eq!(
            summarize_description(desc),
            "Spawn and coordinate multiple AI agents in a pipeline (DAG)."
        );
    }

    #[test]
    fn summarize_description_no_sentence_boundary_returns_paragraph() {
        let desc = "Read a file\nMore details";
        assert_eq!(summarize_description(desc), "Read a file More details");
    }

    #[test]
    fn summarize_description_truncates_at_question_or_exclamation() {
        // A first sentence ending in '?' or '!' must truncate there — not fall
        // through and return the whole paragraph (the prior ". "-only split did).
        // Relevant for third-party (MCP) tool descriptions cyril doesn't control.
        assert_eq!(
            summarize_description("Need a file? Use the read tool."),
            "Need a file?"
        );
        assert_eq!(
            summarize_description("Run it! Then check the output."),
            "Run it!"
        );
        // The earliest terminator wins regardless of which kind it is.
        assert_eq!(summarize_description("Why? Because. More."), "Why?");
    }

    #[test]
    fn format_response_context_breakdown() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "contextUsagePercentage": 42.5,
                "model": "claude-sonnet",
                "breakdown": {
                    "contextFiles": {"tokens": 1000, "percent": 10.0},
                    "tools": {"tokens": 500, "percent": 5.0},
                    "yourPrompts": {"tokens": 2000, "percent": 20.0},
                    "kiroResponses": {"tokens": 0, "percent": 0.0}
                }
            }
        });
        let result = format_command_response("context", &response);
        assert!(result.contains("Context: 42.5% used (model: claude-sonnet)"));
        assert!(result.contains("Context files: 1000 tokens (10.0%)"));
        assert!(result.contains("Tools: 500 tokens (5.0%)"));
        assert!(result.contains("Your prompts: 2000 tokens (20.0%)"));
        // Zero-token categories should be omitted
        assert!(!result.contains("Kiro responses"));
    }

    #[test]
    fn format_response_context_breakdown_lists_files() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "contextUsagePercentage": 7.6,
                "model": "auto",
                "breakdown": {
                    "contextFiles": {
                        "tokens": 8495,
                        "percent": 4.2,
                        "items": [
                            {"name": "AGENTS.md", "tokens": 1843, "percent": 0.92, "matched": true},
                            {"name": "review-process.md", "tokens": 5004, "percent": 2.5, "matched": true},
                            {"name": "SKILL.md", "tokens": 130, "percent": 0.06, "matched": true, "auto_included": true},
                            {"name": "stale.md", "tokens": 50, "percent": 0.02, "matched": false}
                        ]
                    },
                    "tools": {"tokens": 6665, "percent": 3.3}
                }
            }
        });
        let result = format_command_response("context", &response);
        // Category summary still present.
        assert!(result.contains("Context files: 8495 tokens (4.2%)"));
        // Per-file rows are rendered, indented under the category. The trailing
        // newline pins the exact row format (indent, em-dash, .1 precision) and
        // proves a plain matched row carries no stray (auto)/(unmatched) tag.
        assert!(result.contains("    AGENTS.md — 1843 tokens (0.9%)\n"));
        // Heaviest file sorts before lighter ones.
        let heavy = result.find("review-process.md").unwrap();
        let light = result.find("AGENTS.md").unwrap();
        assert!(heavy < light, "items should be sorted by tokens descending");
        // Optional flags surface useful state.
        assert!(result.contains("SKILL.md — 130 tokens (0.1%) (auto)"));
        assert!(result.contains("stale.md — 50 tokens (0.0%) (unmatched)"));
        // Categories without items (tools) render no child rows.
        assert!(result.contains("Tools: 6665 tokens (3.3%)"));
    }

    #[test]
    fn format_response_usage_breakdowns() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "planName": "Pro",
                "usageBreakdowns": [
                    {"displayName": "Fast requests", "used": 150.0, "limit": 500.0, "percentage": 30}
                ]
            }
        });
        let result = format_command_response("usage", &response);
        assert!(result.contains("Plan: Pro"));
        assert!(result.contains("Fast requests: 150 / 500 (30%)"));
    }

    #[test]
    fn format_response_plain_message() {
        let response = serde_json::json!({
            "success": true,
            "message": "Context compacted successfully."
        });
        let result = format_command_response("compact", &response);
        assert_eq!(result, "Context compacted successfully.");
    }

    #[test]
    fn format_response_success_fallback() {
        let response = serde_json::json!({"success": true});
        let result = format_command_response("compact", &response);
        assert_eq!(result, "/compact: done.");
    }

    #[test]
    fn format_response_failure_fallback() {
        let response = serde_json::json!({"success": false});
        let result = format_command_response("compact", &response);
        assert_eq!(result, "/compact: command failed.");
    }

    #[test]
    fn format_response_null_data() {
        let response = serde_json::Value::Null;
        let result = format_command_response("test", &response);
        assert_eq!(result, "/test: done.");
    }

    #[test]
    fn format_response_tools_builtin_source_omitted() {
        let response = serde_json::json!({
            "success": true,
            "message": "Tools:",
            "data": {
                "tools": [
                    {"name": "read", "description": "Read a file", "source": "built-in"}
                ]
            }
        });
        let result = format_command_response("tools", &response);
        // built-in source tag should NOT appear
        assert!(!result.contains("(built-in)"));
        assert!(result.contains("  read — Read a file\n"));
    }

    // --- parse_hooks_response tests ---

    #[test]
    fn parse_hooks_response_well_formed() {
        let response = serde_json::json!({
            "success": true,
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "echo pre", "matcher": "read"},
                    {"trigger": "Stop", "command": "notify done"}
                ]
            }
        });
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].trigger, "PreToolUse");
        assert_eq!(hooks[0].command, "echo pre");
        assert_eq!(hooks[0].matcher.as_deref(), Some("read"));
        assert_eq!(hooks[1].trigger, "Stop");
        assert!(hooks[1].matcher.is_none());
    }

    #[test]
    fn parse_hooks_response_empty_array() {
        let response = serde_json::json!({"data": {"hooks": []}});
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert!(hooks.is_empty());
    }

    #[test]
    fn parse_hooks_response_missing_data_returns_none() {
        let response = serde_json::json!({"success": true, "message": "no data"});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_data_without_hooks_field() {
        let response = serde_json::json!({"data": {"other": "stuff"}});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_hooks_wrong_type_returns_none() {
        let response = serde_json::json!({"data": {"hooks": "not an array"}});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_any_malformed_entry() {
        // Fail-fast semantics: a single bad entry rejects the whole response,
        // so the caller falls through to the generic command-output path and
        // the user sees the raw JSON instead of a silently truncated panel.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "valid"},
                    {"not_a_hook": true},
                    {"trigger": "Stop", "command": "also valid", "matcher": "write"}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_missing_required_field() {
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse"} // missing required `command`
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_with_empty_trigger() {
        // Structural Serde validation accepts empty strings for required
        // fields, so we guard against them explicitly to prevent the widget
        // from rendering a blank trigger column.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "", "command": "echo hi"}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_with_empty_command() {
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": ""}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_mixed_valid_and_empty_entries() {
        // Fail-fast: one bad entry rejects the whole response, same as
        // the malformed-entry case.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "valid"},
                    {"trigger": "Stop", "command": ""}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_preserves_ordering() {
        // parse_hooks_response preserves wire order; sorting happens in
        // `UiState::show_hooks_panel`, not in the parser or the widget.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "Stop", "command": "z"},
                    {"trigger": "AgentSpawn", "command": "a"},
                ]
            }
        });
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert_eq!(hooks[0].trigger, "Stop");
        assert_eq!(hooks[1].trigger, "AgentSpawn");
    }

    // --- is_success_response tests ---

    #[test]
    fn is_success_missing_field_defaults_true() {
        let response = serde_json::json!({"data": {"hooks": []}});
        assert!(is_success_response(&response));
    }

    #[test]
    fn is_success_explicit_true() {
        let response = serde_json::json!({"success": true, "data": {}});
        assert!(is_success_response(&response));
    }

    #[test]
    fn is_success_explicit_false() {
        let response = serde_json::json!({"success": false, "message": "oops"});
        assert!(!is_success_response(&response));
    }

    #[test]
    fn is_success_wrong_type_defaults_true() {
        // Non-bool success field is treated as missing and defaults to true.
        let response = serde_json::json!({"success": "yes", "data": {}});
        assert!(is_success_response(&response));
    }

    // --- dispatch_command_executed tests ---

    fn valid_hooks_response() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "echo pre", "matcher": "read"},
                    {"trigger": "Stop", "command": "notify done"}
                ]
            }
        })
    }

    #[test]
    fn dispatch_hooks_valid_response_opens_panel_and_adds_no_message() {
        let mut ui_state = UiState::new(500);
        let response = valid_hooks_response();

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(ui_state.has_hooks_panel(), "panel should be open");
        assert_eq!(
            ui_state.hooks_panel().expect("panel").hooks.len(),
            2,
            "both hooks should be parsed"
        );
        assert_eq!(
            ui_state.messages().len(),
            0,
            "no command-output message when panel handles the response"
        );
    }

    #[test]
    fn dispatch_hooks_malformed_entry_falls_through_to_message() {
        let mut ui_state = UiState::new(500);
        // Missing `command` field — whole response rejected.
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse"}
                ]
            }
        });

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "panel should NOT open for malformed response"
        );
        assert_eq!(
            ui_state.messages().len(),
            1,
            "should fall through and add a command-output message"
        );
    }

    #[test]
    fn dispatch_hooks_success_false_surfaces_error_message() {
        let mut ui_state = UiState::new(500);
        // Backend reports an error — critical case: the previous implementation
        // opened an empty panel and discarded the `message` field, hiding the
        // error from the user.
        let response = serde_json::json!({
            "success": false,
            "message": "session expired",
            "data": {"hooks": []}
        });

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "panel should NOT open when backend reports success: false"
        );
        assert_eq!(
            ui_state.messages().len(),
            1,
            "error message should be added as a command-output message"
        );
        // The error message should be visible to the user. format_command_response
        // returns the `message` field directly when no structured data shape matches.
        let msg_text = match ui_state.messages()[0].kind() {
            cyril_ui::traits::ChatMessageKind::CommandOutput { text, .. } => text.clone(),
            other => panic!("expected CommandOutput, got {other:?}"),
        };
        assert!(
            msg_text.contains("session expired"),
            "user should see the backend error message; got: {msg_text}"
        );
    }

    #[test]
    fn dispatch_hooks_missing_data_falls_through_to_message() {
        let mut ui_state = UiState::new(500);
        let response = serde_json::json!({"success": true, "message": "no hooks data"});

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(!ui_state.has_hooks_panel());
        assert_eq!(ui_state.messages().len(), 1);
    }

    #[test]
    fn dispatch_non_hooks_command_adds_message() {
        let mut ui_state = UiState::new(500);
        let response = serde_json::json!({
            "success": true,
            "message": "Context compacted successfully."
        });

        dispatch_command_executed("compact", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "non-hooks commands should never open the hooks panel"
        );
        assert_eq!(ui_state.messages().len(), 1);
    }

    // --- dispatch_hooks_panel_key tests ---

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn state_with_open_panel(num_hooks: usize) -> UiState {
        let mut ui_state = UiState::new(500);
        let hooks: Vec<cyril_core::types::HookInfo> = (0..num_hooks)
            .map(|i| cyril_core::types::HookInfo {
                trigger: format!("T{i}"),
                command: format!("cmd{i}"),
                matcher: None,
                id: None,
                name: None,
                enabled: None,
                // v2-shaped, which is the point: the KAS-only fields are
                // absent, and `enabled: None` means "this registry does not
                // model enablement", not "disabled".
            })
            .collect();
        ui_state.show_hooks_panel(hooks);
        ui_state
    }

    #[test]
    fn hooks_panel_key_esc_closes_panel() {
        let mut ui_state = state_with_open_panel(3);
        dispatch_hooks_panel_key(key(KeyCode::Esc), &mut ui_state);
        assert!(!ui_state.has_hooks_panel());
    }

    #[test]
    fn hooks_panel_key_down_scrolls_down_one() {
        let mut ui_state = state_with_open_panel(5);
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 1);
    }

    #[test]
    fn hooks_panel_key_up_scrolls_up_one() {
        let mut ui_state = state_with_open_panel(5);
        // Scroll down twice first to have something to scroll up from.
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::Up), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 1);
    }

    #[test]
    fn hooks_panel_key_pgdown_scrolls_down_ten() {
        let mut ui_state = state_with_open_panel(20);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 10);
    }

    #[test]
    fn hooks_panel_key_pgup_scrolls_up_ten() {
        let mut ui_state = state_with_open_panel(20);
        // Scroll down past 10 first.
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        // Now at offset ~19 (clamped from 20 to len-1 = 19).
        dispatch_hooks_panel_key(key(KeyCode::PageUp), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 9);
    }

    #[test]
    fn hooks_panel_key_unknown_is_noop() {
        let mut ui_state = state_with_open_panel(5);
        dispatch_hooks_panel_key(key(KeyCode::Char('x')), &mut ui_state);
        assert!(ui_state.has_hooks_panel(), "panel should still be open");
        assert_eq!(
            ui_state.hooks_panel().expect("panel").scroll_offset,
            0,
            "unknown key should not affect scroll"
        );
    }

    #[test]
    fn hooks_panel_key_scroll_down_on_empty_panel_is_noop() {
        // Edge case: empty panel. saturating_sub(1) on len=0 yields 0; scroll
        // must stay at 0 without panicking.
        let mut ui_state = state_with_open_panel(0);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 0);
    }

    // --- Chat scroll key dispatch tests ---

    #[test]
    fn chat_scroll_pageup_consumed_and_enters_browse_mode() {
        let mut ui_state = UiState::new(500);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::PageUp), &mut ui_state);
        assert!(consumed, "PageUp should be consumed");
        assert!(
            ui_state.chat_scroll_back().is_some(),
            "should enter browse mode"
        );
    }

    #[test]
    fn chat_scroll_pagedown_consumed() {
        let mut ui_state = UiState::new(500);
        ui_state.chat_scroll_up(20);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::PageDown), &mut ui_state);
        assert!(consumed, "PageDown should be consumed");
    }

    #[test]
    fn chat_scroll_non_scroll_key_not_consumed() {
        let mut ui_state = UiState::new(500);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::Char('a')), &mut ui_state);
        assert!(!consumed, "regular key should not be consumed");
        assert!(
            ui_state.chat_scroll_back().is_none(),
            "scroll state should not change"
        );
    }

    #[test]
    fn chat_scroll_pageup_uses_half_terminal_height() {
        let mut ui_state = UiState::new(500);
        ui_state.set_terminal_size(80, 24);
        dispatch_chat_scroll_key(key(KeyCode::PageUp), &mut ui_state);
        assert_eq!(ui_state.chat_scroll_back(), Some(12));
    }

    // --- dispatch_code_command tests ---

    fn code_session() -> cyril_core::session::SessionController {
        let mut session = cyril_core::session::SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Active);
        session
    }

    fn busy_code_session() -> cyril_core::session::SessionController {
        let mut session = cyril_core::session::SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        session
    }

    fn code_prompt_json() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "data": { "executePrompt": "Analyze the code...", "label": "Code Summary" }
        })
    }

    #[test]
    fn dispatch_code_panel_opens_overlay() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "initialized",
                    "detectedLanguages": ["rust"],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
        assert!(ui.has_code_panel());
        assert!(ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_panel_failed_does_not_set_active() {
        let session = code_session();
        let mut ui = UiState::new(500);
        dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "failed",
                    "detectedLanguages": [],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(ui.has_code_panel());
        assert!(!ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_panel_failed_resets_active_flag() {
        let session = code_session();
        let mut ui = UiState::new(500);
        ui.set_code_intelligence_active(true);
        assert!(ui.code_intelligence_active());

        dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "failed",
                    "detectedLanguages": [],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(
            !ui.code_intelligence_active(),
            "failed status should reset the flag"
        );
    }

    #[test]
    fn dispatch_code_success_false_falls_through_to_message() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": false,
                "message": "Not configured",
                "data": {
                    "status": "initialized",
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
        assert!(
            !ui.has_code_panel(),
            "panel should NOT open on success:false"
        );
        assert!(!ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_prompt_returns_deferred_command() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze the code...",
                    "label": "Code Summary"
                }
            }),
            &session,
            &mut ui,
        );
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], BridgeCommand::SendPrompt { .. }));
        assert_eq!(ui.activity(), Activity::Sending);
        // claim #1: the active path commits the injected prompt as a UserText.
        assert!(
            ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::UserText(t) if t == "Analyze the code..."
            )),
            "active path must commit the injected prompt"
        );
    }

    // cyril-8ej2: busy-guard fences for the /code Prompt arm. A /code prompt
    // injected mid-turn would hit the bridge's one-turn guard and be dropped
    // AFTER a UserText was committed + activity set Sending (commit-without-send
    // desync). The guard advises and drops instead.

    #[test]
    fn dispatch_code_prompt_busy_drops_no_send() {
        // claim #2: busy + Prompt emits zero bridge commands.
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(&code_prompt_json(), &busy_code_session(), &mut ui);
        assert!(
            result.is_empty(),
            "busy must not emit a SendPrompt the bridge would reject"
        );
    }

    #[test]
    fn dispatch_code_prompt_busy_commits_no_user_message() {
        // claim #3: no commit-without-send desync.
        let mut ui = UiState::new(500);
        dispatch_code_command(&code_prompt_json(), &busy_code_session(), &mut ui);
        assert!(
            !ui.messages()
                .iter()
                .any(|m| matches!(m.kind(), cyril_ui::traits::ChatMessageKind::UserText(_))),
            "busy must not commit a UserText it will never send"
        );
    }

    #[test]
    fn dispatch_code_prompt_busy_leaves_activity() {
        // claim #4: busy must not strand activity at Sending.
        let mut ui = UiState::new(500);
        ui.set_activity(Activity::Streaming);
        dispatch_code_command(&code_prompt_json(), &busy_code_session(), &mut ui);
        assert_eq!(
            ui.activity(),
            Activity::Streaming,
            "busy must leave the in-flight activity untouched"
        );
    }

    #[test]
    fn dispatch_code_prompt_busy_advises() {
        // claim #5: the drop is visible to the user as a System advisory.
        let mut ui = UiState::new(500);
        dispatch_code_command(&code_prompt_json(), &busy_code_session(), &mut ui);
        assert!(
            ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::System(t) if t.contains("busy")
            )),
            "busy drop must surface a System advisory"
        );
    }

    #[test]
    fn dispatch_code_panel_opens_when_busy() {
        // claim #7 (stress fixture 1): the guard is scoped to the Prompt arm —
        // a Panel response arriving while busy still opens the panel.
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "initialized",
                    "detectedLanguages": ["rust"],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &busy_code_session(),
            &mut ui,
        );
        assert!(result.is_empty());
        assert!(
            ui.has_code_panel(),
            "busy must not suppress a Panel response"
        );
    }

    #[test]
    fn dispatch_code_prompt_no_session_shows_error() {
        let session = cyril_core::session::SessionController::new(); // no session ID
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze...",
                    "label": "Summary"
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
        // Should show error, not the prompt system message
        assert!(!ui.messages().is_empty());
        assert_eq!(ui.activity(), Activity::Idle);
    }

    #[test]
    fn dispatch_code_prompt_without_label_uses_default() {
        let session = code_session();
        let mut ui = UiState::new(500);
        dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze..."
                }
            }),
            &session,
            &mut ui,
        );
        // System message should use the default label
        let has_default_label = ui.messages().iter().any(|m| {
            matches!(m.kind(), cyril_ui::traits::ChatMessageKind::System(s) if s.contains("Code Intelligence"))
        });
        assert!(
            has_default_label,
            "should use 'Code Intelligence' as default label"
        );
    }

    #[test]
    fn dispatch_code_unknown_adds_command_output() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "message": "Something unexpected"
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
        assert!(!ui.has_code_panel());
        assert!(!ui.messages().is_empty());
    }

    // --- dispatch_rewind_command tests ---
    //
    // /rewind selection orchestration: the agent's commands/execute response
    // carrying `switchSession: true` + a new sessionId must produce the
    // LoadSession + TerminateSession pair that client-orchestrates the fork.
    // See `docs/cyril-acp-coverage-vs-2.4.1.md` "TUI recorder findings" for
    // the empirically-captured wire sequence.

    fn rewind_session() -> cyril_core::session::SessionController {
        let mut session = cyril_core::session::SessionController::new();
        session.set_session(SessionId::new("old-session-uuid"), SessionStatus::Active);
        session
    }

    #[test]
    fn dispatch_rewind_panel_data_returns_empty() {
        // No-args rewind call returns the turn list — no switchSession means
        // no follow-up dispatch.
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "turns": [
                        {
                            "group": "2%",
                            "label": "Say hello",
                            "logIndex": 0,
                            "responseSnippet": "Hello."
                        }
                    ]
                }
            }),
            &session,
            &mut ui,
        );
        assert!(
            result.is_empty(),
            "panel data should produce no deferred commands"
        );
    }

    #[test]
    fn dispatch_rewind_switch_session_emits_load_and_terminate() {
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "message": "Rewound to earlier turn (new session new-session-uuid)",
                "data": {
                    "sessionId": "new-session-uuid",
                    "switchSession": true
                }
            }),
            &session,
            &mut ui,
        );
        assert_eq!(
            result.len(),
            2,
            "should emit LoadSession + TerminateSession"
        );
        match &result[0] {
            BridgeCommand::LoadSession { session_id } => {
                assert_eq!(session_id.as_str(), "new-session-uuid");
            }
            other => panic!("expected LoadSession first, got {other:?}"),
        }
        match &result[1] {
            BridgeCommand::TerminateSession { session_id } => {
                assert_eq!(session_id.as_str(), "old-session-uuid");
            }
            other => panic!("expected TerminateSession second, got {other:?}"),
        }
        // System message announces the swap
        assert!(!ui.messages().is_empty());
    }

    #[test]
    fn dispatch_rewind_switch_session_without_sessionid_is_noop() {
        // Defensive: if Kiro signals switchSession but omits the new
        // sessionId, we can't orchestrate. Warn and return empty.
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": { "switchSession": true }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn dispatch_rewind_switch_session_without_active_session_is_noop() {
        // Defensive: switchSession with no current session ID — we can't
        // terminate "the old one" because there isn't one yet.
        let session = cyril_core::session::SessionController::new();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "sessionId": "new-session-uuid",
                    "switchSession": true
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn dispatch_rewind_success_false_returns_empty() {
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": false,
                "message": "Cannot rewind beyond first turn"
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }

    // ── cyril-6beh slice 22: exact-once App ownership and error isolation ──
    //
    // The tracker/session/UI counters below are incremented at the REAL call
    // sites inside handle_notification (not a parallel fake dispatcher), so
    // these tests prove the routing shape itself: a workflow frame reaches
    // the tracker once and every SessionController/UiState consumer zero
    // times, and a state error stays warning-only without corrupting state.

    fn workflow_id(value: &str) -> WorkflowId {
        match WorkflowId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid workflow id fixture: {error}"),
        }
    }

    fn workflow_node_id(value: &str) -> WorkflowNodeId {
        match WorkflowNodeId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid node id fixture: {error}"),
        }
    }

    fn workflow_run_started_frame(id: &str) -> Notification {
        Notification::Workflow(Box::new(WorkflowEvent::RunStarted(
            WorkflowRunStarted::new(
                workflow_id(id),
                format!("recipe-{id}"),
                serde_json::json!({"input": true}),
                Vec::new(),
                None,
            ),
        )))
    }

    fn workflow_node_started_frame(id: &str, node: &str) -> Notification {
        let path =
            WorkflowNodePath::try_new(&workflow_id(id), vec![id.to_owned(), node.to_owned()])
                .expect("canonical node path fixture");
        Notification::Workflow(Box::new(WorkflowEvent::NodeStarted(
            WorkflowNodeStarted::new(
                workflow_id(id),
                workflow_node_id(node),
                path,
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            ),
        )))
    }

    /// A `run_complete` whose final snapshot declares two sibling nodes with
    /// the same node id — a duplicate canonical path that the tracker must
    /// reject atomically. Deliberately constructed at the domain level (not a
    /// converter output) so it reaches the tracker exactly as a wire frame
    /// would.
    fn duplicate_path_completion_frame(id: &str) -> Notification {
        let snapshot = WorkflowSnapshot::new(
            workflow_id(id),
            format!("recipe-{id}"),
            WorkflowRunStatus::Completed,
            WorkflowSnapshotData::new(
                serde_json::json!({"input": true}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(workflow_node_id("root"), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![
                    WorkflowNodeSnapshot::new(
                        WorkflowNodeDescriptor::step(
                            workflow_node_id("dup"),
                            "agent".to_owned(),
                            None,
                            None,
                        ),
                        WorkflowNodeStatus::Completed,
                        Vec::new(),
                    ),
                    WorkflowNodeSnapshot::new(
                        WorkflowNodeDescriptor::step(
                            workflow_node_id("dup"),
                            "agent".to_owned(),
                            None,
                            None,
                        ),
                        WorkflowNodeStatus::Completed,
                        Vec::new(),
                    ),
                ],
            ),
            WorkflowSnapshotMetadata::new("created".to_owned(), 1),
        );
        let completion = match WorkflowRunCompleted::new(
            workflow_id(id),
            WorkflowCompletionStatus::Completed,
            snapshot,
        ) {
            Ok(completion) => completion,
            Err(error) => panic!("valid completion fixture rejected: {error}"),
        };
        Notification::Workflow(Box::new(WorkflowEvent::RunCompleted(completion)))
    }

    /// Run `f` under a WARN-level capture subscriber; return its result and
    /// the captured log text.
    fn with_captured_logs<T>(f: impl FnOnce() -> T) -> (T, String) {
        let _capture_lock = cyril_core::test_support::tracing_capture_lock();
        let capture = cyril_core::test_support::CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(capture.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let logs = String::from_utf8(capture.captured()).expect("utf8 logs");
        (result, logs)
    }

    #[test]
    fn workflow_notification_branches_before_other_consumers() {
        let mut app = test_app();
        // A main session must exist for a main-scoped frame to route to the
        // old consumers; without the workflow branch it would reach both.
        let main = SessionId::new("sess_main");
        app.handle_notification(session_created_frame(&main));

        let baseline_workflow = app.workflow_apply_calls;
        let baseline_subagent = app.subagent_ui_apply_calls;
        let baseline_session = app.session_apply_calls;
        let baseline_ui = app.ui_apply_calls;

        // Global frame: no session scope involved, still tracker-only.
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-branch",
        )));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 1);
        assert_eq!(app.subagent_ui_apply_calls, baseline_subagent);
        assert_eq!(app.session_apply_calls, baseline_session);
        assert_eq!(app.ui_apply_calls, baseline_ui);
        assert!(
            app.workflow_tracker
                .get(&workflow_id("wf-branch"))
                .is_some(),
            "the frame must reach the tracker"
        );

        // Main-scoped frame: without the branch this lands in BOTH old
        // consumers.
        app.handle_notification(RoutedNotification::scoped(
            main.clone(),
            workflow_node_started_frame("wf-branch", "step-a"),
        ));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 2);
        assert_eq!(app.subagent_ui_apply_calls, baseline_subagent);
        assert_eq!(app.session_apply_calls, baseline_session);
        assert_eq!(app.ui_apply_calls, baseline_ui);

        // Foreign-scoped frame: without the branch this lands in the
        // subagent stream.
        app.handle_notification(RoutedNotification::scoped(
            SessionId::new("sess_foreign"),
            workflow_node_started_frame("wf-branch", "step-b"),
        ));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 3);
        assert_eq!(app.subagent_ui_apply_calls, baseline_subagent);
        assert_eq!(app.session_apply_calls, baseline_session);
        assert_eq!(app.ui_apply_calls, baseline_ui);

        // An unrelated frame still reaches the old consumers, proving the
        // counters sit at the real call sites and routing is unchanged.
        app.handle_notification(RoutedNotification::global(Notification::ModeChanged {
            mode_id: ModeId::new("myagent"),
        }));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 3);
        assert_eq!(app.session_apply_calls, baseline_session + 1);
        assert_eq!(app.ui_apply_calls, baseline_ui + 1);
    }

    #[test]
    fn workflow_notification_is_consumed_exactly_once() {
        let mut app = test_app();
        let baseline_workflow = app.workflow_apply_calls;
        let baseline_session = app.session_apply_calls;
        let baseline_ui = app.ui_apply_calls;

        // The same frame delivered twice: each delivery is consumed exactly
        // once (one tracker apply per frame, nothing forwarded).
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-once",
        )));
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-once",
        )));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 2);
        assert_eq!(app.session_apply_calls, baseline_session);
        assert_eq!(app.ui_apply_calls, baseline_ui);

        // The duplicate opening did not double-apply: an identical replay is
        // an idempotent no-op, not a second state.
        let id = workflow_id("wf-once");
        let run = app
            .workflow_tracker
            .get(&id)
            .expect("opening applied exactly once");
        assert_eq!(run.workflow_name(), "recipe-wf-once");
        assert_eq!(app.workflow_tracker.iter().count(), 1);

        // A node opening lands exactly once per frame: one apply, one node.
        app.handle_notification(RoutedNotification::global(workflow_node_started_frame(
            "wf-once", "step",
        )));
        app.handle_notification(RoutedNotification::global(workflow_node_started_frame(
            "wf-once", "step",
        )));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 4);
        let run = app
            .workflow_tracker
            .get(&id)
            .expect("run survives replayed openings");
        assert_eq!(
            run.nodes().count(),
            1,
            "the node must be applied once, not duplicated"
        );
    }

    #[test]
    fn workflow_state_error_isolated_by_app() {
        let mut app = test_app();
        let id = workflow_id("wf-error");
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-error",
        )));
        let before = app.workflow_tracker.get(&id).cloned();

        let ((), logs) = with_captured_logs(|| {
            app.handle_notification(RoutedNotification::global(duplicate_path_completion_frame(
                "wf-error",
            )));
        });

        // One structured WARN with the manifest schema: message plus
        // workflow_id, event_kind, error_kind, error.
        assert_eq!(
            logs.matches("workflow state application failed").count(),
            1,
            "exactly one WARN per failed frame, got:\n{logs}"
        );
        assert!(logs.contains("workflow_id=wf-error"), "got:\n{logs}");
        assert!(logs.contains("event_kind=\"run_complete\""), "got:\n{logs}");
        assert!(
            logs.contains("error_kind=\"duplicate_canonical_path\""),
            "got:\n{logs}"
        );
        assert!(logs.contains("error="), "got:\n{logs}");

        // The failed frame left the tracked run byte-for-byte unchanged.
        assert_eq!(
            app.workflow_tracker.get(&id).cloned(),
            before,
            "state must be atomic across a rejected frame"
        );

        // The App keeps dispatching: a successor applies normally and the
        // failure never leaked into the old consumers.
        let baseline_workflow = app.workflow_apply_calls;
        app.handle_notification(RoutedNotification::global(workflow_node_started_frame(
            "wf-error", "step",
        )));
        assert_eq!(app.workflow_apply_calls, baseline_workflow + 1);
        let run = app
            .workflow_tracker
            .get(&id)
            .expect("run survives the isolated error");
        assert_eq!(run.nodes().count(), 1);
        assert_eq!(app.session_apply_calls, 0);
        assert_eq!(app.ui_apply_calls, 0);
    }

    // ── cyril-jxfu slices 4/5: workflow routing + the late-claim sweep ──────

    fn workflow_node_claim_frame(id: &str, node: &str, sid: &SessionId) -> Notification {
        let path =
            WorkflowNodePath::try_new(&workflow_id(id), vec![id.to_owned(), node.to_owned()])
                .expect("canonical node path fixture");
        Notification::Workflow(Box::new(WorkflowEvent::NodeStarted(
            WorkflowNodeStarted::new(
                workflow_id(id),
                workflow_node_id(node),
                path,
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new().with_session_id(sid.clone()),
            ),
        )))
    }

    fn agent_text_frame(sid: &SessionId, text: &str, is_streaming: bool) -> RoutedNotification {
        RoutedNotification::scoped(
            sid.clone(),
            Notification::AgentMessage(AgentMessage {
                text: text.into(),
                is_streaming,
            }),
        )
    }

    /// A minimal one-node fetched-run snapshot, as the bridge produces for
    /// `/workflow attach` / `status <id>` (cyril-0qe6).
    fn workflow_snapshot_frame(id: &str, status: WorkflowRunStatus) -> Notification {
        Notification::WorkflowSnapshot(Box::new(WorkflowSnapshot::new(
            workflow_id(id),
            format!("recipe-{id}"),
            status,
            WorkflowSnapshotData::new(
                serde_json::json!({}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(workflow_node_id("root"), Vec::new()),
                WorkflowNodeStatus::Completed,
                Vec::new(),
            ),
            WorkflowSnapshotMetadata::new("2026-08-13T00:00:00Z".to_owned(), 0),
        )))
    }

    /// cyril-0qe6 C4: an attach snapshot seeds the tracker exactly once and
    /// is never forwarded to SessionController or UiState.
    #[test]
    fn workflow_snapshot_seeds_tracker_and_is_not_forwarded() {
        let mut app = test_app();
        let baseline_session = app.session_apply_calls;
        let baseline_ui = app.ui_apply_calls;

        app.handle_notification(RoutedNotification::global(workflow_snapshot_frame(
            "wf-seeded",
            WorkflowRunStatus::Completed,
        )));

        let run = app
            .workflow_tracker
            .get(&workflow_id("wf-seeded"))
            .expect("the snapshot must seed the tracker");
        assert_eq!(run.status(), Some(WorkflowRunStatus::Completed));
        assert_eq!(app.session_apply_calls, baseline_session);
        assert_eq!(app.ui_apply_calls, baseline_ui);
    }

    /// cyril-0qe6 C5: a snapshot conflicting with a terminal run is rejected
    /// without state change — warning-only, never a silent overwrite.
    #[test]
    fn workflow_snapshot_terminal_conflict_changes_nothing() {
        let mut app = test_app();
        app.handle_notification(RoutedNotification::global(workflow_snapshot_frame(
            "wf-conflict",
            WorkflowRunStatus::Completed,
        )));

        // A different terminal status for the same run: apply_snapshot's
        // terminal-conflict guard must refuse it.
        app.handle_notification(RoutedNotification::global(workflow_snapshot_frame(
            "wf-conflict",
            WorkflowRunStatus::Failed,
        )));

        let run = app
            .workflow_tracker
            .get(&workflow_id("wf-conflict"))
            .expect("the run survives the rejected snapshot");
        assert_eq!(
            run.status(),
            Some(WorkflowRunStatus::Completed),
            "the terminal status must be unchanged by the conflicting snapshot"
        );
    }

    /// The display half rides normal routing: a WorkflowCommand outcome
    /// reaches the ordinary consumers and never the tracker.
    #[test]
    fn workflow_command_outcome_rides_normal_routing() {
        let mut app = test_app();
        let baseline_workflow = app.workflow_apply_calls;
        let baseline_session = app.session_apply_calls;
        let baseline_ui = app.ui_apply_calls;

        app.handle_notification(RoutedNotification::global(Notification::WorkflowCommand(
            cyril_core::types::WorkflowCommandOutcome::Failed {
                operation: "workflow list".to_owned(),
                code: Some(-32603),
                details: "details".to_owned(),
            },
        )));

        assert_eq!(app.workflow_apply_calls, baseline_workflow);
        assert_eq!(app.session_apply_calls, baseline_session + 1);
        assert_eq!(app.ui_apply_calls, baseline_ui + 1);
    }

    /// No subagent stream may keep a workflow-owned key after a sweep — the
    /// C5 invariant, asserted directly.
    fn assert_no_owned_subagent_stream(app: &App) {
        let leaked: Vec<&SessionId> = app
            .ui_state
            .subagent_ui()
            .streams()
            .keys()
            .filter(|sid| app.workflow_tracker.session_owner(sid).is_some())
            .collect();
        assert!(
            leaked.is_empty(),
            "workflow-owned ids stranded in the subagent store: {leaked:?}"
        );
    }

    // C5, capture-shaped ordering: frames first (optimistic subagent stream),
    // claim second (re-parent with history), more frames third (workflow
    // store, not a re-created subagent stream). C8 rides the same fixture:
    // the optimistic stream is focused when the claim lands.
    #[test]
    fn late_claim_reparents_optimistic_stream_with_history() {
        let mut app = test_app();
        let main = SessionId::new("sess_main");
        let step = SessionId::new("sess_step");
        app.handle_notification(session_created_frame(&main));

        app.handle_notification(agent_text_frame(&step, "pre-claim one", false));
        app.handle_notification(agent_text_frame(&step, "pre-claim two", false));
        assert_eq!(
            app.ui_state.subagent_ui().streams()[&step].messages().len(),
            2,
            "pre-claim frames must land in the optimistic subagent stream"
        );
        assert!(app.ui_state.focus_subagent(step.clone()));

        app.handle_notification(RoutedNotification::global(workflow_run_started_frame("wf")));
        app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
            "wf", "alpha", &step,
        )));

        assert!(
            !app.ui_state.subagent_ui().streams().contains_key(&step),
            "the claim must move the stream out of the subagent store"
        );
        assert!(
            app.ui_state.subagent_ui().focused_session_id().is_none(),
            "C8: re-parenting the focused stream must clear drill-in focus"
        );
        assert_eq!(
            app.ui_state.workflow_streams()[&step].messages().len(),
            2,
            "adopted history must arrive intact"
        );
        assert_no_owned_subagent_stream(&app);

        app.handle_notification(agent_text_frame(&step, "post-claim", false));
        assert_eq!(
            app.ui_state.workflow_streams()[&step].messages().len(),
            3,
            "post-claim frames must append to the workflow stream"
        );
        assert!(
            !app.ui_state.subagent_ui().streams().contains_key(&step),
            "no subagent stream may be re-created for a claimed id"
        );
        assert_eq!(app.workflow_stream_apply_calls, 1);
        assert_no_owned_subagent_stream(&app);
    }

    // C5, fresh-create path: a claim with no prior stream creates nothing;
    // the first post-claim frame creates the workflow stream directly and
    // the subagent store never hears about the id.
    #[test]
    fn claim_before_any_frame_routes_directly_to_workflow() {
        let mut app = test_app();
        let main = SessionId::new("sess_main");
        let step = SessionId::new("sess_step");
        app.handle_notification(session_created_frame(&main));
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame("wf")));
        app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
            "wf", "alpha", &step,
        )));
        assert!(
            app.ui_state.workflow_streams().is_empty(),
            "a claim alone must not conjure a stream"
        );

        app.handle_notification(agent_text_frame(&step, "first frame", false));
        assert_eq!(app.ui_state.workflow_streams()[&step].messages().len(), 1);
        assert!(!app.ui_state.subagent_ui().streams().contains_key(&step));
        assert_eq!(app.subagent_ui_apply_calls, 0);
        assert_eq!(app.workflow_stream_apply_calls, 1);
    }

    // C5, duplicate claim: an Ok(false) application leaves the invariant
    // already clean — asserted on state, not on sweep mechanics.
    #[test]
    fn duplicate_claim_keeps_invariant_clean() {
        let mut app = test_app();
        let main = SessionId::new("sess_main");
        let step = SessionId::new("sess_step");
        app.handle_notification(session_created_frame(&main));
        app.handle_notification(agent_text_frame(&step, "pre-claim", false));
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame("wf")));
        for _ in 0..2 {
            app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
                "wf", "alpha", &step,
            )));
            assert_no_owned_subagent_stream(&app);
        }
        assert_eq!(app.ui_state.workflow_streams()[&step].messages().len(), 1);
    }

    // C6 / AC1 — THE capture replay fence: the real
    // kas-custom-dag-2.16.0.jsonl through the real conversion path
    // (`test_support::kas_capture_to_routed` wraps the same KasEngine
    // converters the live bridge uses), into the real App dispatch. Expected
    // constants pre-registered in .cyril-jxfu/plan.md slice 6; independent
    // oracle: probe 1 + oracle.sh (.cyril-jxfu/, text-only pipeline). If a
    // constant disagrees, investigate against the probe's line-level data —
    // do not re-pin.
    #[cfg(feature = "kas")]
    #[test]
    fn capture_replay_attributes_every_forwarded_frame() {
        const CAPTURE: &str = include_str!(
            "../../cyril-core/tests/fixtures/kas/workflow/kas-custom-dag-2.16.0.jsonl"
        );
        const MAIN: &str = "sess_2bc0cfdc-ccba-47b7-a3ab-224b23a63d60";
        const ALPHA: &str = "sess_a3d8bb37-4b02-494a-8e82-1dbbc0877fb6";
        const BETA: &str = "sess_fd35dac1-b00e-4d16-b0ae-466cd68523d9";

        let mut app = test_app();
        // Production learns main from its own session/new response; the
        // capture's response frames answer the RECORDER's requests and are
        // skipped by the helper, so feed the equivalent SessionCreated first
        // (the capture names main in its line-9 response; probe 1).
        app.handle_notification(session_created_frame(&SessionId::new(MAIN)));

        for (scope, notification) in cyril_core::test_support::kas_capture_to_routed(CAPTURE) {
            app.handle_notification(match scope {
                Some(sid) => RoutedNotification::scoped(sid, notification),
                None => RoutedNotification::global(notification),
            });
        }

        // Exactly the two step sessions hold workflow streams, each with ONE
        // committed message: its completed "Send Message" tool call (capture
        // lines 59/63 and 64/68; the bootstrap frames are ignored kinds).
        assert_eq!(
            app.ui_state.workflow_streams().len(),
            2,
            "exactly the two step sessions get workflow streams"
        );
        for (sid, node) in [(ALPHA, "alpha"), (BETA, "beta")] {
            let stream = &app.ui_state.workflow_streams()[&SessionId::new(sid)];
            assert_eq!(
                stream.messages().len(),
                1,
                "step {node}: one committed message — its Send Message tool call"
            );
            let cyril_ui::traits::ChatMessageKind::ToolCall(tracked) = &stream.messages()[0].kind
            else {
                panic!("step {node}: the committed message must be the tool call");
            };
            assert_eq!(tracked.title(), "Send Message");
            assert_eq!(
                tracked.status(),
                cyril_core::types::ToolCallStatus::Completed
            );
        }

        // No step frame reaches the subagent domain at end state: the
        // optimistic pre-claim streams were re-parented at the claims
        // (capture lines 46/48), and nothing re-created them.
        assert!(
            app.ui_state.subagent_ui().streams().is_empty(),
            "no subagent stream may survive the claims"
        );
        for sid in [MAIN, ALPHA, BETA] {
            assert!(
                !app.ui_state
                    .subagent_tracker()
                    .is_subagent(&SessionId::new(sid)),
                "the tracker must never learn any of the capture's sessions"
            );
        }
        assert_no_owned_subagent_stream(&app);

        // Both halves of the late-claim path were genuinely exercised:
        // pre-claim bootstrap frames landed optimistically in the subagent
        // store (probe 1: updates begin at lines 33/39, claims land at
        // 46/48), and post-claim frames landed in the workflow store. A
        // conversion regression that stopped forwarding bootstrap kinds
        // would zero the first counter and silently flip this fence to the
        // fresh-create path.
        assert!(
            app.subagent_ui_apply_calls > 0,
            "the capture must exercise the optimistic pre-claim landing"
        );
        assert!(
            app.workflow_stream_apply_calls > 0,
            "the capture must exercise post-claim workflow routing"
        );
        // The main session's own frames reached the main pipeline.
        assert!(app.session_apply_calls > 0);
        assert!(app.ui_apply_calls > 0);
    }

    // C7 with its adversarial counterpart: a streaming workflow step holds
    // the fast tick while main idles; a settled one releases it.
    #[test]
    fn workflow_stream_activity_holds_fast_tick() {
        let mut app = test_app();
        let main = SessionId::new("sess_main");
        let step = SessionId::new("sess_step");
        app.handle_notification(session_created_frame(&main));
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame("wf")));
        app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
            "wf", "alpha", &step,
        )));

        app.handle_notification(agent_text_frame(&step, "streaming…", true));
        assert_eq!(
            app.effective_activity(),
            Activity::Streaming,
            "C7: a streaming workflow step must hold the fast tick"
        );

        app.handle_notification(agent_text_frame(&step, " done", false));
        assert_ne!(
            app.effective_activity(),
            Activity::Streaming,
            "a settled workflow step must release the fast tick"
        );
    }

    /// cyril-nanu C1 — no event-loop path computes a snapshot.
    ///
    /// **Fence shape deviates from the plan, deliberately.** The plan proposed
    /// blocking a snapshot on a barrier and asserting the loop iteration still
    /// completed. That is unreachable by construction now: the App holds no
    /// snapshot source to block, only a channel sender. The plan's alternative,
    /// a wall-clock bound on the trigger path, would have put a stopwatch back
    /// into the ordinary suite — the exact flake this repo removed from CI
    /// twice this week. So the claim is fenced two ways, both deterministic:
    /// behaviorally, the trigger sends a request and marks the panel; and
    /// structurally, the one call this claim forbids is absent from the file.
    /// The structural half is statement-precise (an exact call expression), not
    /// a substring heuristic.
    #[test]
    fn usage_refresh_does_not_block_the_event_loop() {
        let source = include_str!("app.rs");
        // Built by concatenation on purpose: spelled literally, this needle
        // would match its own assertion and the check could never pass. The
        // same self-match bit `no_percentile_computation.rs` on its first
        // attempt.
        let forbidden = format!("self.usage_log.{}()", "snapshot");
        assert!(
            !source.contains(&forbidden),
            "no event-loop path may compute a snapshot inline; that call is what \
             froze the terminal for ~700ms per refresh at 100,000 rows"
        );
        // Positive control: the forbidden call is spelled the way the assertion
        // looks for it, so the check cannot pass merely by looking for
        // something that never existed.
        assert!(
            source.contains(&format!("self.usage_log.{}(", "append")),
            "control: `self.usage_log.<method>()` really is how this file calls \
             the usage log, so the absence above is meaningful"
        );
    }

    /// cyril-nanu C5 — a completed snapshot applies only while a panel is open.
    ///
    /// The `has_usage_panel()` guard runs when the request is SENT; the
    /// operator can close the panel before the result lands. A result applied
    /// to a closed panel would reopen it behind their back.
    #[tokio::test]
    async fn snapshot_result_applies_only_while_a_panel_is_open() {
        let mut app = app_for_tests();
        app.ui_state.open_usage_panel();
        assert!(app.ui_state.has_usage_panel(), "panel opens");

        let mut ready = cyril_core::types::UsageSnapshot::default();
        ready.overview.requests = 7;
        app.handle_usage_snapshot(UsageSnapshotResult::Ready(Box::new(ready.clone())));
        assert_eq!(
            app.ui_state
                .usage_panel()
                .map(|panel| panel.snapshot.overview.requests),
            Some(7),
            "an open panel takes the result"
        );

        // Closed: the result must change nothing and must not reopen the panel.
        app.ui_state.hide_usage_panel();
        let mut later = cyril_core::types::UsageSnapshot::default();
        later.overview.requests = 99;
        app.handle_usage_snapshot(UsageSnapshotResult::Ready(Box::new(later.clone())));
        assert!(
            !app.ui_state.has_usage_panel(),
            "a result must not resurrect a panel the operator closed"
        );

        // Closed then reopened: a later result is welcome again.
        app.ui_state.open_usage_panel();
        app.handle_usage_snapshot(UsageSnapshotResult::Ready(Box::new(later)));
        assert_eq!(
            app.ui_state
                .usage_panel()
                .map(|panel| panel.snapshot.overview.requests),
            Some(99),
            "a reopened panel takes the newer result"
        );
    }

    /// cyril-nanu C9 — an unavailable worker is stated, not waited on.
    ///
    /// A dropped request that reported success would leave the panel in its
    /// computing state forever, which is worse than the stall being fixed.
    #[tokio::test]
    async fn snapshot_worker_unavailable_surfaces_as_failure_status() {
        let mut app = app_for_tests_without_snapshot_worker();
        app.ui_state.open_usage_panel();
        // Positive control: the panel really did open, so the status assertion
        // below is about the status and not about an absent panel.
        assert!(app.ui_state.has_usage_panel(), "panel opens");

        app.request_usage_snapshot();
        assert!(
            matches!(
                app.ui_state.usage_panel().map(|panel| &panel.refresh),
                Some(cyril_ui::traits::UsageRefreshStatus::Failed(_))
            ),
            "a worker that cannot be reached must surface as the panel's failure \
             status rather than a permanent computing state"
        );
    }
}
