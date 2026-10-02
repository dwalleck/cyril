//! `/review resume` (cyril-2ruj): continue a failed or paused review run,
//! even after a restart, with fresh consent.
//!
//! The chain is scan `.code-review/*/run.json` → `workflow/list` (the agent's
//! persisted status, the only authority on whether a run can continue) →
//! choose → read-only form → Enter → install the assets → `workflow/load`
//! (skipped when the tracker already holds the run) → arm → `retry` (failed)
//! or `resume` (paused). Nothing is gathered or checked again: the run keeps
//! its stored inputs and facts.

use super::*;
use cyril_core::review::launch::{Installed, install_assets, reserved_agents};
use cyril_core::review::resume::{self as resume_core, Candidate, Chosen};
use cyril_ui::traits::{ReviewResumeView, TuiState};

pub(super) struct Resume {
    pub(super) id: u64,
    stage: ResumeStage,
}

enum ResumeStage {
    Scanning {
        selector: Option<String>,
    },
    /// `workflow/list` is out; its reply is absorbed, not rendered.
    Listing {
        selector: Option<String>,
        candidates: Vec<Candidate>,
    },
    Choosing,
    /// The read-only form is open.
    Form(Box<Chosen>),
    Installing(Box<Chosen>),
    /// Waiting for the agent-load pause or the `workflow/load` reply.
    Loading(Box<Chosen>),
    /// `retry`/`resume` is out; the run is armed.
    Continuing(WorkflowId),
}

/// The resume steps that run off the event loop.
pub(in crate::app) enum ResumeStep {
    Scanned(std::io::Result<Vec<Candidate>>),
    Chosen(Result<(Box<Chosen>, usize), String>),
    Installed(Result<Installed, LaunchError>),
    /// Load (or skip it) now.
    Proceed,
    /// `workflow/load` registered the run: continue it.
    Loaded,
}

impl App {
    /// `/review resume [<run dir | workflow id>]`.
    pub(in crate::app) fn open_resume(&mut self, selector: Option<String>) {
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
        let id = self.review.next_launch;
        self.review.next_launch += 1;
        self.review.resume = Some(Resume {
            id,
            stage: ResumeStage::Scanning { selector },
        });
        let workspace = self.cwd.clone();
        self.review.spawn_step(id, move || {
            LaunchStep::Resume(ResumeStep::Scanned(resume_core::scan(&workspace)))
        });
    }

    pub(super) async fn handle_resume_step(&mut self, step: ResumeStep) {
        let Some(resume) = self.review.resume.take() else {
            return;
        };
        let id = resume.id;
        match (resume.stage, step) {
            (ResumeStage::Scanning { selector }, ResumeStep::Scanned(Ok(candidates))) => {
                let Some(session_id) = self.session.id().cloned() else {
                    self.ui_state
                        .add_system_message("review resume: no active session".to_owned());
                    return;
                };
                let list = self.workflow_command(session_id, WorkflowOp::ListRuns);
                if let Err(error) = self.bridge_sender.send(list).await {
                    self.ui_state
                        .add_system_message(format!("review resume: {error}"));
                    return;
                }
                self.review.resume = Some(Resume {
                    id,
                    stage: ResumeStage::Listing {
                        selector,
                        candidates,
                    },
                });
            }
            (ResumeStage::Scanning { .. }, ResumeStep::Scanned(Err(error))) => {
                self.ui_state.add_system_message(format!(
                    "review resume: cannot read {}: {error}",
                    launch::RUNS_DIR
                ));
            }
            (ResumeStage::Choosing, ResumeStep::Chosen(Err(problem))) => {
                self.ui_state.add_system_message(problem);
            }
            (ResumeStage::Choosing, ResumeStep::Chosen(Ok((chosen, files)))) => {
                if let Some(live) = self.review.live_run() {
                    let live_id = live.authorization.workflow_id();
                    // A paused run may be reconfirmed; any other armed run
                    // keeps the one-review rule.
                    let same_paused = live_id == &chosen.workflow_id
                        && live.authorization.state() == AuthorizationState::Paused;
                    if !same_paused {
                        self.ui_state.add_system_message(format!(
                            "review resume: {live_id} is still running — /workflow status {live_id}"
                        ));
                        return;
                    }
                }
                let view = ReviewResumeView {
                    run: chosen
                        .dir
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    status: chosen.status.to_string(),
                    target: chosen.record.target.clone(),
                    unreadable: chosen.unreadable.clone(),
                };
                self.ui_state.show_review_form(ReviewForm::resuming(
                    view,
                    chosen.record.scope.clone(),
                    files,
                ));
                self.review.resume = Some(Resume {
                    id,
                    stage: ResumeStage::Form(chosen),
                });
            }
            (ResumeStage::Installing(chosen), ResumeStep::Installed(result)) => {
                self.ui_state.close_review_form();
                match result {
                    Err(error) => self
                        .ui_state
                        .add_system_message(format!("review resume: {error}; nothing was started")),
                    Ok(installed) => {
                        self.review.resume = Some(Resume {
                            id,
                            stage: ResumeStage::Loading(chosen),
                        });
                        let delay = if installed.agents_changed {
                            self.review.agent_load_delay
                        } else {
                            Duration::ZERO
                        };
                        self.review.after(
                            delay,
                            ReviewTask::Launch {
                                launch: id,
                                step: LaunchStep::Resume(ResumeStep::Proceed),
                            },
                        );
                    }
                }
            }
            (ResumeStage::Loading(chosen), ResumeStep::Proceed) => {
                if self.workflow_tracker.get(&chosen.workflow_id).is_some() {
                    self.continue_run(id, *chosen).await;
                } else {
                    self.load_run(id, chosen).await;
                }
            }
            (ResumeStage::Loading(chosen), ResumeStep::Loaded) => {
                self.continue_run(id, *chosen).await;
            }
            (stage, _) => {
                tracing::debug!(launch = id, "review resume: a step arrived out of order");
                self.review.resume = Some(Resume { id, stage });
            }
        }
    }

