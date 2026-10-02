//! `/review` (cyril-iowg): the consent form, the launch chain and the armed
//! run's permission policy.
//!
//! The chain is `/review` → form (probe) → Enter → prepare (install, run
//! directory, gather) → `workflow/new` → `Minted` → arm + `run.json` →
//! `workflow/invoke`. Every blocking step — git probes, file writes, the
//! policy's path checks, reading findings — runs on `spawn_blocking` and
//! reports back through [`ReviewState::rx`]; the event loop only routes.

mod resume;

use super::App;
use cyril_core::review::authorization::{AuthorizationState, RunAuthorization};
use cyril_core::review::config::scope_problem;
use cyril_core::review::config::{CheckCommand, ReviewConfig};
use cyril_core::review::consent::PermissionConsent;
use cyril_core::review::launch::{self, LaunchError, LaunchRequest, Prepared, ReadyRun};
use cyril_core::review::policy::{DENIED_LOG, Decision, PolicyScope, decide, input_problem};
use cyril_core::review::run_record::{RunRecord, RunRecordError};
use cyril_core::review::scope;
use cyril_core::review::summary::{RunEnding, summary};
use cyril_core::review::target::{ReviewArgs, ReviewTarget, TargetArg};
use cyril_core::review::{CrtoolPrefix, ShellDialect};
use cyril_core::types::{
    BridgeCommand, PermissionOptionKind, PermissionRequest, PermissionResponse, SessionId,
    WorkflowCommandOutcome, WorkflowCompletionStatus, WorkflowId, WorkflowOp, WorkflowRunStatus,
    WorkflowRunTarget,
};
use cyril_review::{CheckResult, FindingsError, ReviewError, ReviewRun, read_findings, run_check};
use cyril_ui::traits::{ReviewCheck, ReviewField, ReviewForm, TuiState};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// KAS loads agent files from a watcher (300 ms debounce, 1 s rechecks for a
/// directory it saw appear), so a freshly installed agent is not registered
/// at once. Wait this long before creating the workflow, and double it per
/// retry while KAS still reports a review agent as unregistered.
const AGENT_LOAD_DELAY: Duration = Duration::from_secs(2);
const AGENT_LOAD_RETRIES: u32 = 3;
const RECOUNT_DELAY: Duration = Duration::from_millis(150);
const DIRTY_NOTE: &str = "uncommitted changes in scope are not part of this diff, but the reviewers read the working tree";
/// KAS's `WorkflowAgentNotFoundError` text (2.26.0): "Workflow references
/// custom agent '<name>' which is not registered."
const AGENT_NOT_REGISTERED: &str = "which is not registered";

pub(super) struct ReviewState {
    /// The crtool spelling for the session's host shell, or why there is none.
    prefix: Result<CrtoolPrefix, String>,
    /// Where the assets install: Node's home directory.
    home: Option<PathBuf>,
    agent_load_delay: Duration,
    /// How long the form waits after a change before counting, so holding
    /// an arrow key does not start a git process per step.
    recount_delay: Duration,
    launch: Option<Launch>,
    /// A `/review resume` in progress (its id comes from the same counter).
    resume: Option<resume::Resume>,
    /// `/review cancel` came while `workflow/new` was out: the run it creates
    /// is cancelled as soon as it appears, never left runnable.
    cancel_on_mint: bool,
    /// The review run a `/review cancel` is cancelling, until the agent
    /// answers; only then is success (or failure) reported.
    cancelling: Option<WorkflowId>,
    /// The id the next launch gets.
    next_launch: u64,
    /// Every run this process armed, live or ended. An ended run is kept so
    /// its late requests are denied and logged rather than falling through
    /// to an ordinary prompt — even after a newer review has started.
    runs: HashMap<WorkflowId, ArmedRun>,
    tx: mpsc::UnboundedSender<ReviewTask>,
    pub(super) rx: mpsc::UnboundedReceiver<ReviewTask>,
}

/// A launch between `/review` and a started run.
struct Launch {
    /// Which launch this is: results of an abandoned launch carry an older
    /// id and are dropped instead of driving a newer one.
    id: u64,
    plan: Plan,
    stage: Stage,
}

/// What the review is of. The target follows the form until Enter; the plan,
/// not the form, is what the launch uses.
struct Plan {
    target: ReviewTarget,
    /// Bumped on every target change: only the latest count is applied.
    recount: u64,
    /// The checked pathspecs: what the probe, gather, the workflow and
    /// `run.json` receive.
    scope: Vec<String>,
    /// What the current target's diff touches, for counting without git.
    touched: launch::Touched,
    /// The paths `[review] scope` or `/review -- <paths>` preselected.
    preset: Option<Vec<String>>,
    /// Why the current target cannot be reviewed (a git error, a name the
    /// recipe would refuse); Enter is refused while set.
    target_problem: Option<String>,
    /// The `[review]` settings the form showed: what consent covers.
    config: ReviewConfig,
}

enum Stage {
    /// The form is open, reading `[review]` and counting the diff.
    Opening,
    /// The form is open.
    Form,
    /// Enter was pressed; the blocking launch steps are running.
    Preparing,
    /// Waiting for KAS to load the review agents before `workflow/new`.
    Loading {
        ready: ReadyRun,
        retries: u32,
    },
    /// The check command is running. Held only to be dropped: abandoning
    /// the launch drops it, which stops the command.
    Checking {
        ready: ReadyRun,
        _cancel: oneshot::Sender<()>,
    },
    Minting {
        ready: ReadyRun,
        retries: u32,
    },
    Recording(WorkflowId),
    Invoking(WorkflowId),
}

struct ArmedRun {
    authorization: RunAuthorization,
    run_dir: PathBuf,
    /// Every session the tracker has attributed to this run. A final
    /// snapshot may drop a node's session, and a late request from it must
    /// still be denied rather than prompted.
    sessions: HashSet<SessionId>,
}

/// What the form opens with.
pub(super) struct Opened {
    config: ReviewConfig,
    target: ReviewTarget,
    target_problem: Option<String>,
    touched: launch::Touched,
    branches: Vec<String>,
    branch_note: Option<String>,
}

