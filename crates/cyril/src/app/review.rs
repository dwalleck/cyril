//! `/review` (cyril-iowg): the consent form, the launch chain and the armed
//! run's permission policy.
//!
//! The chain is `/review` → form (probe) → Enter → prepare (install, run
//! directory, gather) → `workflow/new` → `Minted` → arm + `run.json` →
//! `workflow/invoke`. Every blocking step — git probes, file writes, the
//! policy's path checks, reading findings — runs on `spawn_blocking` and
//! reports back through [`ReviewState::rx`]; the event loop only routes.

use super::App;
use cyril_core::review::authorization::RunAuthorization;
use cyril_core::review::consent::PermissionConsent;
use cyril_core::review::launch::{self, LaunchError, LaunchRequest, Prepared, ReadyRun};
use cyril_core::review::policy::{Decision, PolicyScope, decide};
use cyril_core::review::run_record::{RunRecord, RunRecordError};
use cyril_core::review::summary::{RunEnding, summary};
use cyril_core::review::{CrtoolPrefix, ShellDialect};
use cyril_core::types::{
    BridgeCommand, PermissionOptionKind, PermissionRequest, PermissionResponse, SessionId,
    WorkflowCommandOutcome, WorkflowCompletionStatus, WorkflowId, WorkflowOp, WorkflowRunTarget,
};
use cyril_ui::traits::{ReviewCheck, ReviewForm};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// The only target this slice offers: HEAD against its upstream or main.
const TARGET: &str = "auto";
/// Every denial of a run's requests, one per line, in its run directory.
pub(super) const DENIED_LOG: &str = "denied.log";

pub(super) struct ReviewState {
    /// The crtool spelling for the session's host shell, or why there is none.
    prefix: Result<CrtoolPrefix, String>,
    /// Where the assets install: Node's home directory.
    home: Option<PathBuf>,
    launch: Option<Launch>,
    /// The latest run. Kept after it ends so its late requests are denied
    /// and logged rather than falling through to an ordinary prompt.
    run: Option<ArmedRun>,
    tx: mpsc::UnboundedSender<ReviewTask>,
    pub(super) rx: mpsc::UnboundedReceiver<ReviewTask>,
}

/// A launch between `/review` and a started run.
struct Launch {
    target: String,
    scope: Vec<String>,
    stage: Stage,
}

enum Stage {
    /// The form is open.
    Form,
    Preparing,
    Minting(ReadyRun),
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

/// A result from blocking work, back on the event loop.
pub(super) enum ReviewTask {
    Probed(Result<usize, LaunchError>),
    Prepared(Result<Prepared, LaunchError>),
    Recorded {
        workflow_id: WorkflowId,
        result: Result<(), RunRecordError>,
    },
    Decided {
        workflow_id: WorkflowId,
        decision: Decision,
    },
    Finished(String),
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
            launch: None,
            run: None,
            tx,
            rx,
        }
    }

    #[cfg(test)]
    pub(super) fn set_environment(&mut self, prefix: CrtoolPrefix, home: PathBuf) {
        self.prefix = Ok(prefix);
        self.home = Some(home);
    }

    /// Run `work` off the event loop; its result, if any, comes back on `rx`.
    fn spawn(&self, work: impl FnOnce() -> Option<ReviewTask> + Send + 'static) {
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(task) = work()
                && tx.send(task).is_err()
            {
                tracing::debug!("review result dropped: the app is gone");
            }
        });
    }

    fn live_run(&self) -> Option<&WorkflowId> {
        self.run
            .as_ref()
            .filter(|run| run.authorization.is_live())
            .map(|run| run.authorization.workflow_id())
    }

    fn run_for(&mut self, workflow_id: &WorkflowId) -> Option<&mut ArmedRun> {
        self.run
            .as_mut()
            .filter(|run| run.authorization.workflow_id() == workflow_id)
    }

    /// Disarm the live run, if any; it keeps denying late requests.
    pub(super) fn disarm(&mut self, why: &str) {
        if let Some(run) = self.run.as_mut().filter(|run| run.authorization.is_live()) {
            tracing::info!(workflow_id = %run.authorization.workflow_id(), why, "review: disarmed");
            run.authorization.disarm();
        }
    }
}

impl App {
    /// `/review`: open the consent form and count the files off-loop.
    pub(super) fn open_review(&mut self) {
        if let Some(id) = self.review.live_run() {
            let text = format!("review: {id} is still running — /workflow status {id}");
            self.ui_state.add_system_message(text);
            return;
        }
        if self.review.launch.is_some() {
            self.ui_state
                .add_system_message("review: a review is already starting".to_owned());
            return;
        }
        if let Err(problem) = &self.review.prefix {
            let text = format!("review: unavailable — {problem}");
            self.ui_state.add_system_message(text);
            return;
        }
        let target = TARGET.to_owned();
        let scope = vec![".".to_owned()];
        self.ui_state.show_review_form(ReviewForm {
            target: target.clone(),
            scope: scope.clone(),
            file_count: None,
            check: ReviewCheck::NotConfigured,
            busy: false,
        });
        self.review.launch = Some(Launch {
            target: target.clone(),
            scope: scope.clone(),
            stage: Stage::Form,
        });
        let workspace = self.cwd.clone();
        self.review.spawn(move || {
            Some(ReviewTask::Probed(launch::probe(
                &workspace, &target, &scope,
            )))
        });
    }