    async fn load_run(&mut self, id: u64, chosen: Box<Chosen>) {
        let Some(session_id) = self.session.id().cloned() else {
            self.ui_state
                .add_system_message("review resume: no active session".to_owned());
            return;
        };
        let load = self.workflow_command(
            session_id,
            WorkflowOp::Load {
                id: chosen.workflow_id.clone(),
            },
        );
        if let Err(error) = self.bridge_sender.send(load).await {
            self.ui_state
                .add_system_message(format!("review resume: {error}; nothing was started"));
            return;
        }
        // Continue once the reply says the run is registered.
        self.review.resume = Some(Resume {
            id,
            stage: ResumeStage::Loading(chosen),
        });
    }

    /// Arm the run (or re-arm the paused one this process holds), then send
    /// `retry` for a failed run or `resume` for a paused one.
    async fn continue_run(&mut self, id: u64, chosen: Chosen) {
        let workflow_id = chosen.workflow_id.clone();
        match self.review.run_for(&workflow_id) {
            Some(run) => run.authorization.rearm(),
            None => {
                let scope = PolicyScope::new(
                    self.cwd.clone(),
                    chosen.dir.clone(),
                    chosen.record.crtool_prefix.clone(),
                );
                self.review.runs.insert(
                    workflow_id.clone(),
                    ArmedRun {
                        authorization: RunAuthorization::arm(workflow_id.clone(), scope),
                        run_dir: chosen.dir.clone(),
                        sessions: HashSet::new(),
                    },
                );
            }
        }
        let op = match chosen.status {
            WorkflowRunStatus::Paused => WorkflowOp::Resume {
                id: workflow_id.clone(),
            },
            _ => WorkflowOp::Retry {
                id: workflow_id.clone(),
            },
        };
        let Some(session_id) = self.session.id().cloned() else {
            self.withdraw_resume(&workflow_id, "the session is gone");
            return;
        };
        let command = self.workflow_command(session_id, op);
        if let Err(error) = self.bridge_sender.send(command).await {
            self.withdraw_resume(&workflow_id, &error.to_string());
            return;
        }
        self.review.resume = Some(Resume {
            id,
            stage: ResumeStage::Continuing(workflow_id),
        });
    }

    fn withdraw_resume(&mut self, workflow_id: &WorkflowId, why: &str) {
        if let Some(run) = self.review.run_for(workflow_id) {
            run.authorization.disarm();
        }
        self.review.resume = None;
        self.ui_state.add_system_message(format!(
            "review resume: {workflow_id} did not continue ({why}); its authorization is withdrawn"
        ));
    }

    /// `/review cancel` during a resume. Returns whether one was in progress.
    pub(super) fn cancel_resume(&mut self) -> bool {
        let Some(resume) = self.review.resume.take() else {
            return false;
        };
        self.ui_state.close_review_form();
        match resume.stage {
            // retry/resume is out: the run may be executing again.
            ResumeStage::Continuing(workflow_id) => self.stop_run(&workflow_id),
            _ => self
                .ui_state
                .add_system_message("review resume: cancelled; nothing was continued".to_owned()),
        }
        true
    }