/// A step of one launch, tagged with that launch's id.
pub(super) enum LaunchStep {
    Opened(Result<Opened, LaunchError>),
    /// The form settled on a new target: count it if still current.
    RecountDue {
        recount: u64,
    },
    /// The count (and whether scope has uncommitted changes) for recount
    /// number `recount`.
    Recounted {
        recount: u64,
        files: Result<launch::Touched, LaunchError>,
    },
    Prepared(Result<Prepared, LaunchError>),
    Checked(Result<CheckResult, ReviewError>),
    /// The agent-load wait is over: send `workflow/new`.
    SendNew,
    /// A step of a `/review resume`.
    Resume(resume::ResumeStep),
}

/// A result from off-loop work, back on the event loop.
pub(super) enum ReviewTask {
    Launch {
        launch: u64,
        step: LaunchStep,
    },
    Recorded {
        workflow_id: WorkflowId,
        result: Result<(), RunRecordError>,
    },
    Decided {
        workflow_id: WorkflowId,
        decision: Decision,
    },
    Finished(String),
    /// Send `workflow/cancel` for a review run `/review cancel` stopped.
    SendCancel(WorkflowId),
    /// Off-loop work panicked. A launch's work abandons that launch.
    Crashed {
        launch: Option<u64>,
        error: String,
    },
}

impl ReviewState {
    pub(super) fn new(shell: Option<ShellDialect>) -> Self {
        let prefix = match shell {
            Some(dialect) => CrtoolPrefix::current(dialect).map_err(|error| error.to_string()),
            None => Err("it needs the KAS engine and its host shell".to_owned()),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            prefix,
            home: launch::node_home(),
            agent_load_delay: AGENT_LOAD_DELAY,
            recount_delay: RECOUNT_DELAY,
            launch: None,
            resume: None,
            cancel_on_mint: false,
            cancelling: None,
            next_launch: 0,
            runs: HashMap::new(),
            tx,
            rx,
        }
    }

    #[cfg(test)]
    pub(super) fn set_environment(&mut self, prefix: CrtoolPrefix, home: PathBuf) {
        self.prefix = Ok(prefix);
        self.home = Some(home);
        self.agent_load_delay = Duration::from_millis(10);
        self.recount_delay = Duration::from_millis(1);
    }

    #[cfg(test)]
    pub(super) fn current_launch(&self) -> Option<u64> {
        self.launch.as_ref().map(|launch| launch.id)
    }

    /// Let any pending agent-load wait fire.
    #[cfg(test)]
    pub(super) async fn flush_delays_for_tests(&self) {
        tokio::time::sleep(self.agent_load_delay * 4).await;
    }

    /// Deliver `task` on `rx` after `delay`.
    fn after(&self, delay: Duration, task: ReviewTask) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            if tx.send(task).is_err() {
                tracing::debug!("review result dropped: the app is gone");
            }
        });
    }

    /// Run blocking `work` off the event loop; its result, if any, comes back
    /// on `rx`. A panic comes back as [`ReviewTask::Crashed`], so nothing
    /// waits on a result that will never arrive.
    pub(super) fn spawn(&self, work: impl FnOnce() -> Option<ReviewTask> + Send + 'static) {
        self.report(None, tokio::task::spawn_blocking(work));
    }

    /// Run one blocking step of launch `launch`.
    pub(super) fn spawn_step(
        &self,
        launch: u64,
        work: impl FnOnce() -> LaunchStep + Send + 'static,
    ) {
        self.report(
            Some(launch),
            tokio::task::spawn_blocking(move || {
                Some(ReviewTask::Launch {
                    launch,
                    step: work(),
                })
            }),
        );
    }

    /// Run an async step of launch `launch` as its own task.
    fn spawn_async_step(
        &self,
        launch: u64,
        work: impl Future<Output = LaunchStep> + Send + 'static,
    ) {
        self.report(
            Some(launch),
            tokio::spawn(async move {
                Some(ReviewTask::Launch {
                    launch,
                    step: work.await,
                })
            }),
        );
    }

    fn report(&self, launch: Option<u64>, work: tokio::task::JoinHandle<Option<ReviewTask>>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let task = match work.await {
                Ok(task) => task,
                Err(error) => Some(ReviewTask::Crashed {
                    launch,
                    error: error.to_string(),
                }),
            };
            if let Some(task) = task
                && tx.send(task).is_err()
            {
                tracing::debug!("review result dropped: the app is gone");
            }
        });
    }

    /// The armed or paused run, if any: one review at a time.
    fn live_run(&self) -> Option<&ArmedRun> {
        self.runs.values().find(|run| run.authorization.is_live())
    }

    fn run_for(&mut self, workflow_id: &WorkflowId) -> Option<&mut ArmedRun> {
        self.runs.get_mut(workflow_id)
    }

    /// Disarm every live run; each keeps denying late requests.
    pub(super) fn disarm(&mut self, why: &str) {
        for run in self
            .runs
            .values_mut()
            .filter(|run| run.authorization.is_live())
        {
            tracing::info!(workflow_id = %run.authorization.workflow_id(), why, "review: disarmed");
            run.authorization.disarm();
        }
    }
}