    /// Esc backs out with no side effects (ignored once the launch is under
    /// way); Enter confirms once the count is known; every other key is
    /// consumed so nothing reaches the chat behind the form.
    pub(super) fn handle_review_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        let Some(form) = self.ui_state.review_form_mut() else {
            return;
        };
        if form.busy {
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.ui_state.close_review_form();
                self.review.launch = None;
            }
            KeyCode::Enter if form.file_count.is_some() => self.confirm_review(),
            _ => {}
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
        if let Some(form) = self.ui_state.review_form_mut() {
            form.busy = true;
        }
        let request = LaunchRequest {
            workspace: self.cwd.clone(),
            target: launch.target.clone(),
            scope: launch.scope.clone(),
            crtool,
            home: self.review.home.clone(),
        };
        self.review
            .spawn(move || Some(ReviewTask::Prepared(launch::prepare(&request))));
    }

    pub(super) async fn handle_review_task(&mut self, task: ReviewTask) {
        self.redraw_needed = true;
        match task {
            ReviewTask::Probed(result) => {
                if !matches!(
                    self.review.launch,
                    Some(Launch {
                        stage: Stage::Form,
                        ..
                    })
                ) {
                    return;
                }
                match result {
                    Ok(count) => {
                        if let Some(form) = self.ui_state.review_form_mut() {
                            form.file_count = Some(count);
                        }
                    }
                    Err(error) => self.abandon_launch(launch_message(&error)),
                }
            }
            ReviewTask::Prepared(result) => {
                let Some(launch) = self.review.launch.take() else {
                    return;
                };
                self.ui_state.close_review_form();
                match result {
                    Ok(Prepared::Nothing) => self.ui_state.add_system_message(format!(
                        "review: nothing to review — {} in {}",
                        launch.target,
                        launch.scope.join(" ")
                    )),
                    Err(error) => self.ui_state.add_system_message(launch_message(&error)),
                    Ok(Prepared::Ready(ready)) => self.send_new(launch, ready).await,
                }
            }
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
        }
    }

    async fn send_new(&mut self, launch: Launch, ready: ReadyRun) {
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
        self.ui_state.add_system_message(format!(
            "review: gathered {} file(s) into {} — creating the workflow",
            ready.files,
            ready.run_dir.display()
        ));
        self.review.launch = Some(Launch {
            stage: Stage::Minting(ready),
            ..launch
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
                        .run
                        .as_ref()
                        .map(|run| run.run_dir.join("report.md"));
                    self.ui_state.add_system_message(format!(
                        "review: {workflow_id} is running — /workflow status {workflow_id} \
                         follows it; the report lands in {}",
                        report.as_deref().unwrap_or(Path::new("?")).display()
                    ));
                }
            }
            WorkflowCommandOutcome::Failed { operation, .. } => {
                let stage = self.review.launch.as_ref().map(|launch| &launch.stage);
                match (operation.as_str(), stage) {
                    ("workflow new", Some(Stage::Minting(_))) => {
                        self.review.launch = None;
                        self.ui_state.add_system_message(
                            "review: the workflow was not created; nothing was started".to_owned(),
                        );
                    }
                    ("workflow invoke", Some(Stage::Invoking(id))) => {
                        let id = id.clone();
                        self.withdraw(&id, format!("review: {id} did not start"));
                    }
                    _ => {}
                }
            }
            WorkflowCommandOutcome::Cancelled { workflow_id, .. } => {
                if self.review.live_run() == Some(workflow_id) {
                    self.review.disarm("cancelled");
                }
            }
            _ => {}
        }
    }

    /// `Minted`: arm the run's authorization, then persist its identity.
    /// Invoke waits for the record.
    fn arm_review(&mut self, workflow_id: &WorkflowId) {
        let Some(launch) = self.review.launch.take() else {
            return;
        };
        let Stage::Minting(ready) = launch.stage else {
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
            target: launch.target.clone(),
            scope: launch.scope.clone(),
        };
        self.review.run = Some(ArmedRun {
            authorization: RunAuthorization::arm(workflow_id.clone(), scope),
            run_dir: ready.run_dir.clone(),
            sessions: HashSet::new(),
        });
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

    /// Remember the sessions the tracker attributes to the review's run;
    /// called around every lifecycle frame of that run.
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
        let ending = match status {
            WorkflowCompletionStatus::Paused => {
                run.authorization.pause();
                RunEnding::Paused
            }
            WorkflowCompletionStatus::Failed => RunEnding::Failed,
            WorkflowCompletionStatus::Aborted => RunEnding::Aborted,
            WorkflowCompletionStatus::Completed => {
                run.authorization.disarm();
                let run_dir = run.run_dir.clone();
                let denials = run.authorization.denials().to_vec();
                let workspace = self.cwd.clone();
                self.review.spawn(move || {
                    let findings = cyril_review::ReviewRun::new(&workspace, &run_dir)
                        .map_err(|error| cyril_review::FindingsError::Unreadable {
                            path: run_dir.join("findings.json"),
                            source: std::io::Error::other(error.to_string()),
                        })
                        .and_then(|run| cyril_review::read_findings(&run));
                    let ending = RunEnding::Completed(findings);
                    Some(ReviewTask::Finished(summary(&ending, &run_dir, &denials)))
                });
                return;
            }
        };
        if !matches!(ending, RunEnding::Paused) {
            run.authorization.disarm();
        }
        let text = summary(&ending, &run.run_dir, run.authorization.denials());
        self.ui_state.add_system_message(text);
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
                .run
                .as_ref()
                .filter(|run| run.sessions.contains(&request.session_id))
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