    /// Esc leaves everything as it was (a paused run stays armed); Enter
    /// installs the assets and continues.
    pub(super) fn handle_resume_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        let busy = self.ui_state.review_form().is_some_and(|form| form.busy);
        match key.code {
            KeyCode::Esc if !busy => {
                self.ui_state.close_review_form();
                self.review.resume = None;
            }
            KeyCode::Enter if !busy => {
                let Some(resume) = self.review.resume.take() else {
                    return;
                };
                let ResumeStage::Form(chosen) = resume.stage else {
                    self.review.resume = Some(resume);
                    return;
                };
                if let Some(form) = self.ui_state.review_form_mut() {
                    form.busy = true;
                }
                let workspace = self.cwd.clone();
                let home = self.review.home.clone();
                self.review.spawn_step(resume.id, move || {
                    let installed = reserved_agents(&workspace).and_then(|reserved| {
                        if !reserved.is_empty() {
                            return Err(LaunchError::ReservedAgents { paths: reserved });
                        }
                        install_assets(home.as_deref().ok_or(LaunchError::NoHome)?)
                    });
                    LaunchStep::Resume(ResumeStep::Installed(installed))
                });
                self.review.resume = Some(Resume {
                    id: resume.id,
                    stage: ResumeStage::Installing(chosen),
                });
            }
            _ => {}
        }
    }

    /// `workflow/list` answered: choose the run off-loop. Returns whether the
    /// listing belonged to a resume (and is not shown as a run table).
    pub(in crate::app) fn absorb_resume_listing(
        &mut self,
        outcome: &WorkflowCommandOutcome,
    ) -> bool {
        let Some(resume) = self.review.resume.take() else {
            return false;
        };
        let ResumeStage::Listing {
            selector,
            candidates,
        } = resume.stage
        else {
            self.review.resume = Some(resume);
            return false;
        };
        let id = resume.id;
        match outcome {
            WorkflowCommandOutcome::Runs { runs, .. } => {
                let statuses = runs
                    .iter()
                    .map(|run| (run.workflow_id.clone(), run.status))
                    .collect::<HashMap<_, _>>();
                let prefix = match &self.review.prefix {
                    Ok(prefix) => prefix.as_str().to_owned(),
                    Err(problem) => problem.clone(),
                };
                self.review.spawn_step(id, move || {
                    let chosen = resume_core::choose(candidates, selector.as_deref(), &statuses)
                        .and_then(|chosen| {
                            resume_core::compatible(
                                &chosen.record,
                                &prefix,
                                env!("CARGO_PKG_VERSION"),
                            )?;
                            Ok(chosen)
                        })
                        .map_err(|error| error.to_string())
                        .and_then(|chosen| {
                            let files = manifest_files(&chosen.dir)?;
                            Ok((Box::new(chosen), files))
                        });
                    LaunchStep::Resume(ResumeStep::Chosen(chosen))
                });
                self.review.resume = Some(Resume {
                    id,
                    stage: ResumeStage::Choosing,
                });
                true
            }
            WorkflowCommandOutcome::Failed { operation, .. } if operation == "workflow list" => {
                self.ui_state.add_system_message(
                    "review resume: the agent could not list its runs".to_owned(),
                );
                false
            }
            _ => {
                self.review.resume = Some(Resume {
                    id,
                    stage: ResumeStage::Listing {
                        selector,
                        candidates,
                    },
                });
                false
            }
        }
    }

    /// Track the resume through load/retry/resume outcomes.
    pub(in crate::app) fn observe_resume_outcome(&mut self, outcome: &WorkflowCommandOutcome) {
        let Some(resume) = self.review.resume.as_ref() else {
            return;
        };
        let id = resume.id;
        match (&resume.stage, outcome) {
            (ResumeStage::Loading(chosen), WorkflowCommandOutcome::Loaded { workflow_id, .. })
                if *workflow_id == chosen.workflow_id =>
            {
                self.review.after(
                    Duration::ZERO,
                    ReviewTask::Launch {
                        launch: id,
                        step: LaunchStep::Resume(ResumeStep::Loaded),
                    },
                );
            }
            (ResumeStage::Loading(chosen), WorkflowCommandOutcome::Failed { operation, .. })
                if operation == "workflow load" =>
            {
                let text = format!(
                    "review resume: could not load {}; nothing was started",
                    chosen.workflow_id
                );
                self.review.resume = None;
                self.ui_state.add_system_message(text);
            }
            (
                ResumeStage::Continuing(expected),
                WorkflowCommandOutcome::Retried { workflow_id, .. }
                | WorkflowCommandOutcome::Resumed { workflow_id, .. },
            ) if workflow_id == expected => {
                let text = format!(
                    "review: {workflow_id} continues — /workflow status {workflow_id} follows it"
                );
                self.review.resume = None;
                self.ui_state.add_system_message(text);
            }
            (
                ResumeStage::Continuing(expected),
                WorkflowCommandOutcome::Failed { operation, .. },
            ) if operation == "workflow retry" || operation == "workflow resume" => {
                let expected = expected.clone();
                self.withdraw_resume(&expected, "the agent refused it");
            }
            _ => {}
        }
    }
}

/// How many files the run gathered, from its manifest.
fn manifest_files(run_dir: &Path) -> Result<usize, String> {
    let path = run_dir.join("manifest.json");
    let manifest: serde_json::Value = std::fs::read(&path)
        .map_err(|error| format!("review resume: cannot read {}: {error}", path.display()))
        .and_then(|bytes| {
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("review resume: {} is corrupt: {error}", path.display()))
        })?;
    manifest["total_files"]
        .as_u64()
        .and_then(|files| usize::try_from(files).ok())
        .ok_or_else(|| format!("review resume: {} has no total_files", path.display()))
}