impl App {
    /// `/review`: open the consent form, then read the repository's
    /// `[review]` settings and count the diff off-loop.
    pub(super) fn open_review(&mut self, args: ReviewArgs) {
        if let Some(run) = self.review.live_run() {
            let id = run.authorization.workflow_id();
            let text = if run.authorization.state() == AuthorizationState::Paused {
                format!("review: {id} is paused — /workflow resume {id} continues it")
            } else {
                format!("review: {id} is still running — /workflow status {id}")
            };
            self.ui_state.add_system_message(text);
            return;
        }
        if self.review.launch.is_some() || self.review.resume.is_some() {
            self.ui_state
                .add_system_message("review: a review is already starting".to_owned());
            return;
        }
        if let Err(problem) = &self.review.prefix {
            let text = format!("review: unavailable — {problem}");
            self.ui_state.add_system_message(text);
            return;
        }
        if let Some(problem) = args
            .paths
            .iter()
            .flatten()
            .find_map(|path| scope_problem(path))
        {
            self.ui_state
                .add_system_message(format!("review: scope {problem}"));
            return;
        }
        // The field the arguments left open gets the focus.
        let focus = match &args.target {
            None => ReviewField::Target,
            Some(TargetArg::Base(None)) => ReviewField::Base,
            Some(_) if args.paths.is_none() => ReviewField::Paths,
            Some(_) => ReviewField::Target,
        };
        let shown = match &args.target {
            None | Some(TargetArg::Auto) => ReviewTarget::Auto,
            Some(TargetArg::Uncommitted) => ReviewTarget::Uncommitted,
            Some(TargetArg::HeadCommit) => ReviewTarget::HeadCommit,
            Some(TargetArg::Base(Some(base))) => ReviewTarget::Base(base.clone()),
            // The branch is picked once the branch list is read.
            Some(TargetArg::Base(None)) => ReviewTarget::Auto,
        };
        let id = self.review.next_launch;
        self.review.next_launch += 1;
        self.review.launch = Some(Launch {
            id,
            plan: Plan {
                target: shown.clone(),
                recount: 0,
                scope: Vec::new(),
                touched: launch::Touched::default(),
                preset: args.paths.clone(),
                target_problem: None,
                config: ReviewConfig::default(),
            },
            stage: Stage::Opening,
        });
        self.ui_state.show_review_form(ReviewForm {
            focus,
            ..ReviewForm::opening(shown)
        });
        let workspace = self.cwd.clone();
        self.review.spawn_step(id, move || {
            LaunchStep::Opened(open_launch(&workspace, args.target))
        });
    }

    /// Esc backs out: before Enter with no side effects, after it by
    /// abandoning the launch (whatever the launch steps already wrote stays).
    /// Enter confirms once the count is known; every other key is consumed so
    /// nothing reaches the chat behind the form.
    pub(super) fn handle_review_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        if self.review.resume.is_some() {
            self.handle_resume_key(key);
            return;
        }
        let Some(form) = self.ui_state.review_form_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc if form.busy => self.abandon_launch(
                "review: abandoned — anything already written stays under .code-review/".to_owned(),
            ),
            KeyCode::Esc => {
                self.ui_state.close_review_form();
                self.review.launch = None;
            }
            _ if form.busy => {}
            KeyCode::Enter if form.ready() => self.confirm_review(),
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                let fields = form.fields();
                let here = fields.iter().position(|field| *field == form.focus);
                let back = matches!(key.code, KeyCode::BackTab | KeyCode::Up);
                form.focus = match here {
                    Some(here) if back => fields[(here + fields.len() - 1) % fields.len()],
                    Some(here) => fields[(here + 1) % fields.len()],
                    None => ReviewField::Target,
                };
            }
            KeyCode::Left | KeyCode::Right if form.focus == ReviewField::Paths => {
                let last = form.paths.len().saturating_sub(1);
                form.path_cursor = match key.code {
                    KeyCode::Left => form.path_cursor.saturating_sub(1),
                    _ => (form.path_cursor + 1).min(last),
                };
            }
            KeyCode::Char(' ') if form.focus == ReviewField::Paths => {
                if let Some(choice) = form.paths.get_mut(form.path_cursor) {
                    choice.checked = !choice.checked;
                }
                self.refresh_scope();
            }
            KeyCode::Left | KeyCode::Right => {
                let back = key.code == KeyCode::Left;
                let target = match form.focus {
                    ReviewField::Target => {
                        form.target
                            .cycle(&form.branches, form.base_choice.as_deref(), back)
                    }
                    ReviewField::Base => form.target.cycle_base(&form.branches, back),
                    ReviewField::Paths => form.target.clone(),
                };
                if target != form.target {
                    self.choose_target(target);
                }
            }
            _ => {}
        }
    }

    /// The checked paths changed (or the touched files did): the plan's
    /// scope, the count, the problem and the note all follow, without git.
    fn refresh_scope(&mut self) {
        let Some(launch) = self.review.launch.as_mut() else {
            return;
        };
        let Some(form) = self.ui_state.review_form_mut() else {
            return;
        };
        // Nothing touched means nothing to choose: the whole repository, so
        // Enter reports "nothing to review" rather than an empty selection.
        launch.plan.scope = if form.paths.is_empty() {
            vec![".".to_owned()]
        } else {
            scope::selected(&form.paths)
        };
        let touched = &launch.plan.touched;
        form.file_count = Some(scope::count(&form.paths, &touched.files));
        form.problem = launch.plan.target_problem.clone().or_else(|| {
            launch
                .plan
                .scope
                .is_empty()
                .then(|| "select at least one path".to_owned())
        });
        let dirty = touched.uncommitted.iter().any(|file| {
            launch
                .plan
                .scope
                .iter()
                .any(|path| scope::covers(path, file))
        });
        form.note =
            (dirty && launch.plan.target.ends_at_head_commit()).then(|| DIRTY_NOTE.to_owned());
    }

    /// The form's target changed. A target the recipe would refuse is shown
    /// as a problem at once; otherwise the diff is recounted off-loop after a
    /// short pause, and only the latest count is applied.
    fn choose_target(&mut self, target: ReviewTarget) {
        let Some(launch) = self
            .review
            .launch
            .as_mut()
            .filter(|launch| matches!(launch.stage, Stage::Form))
        else {
            return;
        };
        launch.plan.target = target.clone();
        launch.plan.recount += 1;
        let (id, recount) = (launch.id, launch.plan.recount);
        let problem = input_problem("target", &target.spec());
        launch.plan.target_problem = problem.clone();
        if let Some(form) = self.ui_state.review_form_mut() {
            if let ReviewTarget::Base(base) = &target {
                form.base_choice = Some(base.clone());
            }
            form.target = target;
            form.file_count = None;
            form.note = None;
            form.problem = problem.clone();
            if form.focus == ReviewField::Base && !form.shows_base() {
                form.focus = ReviewField::Target;
            }
        }
        if problem.is_none() {
            self.review.after(
                self.review.recount_delay,
                ReviewTask::Launch {
                    launch: id,
                    step: LaunchStep::RecountDue { recount },
                },
            );
        }
    }

    fn confirm_review(&mut self) {
        let Some(launch) = self
            .review
            .launch
            .as_mut()
            .filter(|launch| matches!(launch.stage, Stage::Form))
        else {
            return;
        };
        let Ok(crtool) = self.review.prefix.clone() else {
            return;
        };
        launch.stage = Stage::Preparing;
        let id = launch.id;
        if let Some(form) = self.ui_state.review_form_mut() {
            form.busy = true;
        }
        let request = LaunchRequest {
            workspace: self.cwd.clone(),
            target: launch.plan.target.spec(),
            scope: launch.plan.scope.clone(),
            crtool,
            home: self.review.home.clone(),
            config: launch.plan.config.clone(),
        };
        self.review
            .spawn_step(id, move || LaunchStep::Prepared(launch::prepare(&request)));
    }

    pub(super) async fn handle_review_task(&mut self, task: ReviewTask) {
        self.redraw_needed = true;
        match task {
            ReviewTask::Launch { launch, step } => self.handle_launch_step(launch, step).await,
            ReviewTask::Recorded {
                workflow_id,
                result,
            } => {
                let recording = matches!(
                    &self.review.launch,
                    Some(Launch { stage: Stage::Recording(id), .. }) if *id == workflow_id
                );
                if !recording {
                    return;
                }
                if let Err(error) = result {
                    self.withdraw(
                        &workflow_id,
                        format!("review: {workflow_id} not started — cannot record it: {error}"),
                    );
                    return;
                }
                let Some(session_id) = self.session.id().cloned() else {
                    self.withdraw(
                        &workflow_id,
                        format!("review: {workflow_id} not started — the session is gone"),
                    );
                    return;
                };
                let invoke = self.workflow_command(
                    session_id,
                    WorkflowOp::Invoke {
                        id: workflow_id.clone(),
                    },
                );
                if let Err(error) = self.bridge_sender.send(invoke).await {
                    self.withdraw(
                        &workflow_id,
                        format!("review: {workflow_id} not started — {error}"),
                    );
                    return;
                }
                if let Some(launch) = self.review.launch.as_mut() {
                    launch.stage = Stage::Invoking(workflow_id);
                }
            }
            ReviewTask::Decided {
                workflow_id,
                decision,
            } => {
                if let Some(run) = self.review.run_for(&workflow_id) {
                    run.authorization.record(&decision);
                }
            }
            ReviewTask::Finished(text) => self.ui_state.add_system_message(text),
            ReviewTask::SendCancel(workflow_id) => {
                let Some(session_id) = self.session.id().cloned() else {
                    self.ui_state.add_system_message(format!(
                        "review: cannot cancel {workflow_id} — no active session"
                    ));
                    return;
                };
                let cancel = self.workflow_command(
                    session_id,
                    WorkflowOp::Cancel {
                        id: workflow_id.clone(),
                    },
                );
                match self.bridge_sender.send(cancel).await {
                    Ok(()) => self.review.cancelling = Some(workflow_id),
                    Err(error) => self.ui_state.add_system_message(format!(
                        "review: could not cancel {workflow_id} — {error}"
                    )),
                }
            }
            ReviewTask::Crashed { launch, error } => {
                tracing::error!(%error, "review: off-loop work panicked");
                if launch.is_some()
                    && launch == self.review.launch.as_ref().map(|current| current.id)
                {
                    self.abandon_launch(format!("review: internal error — {error}"));
                }
                if launch.is_some()
                    && launch == self.review.resume.as_ref().map(|current| current.id)
                {
                    self.review.resume = None;
                    self.ui_state.close_review_form();
                    self.ui_state
                        .add_system_message(format!("review resume: internal error — {error}"));
                }
            }
        }
    }

    /// One step of a launch. A step of any launch but the current one is
    /// stale (that launch was abandoned) and is dropped.
    async fn handle_launch_step(&mut self, id: u64, step: LaunchStep) {
        if let LaunchStep::Resume(step) = step {
            if self.review.resume.as_ref().map(|resume| resume.id) == Some(id) {
                self.handle_resume_step(step).await;
            } else {
                tracing::debug!(
                    launch = id,
                    "review: dropping a step of an abandoned resume"
                );
            }
            return;
        }
        if self.review.launch.as_ref().map(|launch| launch.id) != Some(id) {
            tracing::debug!(
                launch = id,
                "review: dropping a step of an abandoned launch"
            );
            return;
        }
        match step {
            // Routed to the resume above.
            LaunchStep::Resume(_) => {}
            LaunchStep::Opened(result) => {
                let Some(launch) = self
                    .review
                    .launch
                    .as_mut()
                    .filter(|launch| matches!(launch.stage, Stage::Opening))
                else {
                    return;
                };
                match result {
                    Ok(Opened {
                        config,
                        target,
                        target_problem,
                        touched,
                        branches,
                        branch_note,
                    }) => {
                        let check = match &config.check {
                            Some(check) => ReviewCheck::WillRun {
                                command: check.command.clone(),
                                timeout_secs: check.timeout.as_secs(),
                            },
                            None => ReviewCheck::NotConfigured,
                        };
                        if launch.plan.preset.is_none() {
                            launch.plan.preset.clone_from(&config.scope);
                        }
                        let paths = scope::choices(&touched.files, launch.plan.preset.as_deref());
                        launch.plan.target = target.clone();
                        launch.plan.touched = touched;
                        launch.plan.target_problem = target_problem;
                        launch.plan.config = config;
                        launch.stage = Stage::Form;
                        if let Some(form) = self.ui_state.review_form_mut() {
                            if let ReviewTarget::Base(base) = &target {
                                form.base_choice = Some(base.clone());
                            }
                            form.target = target;
                            form.paths = paths;
                            form.branches = branches;
                            form.check = check;
                            if !form.fields().contains(&form.focus) {
                                form.focus = ReviewField::Target;
                            }
                        }
                        self.refresh_scope();
                        if let (Some(note), Some(form)) =
                            (branch_note, self.ui_state.review_form_mut())
                            && form.note.is_none()
                        {
                            form.note = Some(note);
                        }
                    }
                    Err(error) => self.abandon_launch(launch_message(&error)),
                }
            }
            LaunchStep::RecountDue { recount } => {
                let Some(launch) = self
                    .review
                    .launch
                    .as_ref()
                    .filter(|launch| launch.plan.recount == recount)
                else {
                    return;
                };
                let workspace = self.cwd.clone();
                let spec = launch.plan.target.spec();
                self.review.spawn_step(id, move || LaunchStep::Recounted {
                    recount,
                    files: launch::touched(&workspace, &spec),
                });
            }
            LaunchStep::Recounted { recount, files } => {
                let Some(launch) = self
                    .review
                    .launch
                    .as_mut()
                    .filter(|launch| launch.plan.recount == recount)
                else {
                    return;
                };
                match files {
                    Ok(touched) => {
                        // The form is open while the launch is in the form stage.
                        let Some(previous) =
                            self.ui_state.review_form().map(|form| form.paths.clone())
                        else {
                            return;
                        };
                        let paths = scope::rebuild(
                            &previous,
                            &touched.files,
                            launch.plan.preset.as_deref(),
                        );
                        launch.plan.touched = touched;
                        launch.plan.target_problem = None;
                        if let Some(form) = self.ui_state.review_form_mut() {
                            form.paths = paths;
                            form.path_cursor =
                                form.path_cursor.min(form.paths.len().saturating_sub(1));
                        }
                        self.refresh_scope();
                    }
                    Err(error) => {
                        launch.plan.target_problem = Some(error.to_string());
                        if let Some(form) = self.ui_state.review_form_mut() {
                            form.problem = Some(error.to_string());
                        }
                    }
                }
            }
            LaunchStep::Prepared(result) => {
                let Some(launch) = self.review.launch.take() else {
                    return;
                };
                match result {
                    Ok(Prepared::Nothing) => {
                        self.ui_state.close_review_form();
                        self.ui_state.add_system_message(format!(
                            "review: nothing to review — {} in {}",
                            launch.plan.target.spec(),
                            launch.plan.scope.join(" ")
                        ));
                    }
                    Err(error) => {
                        self.ui_state.close_review_form();
                        self.ui_state.add_system_message(launch_message(&error));
                    }
                    Ok(Prepared::Ready(ready)) => {
                        self.ui_state.add_system_message(format!(
                            "review: gathered {} file(s) into {}",
                            ready.files,
                            ready.run_dir.display()
                        ));
                        match launch.plan.config.check.clone() {
                            Some(check) => self.start_check(launch.id, launch.plan, ready, check),
                            None => {
                                self.ui_state.close_review_form();
                                self.create_workflow(launch.id, launch.plan, ready).await;
                            }
                        }
                    }
                }
            }
            LaunchStep::Checked(result) => {
                let Some(launch) = self.review.launch.take() else {
                    return;
                };
                let Stage::Checking { ready, .. } = launch.stage else {
                    self.review.launch = Some(launch);
                    return;
                };
                self.ui_state.close_review_form();
                match result {
                    Ok(check) => {
                        let mut text = format!(
                            "review: check {} — recorded for the reviewers",
                            check.outcome().status()
                        );
                        if let Some(problem) = check.cleanup_error() {
                            text.push_str(&format!(" ({problem})"));
                        }
                        self.ui_state.add_system_message(text);
                        self.create_workflow(launch.id, launch.plan, ready).await;
                    }
                    Err(error) => self.ui_state.add_system_message(format!(
                        "review: the check could not run — {error}; nothing was started"
                    )),
                }
            }
            LaunchStep::SendNew => {
                let Some(launch) = self.review.launch.take() else {
                    return;
                };
                match launch.stage {
                    Stage::Loading { ready, retries } => {
                        self.send_new(launch.id, launch.plan, ready, retries).await;
                    }
                    stage => self.review.launch = Some(Launch { stage, ..launch }),
                }
            }
        }
    }

    /// Run the configured check once, off-loop, before any workflow exists.
    /// The form stays open: Esc abandons the launch, and dropping the
    /// launch's cancel sender stops the command.
    fn start_check(&mut self, id: u64, plan: Plan, ready: ReadyRun, check: CheckCommand) {
        self.ui_state
            .add_system_message("review: running check…".to_owned());
        let (cancel, cancelled) = oneshot::channel::<()>();
        let workspace = self.cwd.clone();
        let run_dir = ready.run_dir.clone();
        self.review.spawn_async_step(id, async move {
            let stop = async move {
                // An explicit cancel or a dropped launch both stop the check.
                match cancelled.await {
                    Ok(()) | Err(_) => {}
                }
            };
            let result = match ReviewRun::new(&workspace, &run_dir) {
                Ok(run) => run_check(&run, &check.command, check.timeout, stop).await,
                Err(error) => Err(error),
            };
            LaunchStep::Checked(result)
        });
        self.review.launch = Some(Launch {
            id,
            plan,
            stage: Stage::Checking {
                ready,
                _cancel: cancel,
            },
        });
    }

    /// Send `workflow/new`, after the agent-load wait when agents were just
    /// installed.
    async fn create_workflow(&mut self, id: u64, plan: Plan, ready: ReadyRun) {
        self.ui_state
            .add_system_message("review: creating the workflow".to_owned());
        if ready.agents_changed {
            self.review.after(
                self.review.agent_load_delay,
                ReviewTask::Launch {
                    launch: id,
                    step: LaunchStep::SendNew,
                },
            );
            self.review.launch = Some(Launch {
                id,
                plan,
                stage: Stage::Loading { ready, retries: 0 },
            });
        } else {
            self.send_new(id, plan, ready, 0).await;
        }
    }

    async fn send_new(&mut self, id: u64, plan: Plan, ready: ReadyRun, retries: u32) {
        let Some(session_id) = self.session.id().cloned() else {
            self.ui_state
                .add_system_message("review: no active session — nothing was started".to_owned());
            return;
        };
        let new = self.workflow_command(
            session_id,
            WorkflowOp::New {
                target: WorkflowRunTarget::RecipeFile(ready.recipe.clone()),
                inputs: ready.inputs.clone(),
            },
        );
        if let Err(error) = self.bridge_sender.send(new).await {
            self.ui_state
                .add_system_message(format!("review: cannot create the workflow — {error}"));
            return;
        }
        self.review.launch = Some(Launch {
            id,
            plan,
            stage: Stage::Minting { ready, retries },
        });
    }

    fn workflow_command(&self, session_id: SessionId, op: WorkflowOp) -> BridgeCommand {
        BridgeCommand::Workflow {
            session_id,
            workspace_paths: vec![self.cwd.clone()],
            op,
        }
    }

    /// Track the launch and run through `/workflow` command outcomes. The
    /// outcome itself still renders through normal routing.
    pub(super) fn observe_review_outcome(&mut self, outcome: &WorkflowCommandOutcome) {
        match outcome {
            WorkflowCommandOutcome::Minted { workflow_id, .. } if self.review.launch.is_none() => {
                if std::mem::take(&mut self.review.cancel_on_mint) {
                    self.ui_state.add_system_message(format!(
                        "review: {workflow_id} was created after /review cancel; cancelling it"
                    ));
                    self.review
                        .after(Duration::ZERO, ReviewTask::SendCancel(workflow_id.clone()));
                }
            }
            WorkflowCommandOutcome::Minted { workflow_id, .. } => self.arm_review(workflow_id),
            WorkflowCommandOutcome::Invoked { workflow_id } => {
                let invoking = matches!(
                    &self.review.launch,
                    Some(Launch { stage: Stage::Invoking(id), .. }) if id == workflow_id
                );
                if invoking {
                    self.review.launch = None;
                    let report = self
                        .review
                        .runs
                        .get(workflow_id)
                        .map(|run| run.run_dir.join("report.md"));
                    self.ui_state.add_system_message(format!(
                        "review: {workflow_id} is running — /workflow status {workflow_id} \
                         follows it; the report lands in {}",
                        report.as_deref().unwrap_or(Path::new("?")).display()
                    ));
                }
            }
            WorkflowCommandOutcome::Failed {
                operation,
                workflow_id: failed,
                details,
                ..
            } => {
                if operation == "workflow new" {
                    self.review.cancel_on_mint = false;
                }
                if operation == "workflow cancel"
                    && let Some(workflow_id) = self.review.cancelling.take_if(|cancelling| {
                        failed.as_ref().is_none_or(|failed| failed == cancelling)
                    })
                {
                    self.ui_state.add_system_message(format!(
                        "review: cancelling {workflow_id} failed — {details}; its authorization stays withdrawn"
                    ));
                }
                let stage = self.review.launch.as_ref().map(|launch| &launch.stage);
                match (operation.as_str(), stage) {
                    ("workflow new", Some(Stage::Minting { .. })) => {
                        self.review.launch = None;
                        self.ui_state.add_system_message(
                            "review: the workflow was not created; nothing was started".to_owned(),
                        );
                    }
                    ("workflow invoke", Some(Stage::Invoking(id)))
                        if failed.as_ref().is_none_or(|failed| failed == id) =>
                    {
                        let id = id.clone();
                        self.withdraw(&id, format!("review: {id} did not start"));
                    }
                    _ => {}
                }
            }
            WorkflowCommandOutcome::Cancelled { workflow_id, .. } => {
                if self.review.cancelling.as_ref() == Some(workflow_id) {
                    self.review.cancelling = None;
                    self.ui_state
                        .add_system_message(format!("review: {workflow_id} cancelled"));
                }
                if let Some(run) = self.review.run_for(workflow_id)
                    && run.authorization.is_live()
                {
                    tracing::info!(%workflow_id, "review: disarmed (cancelled)");
                    run.authorization.disarm();
                }
            }
            WorkflowCommandOutcome::Resumed { workflow_id, .. } => {
                if let Some(run) = self.review.run_for(workflow_id)
                    && run.authorization.state() == AuthorizationState::Paused
                {
                    run.authorization.rearm();
                }
            }
            _ => {}
        }
    }

    /// A `workflow/new` that failed only because KAS has not loaded the
    /// review agents yet is retried after a longer wait instead of reported.
    /// Returns whether the outcome was absorbed.
    pub(super) fn absorb_review_retry(&mut self, outcome: &WorkflowCommandOutcome) -> bool {
        let WorkflowCommandOutcome::Failed {
            operation, details, ..
        } = outcome
        else {
            return false;
        };
        if operation != "workflow new" || !details.contains(AGENT_NOT_REGISTERED) {
            return false;
        }
        let Some(launch) = self.review.launch.take() else {
            return false;
        };
        match launch.stage {
            Stage::Minting { ready, retries } if retries < AGENT_LOAD_RETRIES => {
                let delay = self.review.agent_load_delay * 2u32.pow(retries);
                tracing::info!(retries, ?delay, %details, "review: agents not loaded yet; retrying workflow/new");
                self.review.after(
                    delay,
                    ReviewTask::Launch {
                        launch: launch.id,
                        step: LaunchStep::SendNew,
                    },
                );
                self.review.launch = Some(Launch {
                    stage: Stage::Loading {
                        ready,
                        retries: retries + 1,
                    },
                    ..launch
                });
                true
            }
            stage => {
                self.review.launch = Some(Launch { stage, ..launch });
                false
            }
        }
    }

    /// `Minted`: arm the run's authorization, then persist its identity.
    /// Invoke waits for the record.
    fn arm_review(&mut self, workflow_id: &WorkflowId) {
        let Some(launch) = self.review.launch.take() else {
            return;
        };
        let Stage::Minting { ready, .. } = launch.stage else {
            self.review.launch = Some(launch);
            return;
        };
        let Ok(crtool) = self.review.prefix.as_ref() else {
            return;
        };
        let scope = PolicyScope::new(
            self.cwd.clone(),
            ready.run_dir.clone(),
            crtool.as_str().to_owned(),
        );
        let record = RunRecord {
            workflow_id: workflow_id.to_string(),
            crtool_prefix: crtool.as_str().to_owned(),
            cyril_version: env!("CARGO_PKG_VERSION").to_owned(),
            target: launch.plan.target.spec(),
            scope: launch.plan.scope.clone(),
        };
        self.review.runs.insert(
            workflow_id.clone(),
            ArmedRun {
                authorization: RunAuthorization::arm(workflow_id.clone(), scope),
                run_dir: ready.run_dir.clone(),
                sessions: HashSet::new(),
            },
        );
        self.review.launch = Some(Launch {
            stage: Stage::Recording(workflow_id.clone()),
            ..launch
        });
        let workflow_id = workflow_id.clone();
        let run_dir = ready.run_dir;
        self.review.spawn(move || {
            Some(ReviewTask::Recorded {
                workflow_id,
                result: record.write(&run_dir),
            })
        });
    }

    /// The run never started: disarm it and end the launch.
    fn withdraw(&mut self, workflow_id: &WorkflowId, message: String) {
        if let Some(run) = self.review.run_for(workflow_id) {
            run.authorization.disarm();
        }
        self.review.launch = None;
        self.ui_state
            .add_system_message(format!("{message}; its authorization is withdrawn"));
    }

    fn abandon_launch(&mut self, message: String) {
        self.ui_state.close_review_form();
        self.review.launch = None;
        self.ui_state.add_system_message(message);
    }

    /// Remember the sessions the tracker attributes to a review run; called
    /// around every lifecycle frame and fetched snapshot of that run.
    pub(super) fn remember_review_sessions(&mut self, workflow_id: &WorkflowId) {
        let Some(run) = self.review.run_for(workflow_id) else {
            return;
        };
        if let Some(tracked) = self.workflow_tracker.get(workflow_id) {
            run.sessions.extend(
                tracked
                    .nodes()
                    .filter_map(|(_, node)| node.session_id().cloned()),
            );
        }
    }

    /// `run_complete` for the armed run: pause keeps it armed; any other
    /// status disarms it and reports. A completed run's findings are read
    /// off-loop.
    pub(super) fn review_run_completed(
        &mut self,
        workflow_id: &WorkflowId,
        status: WorkflowCompletionStatus,
    ) {
        let Some(run) = self.review.run_for(workflow_id) else {
            return;
        };
        // Lifecycle frames and fetched snapshots both report endings; act on
        // the first only.
        let fresh = match status {
            WorkflowCompletionStatus::Paused => {
                run.authorization.state() == AuthorizationState::Armed
            }
            _ => run.authorization.is_live(),
        };
        if !fresh {
            return;
        }
        tracing::info!(
            %workflow_id,
            ?status,
            allowed = run.authorization.allowed_count(),
            denied = run.authorization.denials().len(),
            "review: run ended"
        );
        let ending = match status {
            WorkflowCompletionStatus::Paused => {
                run.authorization.pause();
                RunEnding::Paused {
                    workflow_id: workflow_id.to_string(),
                }
            }
            WorkflowCompletionStatus::Failed => RunEnding::Failed,
            WorkflowCompletionStatus::Aborted => RunEnding::Aborted,
            WorkflowCompletionStatus::Completed => {
                run.authorization.disarm();
                let run_dir = run.run_dir.clone();
                let denials = run.authorization.denials().to_vec();
                let workspace = self.cwd.clone();
                self.review.spawn(move || {
                    let findings = ReviewRun::new(&workspace, &run_dir)
                        .map_err(|error| FindingsError::Unreadable {
                            path: run_dir.join("findings.json"),
                            source: std::io::Error::other(error.to_string()),
                        })
                        .and_then(|run| read_findings(&run));
                    match &findings {
                        Ok(findings) => {
                            tracing::info!(findings = findings.len(), "review: summary ready");
                        }
                        Err(error) => tracing::warn!(%error, "review: findings unreadable"),
                    }
                    let ending = RunEnding::Completed(findings);
                    Some(ReviewTask::Finished(summary(&ending, &run_dir, &denials)))
                });
                return;
            }
        };
        if !matches!(ending, RunEnding::Paused { .. }) {
            run.authorization.disarm();
        }
        let text = summary(&ending, &run.run_dir, run.authorization.denials());
        self.ui_state.add_system_message(text);
    }

    #[cfg(test)]
    pub(super) fn review_run_dir_for_tests(&self, workflow_id: &WorkflowId) -> Option<PathBuf> {
        self.review
            .runs
            .get(workflow_id)
            .map(|run| run.run_dir.clone())
    }

    /// `/review cancel`: stop the review at whatever phase it is in. Before
    /// a workflow exists the launch is abandoned (a running check is killed);
    /// once one exists its authorization is withdrawn at once and the owned
    /// workflow is cancelled. Success is reported only when the agent says so.
    pub(super) fn cancel_review(&mut self) {
        let mut acted = self.cancel_resume();
        if let Some(launch) = self.review.launch.take() {
            acted = true;
            self.ui_state.close_review_form();
            match launch.stage {
                Stage::Minting { .. } | Stage::Loading { .. } => {
                    // New may be out (or about to go): whatever it creates is
                    // cancelled when it appears.
                    self.review.cancel_on_mint = matches!(launch.stage, Stage::Minting { .. });
                    self.ui_state.add_system_message(
                        "review: cancelled before the workflow started".to_owned(),
                    );
                }
                Stage::Recording(workflow_id) | Stage::Invoking(workflow_id) => {
                    self.stop_run(&workflow_id);
                }
                _ => self
                    .ui_state
                    .add_system_message("review: cancelled; nothing was started".to_owned()),
            }
        }
        if !acted {
            match self
                .review
                .live_run()
                .map(|run| run.authorization.workflow_id().clone())
            {
                Some(workflow_id) => self.stop_run(&workflow_id),
                None => self
                    .ui_state
                    .add_system_message("review: nothing to cancel".to_owned()),
            }
        }
    }

    /// Withdraw a workflow's authorization now and cancel the workflow.
    fn stop_run(&mut self, workflow_id: &WorkflowId) {
        if let Some(run) = self.review.run_for(workflow_id) {
            run.authorization.disarm();
        }
        self.ui_state.add_system_message(format!(
            "review: cancelling {workflow_id}; its authorization is withdrawn"
        ));
        self.review
            .after(Duration::ZERO, ReviewTask::SendCancel(workflow_id.clone()));
    }

    /// The agent connection is gone: no run can finish or ask again, so
    /// disarm everything and end any launch in flight.
    pub(super) fn review_agent_gone(&mut self) {
        self.review.disarm("the agent disconnected");
        if self.review.resume.take().is_some() {
            self.ui_state.close_review_form();
            self.ui_state.add_system_message(
                "review resume: the agent disconnected; nothing was continued".to_owned(),
            );
        }
        if self.review.launch.is_some() {
            self.abandon_launch(
                "review: the agent disconnected; the launch was abandoned".to_owned(),
            );
        }
    }

    /// A fetched snapshot that shows a review run paused or ended reports it
    /// like `run_complete` would: some endings only ever arrive that way.
    pub(super) fn review_snapshot_status(
        &mut self,
        workflow_id: &WorkflowId,
        status: WorkflowRunStatus,
    ) {
        let completion = match status {
            WorkflowRunStatus::Running => return,
            WorkflowRunStatus::Paused => WorkflowCompletionStatus::Paused,
            WorkflowRunStatus::Completed => WorkflowCompletionStatus::Completed,
            WorkflowRunStatus::Failed => WorkflowCompletionStatus::Failed,
            WorkflowRunStatus::Aborted => WorkflowCompletionStatus::Aborted,
        };
        self.review_run_completed(workflow_id, completion);
    }

    /// Decide a request from the armed run's step sessions; hand back every
    /// other request for an ordinary prompt. Only the workflow tracker says
    /// who owns a session.
    pub(super) fn route_review_permission(
        &mut self,
        request: PermissionRequest,
    ) -> Option<PermissionRequest> {
        let owner = match self.workflow_tracker.session_owner(&request.session_id) {
            Some((owner, _)) => owner.clone(),
            None => match self
                .review
                .runs
                .values()
                .find(|run| run.sessions.contains(&request.session_id))
            {
                Some(run) => run.authorization.workflow_id().clone(),
                None => return Some(request),
            },
        };
        let Some(run) = self.review.run_for(&owner) else {
            return Some(request);
        };
        run.sessions.insert(request.session_id.clone());
        let consent = request
            .consent
            .clone()
            .unwrap_or_else(|| PermissionConsent::new(None, None, None, None, None, Vec::new()));
        let run_dir = run.run_dir.clone();
        if run.authorization.is_live() {
            let scope = run.authorization.scope().clone();
            self.review.spawn(move || {
                let decision = decide(&consent, &scope);
                answer(request, &decision, &run_dir);
                Some(ReviewTask::Decided {
                    workflow_id: owner,
                    decision,
                })
            });
        } else {
            // A late request from an ended run: denied without a path check.
            let decision = run.authorization.decide(&consent);
            self.review.spawn(move || {
                answer(request, &decision, &run_dir);
                None
            });
        }
        None
    }
}

/// The blocking half of opening the form: settings, branches, the target the
/// arguments asked for, and what its diff touches.
fn open_launch(workspace: &Path, requested: Option<TargetArg>) -> Result<Opened, LaunchError> {
    let config = ReviewConfig::load(workspace)?;
    // Without a branch list the form offers no base mode and says why; the
    // other targets still work.
    let (branches, branch_note) = match launch::base_branches(workspace) {
        Ok(branches) => (branches, None),
        Err(error) => {
            tracing::warn!(%error, "review: cannot list base branches");
            (Vec::new(), Some(format!("no base branches: {error}")))
        }
    };
    let (target, target_problem) = match requested {
        None | Some(TargetArg::Auto) => (ReviewTarget::Auto, None),
        Some(TargetArg::Uncommitted) => (ReviewTarget::Uncommitted, None),
        Some(TargetArg::HeadCommit) => (ReviewTarget::HeadCommit, None),
        Some(TargetArg::Base(Some(base))) => {
            let problem = (!branches.contains(&base))
                .then(|| format!("there is no branch {base:?} to compare against"));
            (ReviewTarget::Base(base), problem)
        }
        Some(TargetArg::Base(None)) => match ReviewTarget::Auto.cycle(&branches, None, false) {
            base @ ReviewTarget::Base(_) => (base, None),
            _ => (
                ReviewTarget::Auto,
                Some("there is no other branch to compare against".to_owned()),
            ),
        },
    };
    let target_problem = target_problem.or_else(|| input_problem("target", &target.spec()));
    // A target the operator named but cannot be diffed is a problem on the
    // form; outside the repository root the review cannot open at all.
    let touched = if target_problem.is_some() {
        Ok(launch::Touched::default())
    } else {
        launch::touched(workspace, &target.spec())
    };
    let (touched, target_problem) = match touched {
        Ok(touched) => (touched, target_problem),
        Err(error @ LaunchError::NotRoot { .. }) => return Err(error),
        Err(error) if target != ReviewTarget::Auto => {
            (launch::Touched::default(), Some(error.to_string()))
        }
        Err(error) => return Err(error),
    };
    Ok(Opened {
        config,
        target,
        target_problem,
        touched,
        branches,
        branch_note,
    })
}

/// The refusal outside the root is quoted as the spec words it.
fn launch_message(error: &LaunchError) -> String {
    match error {
        LaunchError::NotRoot { .. } => error.to_string(),
        _ => format!("review: {error}"),
    }
}

/// Answer with the request's own allow-once or reject-once option — never
/// an always or trust option — or cancel when it offers none.
fn answer(request: PermissionRequest, decision: &Decision, run_dir: &Path) {
    let kind = if decision.allowed() {
        tracing::debug!(session_id = %request.session_id, reason = decision.reason(), "review: allowed");
        PermissionOptionKind::AllowOnce
    } else {
        tracing::warn!(session_id = %request.session_id, reason = decision.reason(), "review: denied");
        append_denial(run_dir, &request.session_id, decision.reason());
        PermissionOptionKind::RejectOnce
    };
    let response = request
        .options
        .iter()
        .find(|option| option.kind == kind)
        .map_or(PermissionResponse::Cancel, |option| {
            PermissionResponse::Selected {
                option_id: option.id.clone(),
                trust_option: None,
            }
        });
    if request.responder.send(response).is_err() {
        tracing::warn!(session_id = %request.session_id, "review: the agent stopped waiting for a permission answer");
    }
}

fn append_denial(run_dir: &Path, session_id: &SessionId, reason: &str) {
    let path = run_dir.join(DENIED_LOG);
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| writeln!(file, "{session_id}\t{reason}"));
    if let Err(error) = written {
        tracing::warn!(path = %path.display(), %error, "review: cannot append to the denial log");
    }
}
