//! `/review` through the App (cyril-iowg): form keys, the launch chain, the
//! armed run's permission policy and completion — asserting outbound bridge
//! commands, permission answers, chat messages and files on disk against a
//! real fixture repository and a fake home.

use super::*;
use cyril_core::review::consent::PermissionConsent;
use cyril_core::review::{CrtoolPrefix, ShellDialect};
use cyril_core::types::{
    WorkflowCommandOutcome, WorkflowNodeDescriptor, WorkflowNodeSnapshot, WorkflowOp,
    WorkflowRunCompleted, WorkflowRunTarget, WorkflowSnapshot, WorkflowSnapshotData,
    WorkflowSnapshotMetadata,
};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const RUN: &str = "wf_review";

struct Repo {
    _tree: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", repo.join("../gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Review")
        .env("GIT_AUTHOR_EMAIL", "review@example.invalid")
        .env("GIT_COMMITTER_NAME", "Review")
        .env("GIT_COMMITTER_EMAIL", "review@example.invalid")
        .output()
        .unwrap_or_else(|error| panic!("spawn git: {error}"));
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A committed repository on `main`; `changed` edits one tracked file.
fn repo(changed: bool) -> Repo {
    let tree = tempfile::tempdir().expect("tempdir");
    let base = tree.path().to_path_buf();
    fs::write(base.join("gitconfig"), "").expect("gitconfig");
    let root = base.join("repo");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(root.join("src/a.rs"), "fn a() {}\n").expect("a.rs");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "core.autocrlf", "false"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "first"]);
    if changed {
        fs::write(root.join("src/a.rs"), "fn a() { todo!() }\n").expect("edit");
    }
    let home = base.join("home");
    fs::create_dir_all(&home).expect("home");
    Repo {
        _tree: tree,
        root,
        home,
    }
}

/// The recipe's forward-slash `rundir` as the native path the App reports.
fn native(rundir: &str) -> PathBuf {
    PathBuf::from(rundir.replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn prefix() -> CrtoolPrefix {
    let exe = if cfg!(windows) {
        "C:/bin/cyril.exe"
    } else {
        "/bin/cyril"
    };
    CrtoolPrefix::from_executable(Path::new(exe), ShellDialect::Posix).expect("a valid prefix")
}

fn review_app(repo: &Repo) -> (App, tokio::sync::mpsc::Receiver<BridgeCommand>) {
    let (mut app, rx) = test_app_with_command_rx();
    app.cwd = repo.root.clone();
    app.review.set_environment(prefix(), repo.home.clone());
    app.handle_notification(session_created_frame(&SessionId::new("sess_main")));
    (app, rx)
}

/// Deliver the next result of the review's off-loop work.
async fn settle(app: &mut App) {
    let task = tokio::time::timeout(Duration::from_secs(30), app.review.rx.recv())
        .await
        .expect("review work finishes")
        .expect("the App holds the sender");
    app.handle_review_task(task).await;
}

fn last_message(app: &App) -> String {
    app.ui_state
        .messages()
        .iter()
        .rev()
        .find_map(|message| match message.kind() {
            ChatMessageKind::System(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn outcome(outcome: WorkflowCommandOutcome) -> RoutedNotification {
    RoutedNotification::global(Notification::WorkflowCommand(outcome))
}

fn run_id() -> WorkflowId {
    WorkflowId::try_from(RUN.to_owned()).expect("valid id")
}

/// [`RUN`]'s state with the `alpha` step, at `status`.
fn final_snapshot(status: WorkflowRunStatus) -> WorkflowSnapshot {
    let node_status = match status {
        WorkflowRunStatus::Paused => WorkflowNodeStatus::Paused,
        WorkflowRunStatus::Completed => WorkflowNodeStatus::Completed,
        WorkflowRunStatus::Failed => WorkflowNodeStatus::Failed,
        _ => WorkflowNodeStatus::Aborted,
    };
    WorkflowSnapshot::new(
        run_id(),
        format!("recipe-{RUN}"),
        status,
        WorkflowSnapshotData::new(
            serde_json::json!({"input": true}),
            serde_json::json!({}),
            serde_json::json!({}),
        ),
        WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(workflow_node_id(RUN), Vec::new()),
            node_status,
            vec![WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::step(
                    workflow_node_id("alpha"),
                    "agent".to_owned(),
                    None,
                    None,
                ),
                node_status,
                Vec::new(),
            )],
        ),
        WorkflowSnapshotMetadata::new("created".to_owned(), 1),
    )
}

/// `run_complete` for [`RUN`] with the claimed `alpha` step.
fn completion(status: WorkflowRunStatus) -> RoutedNotification {
    let completion_status = match status {
        WorkflowRunStatus::Paused => WorkflowCompletionStatus::Paused,
        WorkflowRunStatus::Completed => WorkflowCompletionStatus::Completed,
        WorkflowRunStatus::Failed => WorkflowCompletionStatus::Failed,
        _ => WorkflowCompletionStatus::Aborted,
    };
    let completed = WorkflowRunCompleted::new(run_id(), completion_status, final_snapshot(status))
        .expect("valid completion");
    RoutedNotification::global(Notification::Workflow(Box::new(
        WorkflowEvent::RunCompleted(completed),
    )))
}

/// A step's request carrying `consent`, offering the options KAS offers.
fn step_request(
    session: &str,
    consent: PermissionConsent,
    kinds: &[PermissionOptionKind],
) -> (
    PermissionRequest,
    tokio::sync::oneshot::Receiver<PermissionResponse>,
) {
    let (responder, receiver) = tokio::sync::oneshot::channel();
    let options = kinds
        .iter()
        .map(|kind| PermissionOption {
            id: PermissionOptionId::new(format!("{kind:?}")),
            label: format!("{kind:?}"),
            kind: *kind,
            is_destructive: false,
        })
        .collect();
    (
        PermissionRequest {
            session_id: SessionId::new(session),
            tool_call: ToolCall::new(
                ToolCallId::new(format!("tool-{session}")),
                "step tool".into(),
                ToolKind::Execute,
                ToolCallStatus::Pending,
                None,
            ),
            message: "Allow?".into(),
            options,
            trust_options: Vec::new(),
            can_reject_with_reason: false,
            responder,
            consent: Some(consent),
        },
        receiver,
    )
}

const KAS_OPTIONS: [PermissionOptionKind; 3] = [
    PermissionOptionKind::AllowAlways,
    PermissionOptionKind::AllowOnce,
    PermissionOptionKind::RejectOnce,
];

fn read(resource: &str) -> PermissionConsent {
    PermissionConsent::new(
        Some("fs_read"),
        Some(resource.to_owned()),
        None,
        None,
        None,
        Vec::new(),
    )
}

fn shell(command: &str) -> PermissionConsent {
    PermissionConsent::new(
        Some("shell"),
        Some(command.to_owned()),
        None,
        None,
        None,
        Vec::new(),
    )
}

/// The option id the policy answered with, or `None` for a cancel.
async fn answered(receiver: tokio::sync::oneshot::Receiver<PermissionResponse>) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(30), receiver)
        .await
        .expect("answered in time")
        .expect("answered, not dropped")
    {
        PermissionResponse::Selected { option_id, .. } => Some(option_id.as_str().to_owned()),
        PermissionResponse::Cancel => None,
        other => panic!("the policy never answers with {other:?}"),
    }
}

/// The next outbound command once `workflow/new` is due: a fresh install
/// first waits for KAS to load the review agents.
async fn next_new(
    app: &mut App,
    rx: &mut tokio::sync::mpsc::Receiver<BridgeCommand>,
) -> Result<BridgeCommand, tokio::sync::mpsc::error::TryRecvError> {
    if let Ok(command) = rx.try_recv() {
        return Ok(command);
    }
    settle(app).await;
    rx.try_recv()
}

/// Decide `request` through the policy: it must never reach the overlay.
async fn decide(app: &mut App, request: PermissionRequest) {
    assert!(
        app.route_review_permission(request).is_none(),
        "the armed run's request must not be prompted"
    );
}

/// Drive `/review` → Enter → New → Minted → run.json → Invoke → Invoked,
/// with the `alpha` step claimed by `sess_step`. Returns the run directory.
async fn launch(
    app: &mut App,
    rx: &mut tokio::sync::mpsc::Receiver<BridgeCommand>,
    repo: &Repo,
) -> PathBuf {
    app.handle_command_result(CommandResult::review());
    settle(app).await;
    assert_eq!(
        app.ui_state.review_form().and_then(|form| form.file_count),
        Some(1)
    );
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    assert!(app.ui_state.review_form().is_some_and(|form| form.busy));
    settle(app).await;
    assert!(app.ui_state.review_form().is_none());

    let Ok(BridgeCommand::Workflow {
        op: WorkflowOp::New { target, inputs },
        workspace_paths,
        ..
    }) = next_new(app, rx).await
    else {
        panic!("Enter must send workflow/new");
    };
    assert_eq!(
        target,
        WorkflowRunTarget::RecipeFile(repo.home.join(".kiro/workflows/cyril-review.workflow.json")),
        "the recipe goes by absolute path so no workspace recipe can shadow it"
    );
    assert_eq!(workspace_paths, vec![repo.root.clone()]);
    assert_eq!(inputs["target"], "auto");
    assert_eq!(inputs["crtool"], prefix().as_str());
    let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
    assert!(
        run_dir.join("manifest.json").is_file(),
        "gathered before New"
    );
    assert!(rx.try_recv().is_err(), "no Invoke before the run is minted");

    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    assert!(
        rx.try_recv().is_err(),
        "no Invoke before run.json is written"
    );
    settle(app).await;
    assert!(matches!(
        rx.try_recv(),
        Ok(BridgeCommand::Workflow { op: WorkflowOp::Invoke { id }, .. }) if id == run_id()
    ));
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(run_dir.join("run.json")).expect("run.json"))
            .expect("run.json parses");
    assert_eq!(record["workflow_id"], RUN);
    assert_eq!(record["crtool_prefix"], prefix().as_str());
    assert_eq!(record["target"], "auto");
    assert_eq!(record["scope"], serde_json::json!(["."]));

    app.handle_notification(outcome(WorkflowCommandOutcome::Invoked {
        workflow_id: run_id(),
    }));
    assert!(last_message(app).contains(&format!("review: {RUN} is running")));
    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(RUN)));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        RUN,
        "alpha",
        &SessionId::new("sess_step"),
    )));
    run_dir
}

#[tokio::test]
async fn refuses_below_the_repository_root() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.cwd = repo.root.join("src");
    app.handle_command_result(CommandResult::review());
    assert!(
        app.ui_state
            .review_form()
            .is_some_and(|form| form.file_count.is_none()),
        "the form opens at once, counting"
    );
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    assert_eq!(
        last_message(&app),
        format!("run /review from the repo root ({})", repo.root.display())
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn esc_closes_the_form_without_writes() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    let form = app.ui_state.review_form().expect("the form is open");
    assert_eq!(form.file_count, Some(1));
    assert_eq!(form.target, cyril_core::review::target::ReviewTarget::Auto);
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    assert!(app.ui_state.review_form().is_none());
    assert!(!repo.root.join(".code-review").exists());
    assert!(!repo.home.join(".kiro").exists());
    assert!(rx.try_recv().is_err(), "no workflow is created");
}

#[tokio::test]
async fn an_empty_diff_closes_with_nothing_to_review() {
    let repo = repo(false);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    assert_eq!(last_message(&app), "review: nothing to review — auto in .");
    assert!(!repo.root.join(".code-review").exists());
    assert!(!repo.home.join(".kiro").exists());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn an_armed_run_decides_its_own_steps_and_reports_its_findings() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    let run_dir = launch(&mut app, &mut rx, &repo).await;

    // A read in the workspace: allowed once, never always.
    let (request, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(answered(answer).await.as_deref(), Some("AllowOnce"));

    // An arbitrary command: rejected, logged, not prompted.
    let (request, answer) = step_request("sess_step", shell("rm -rf src"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
    let log = fs::read_to_string(run_dir.join("denied.log")).expect("denied.log");
    assert!(log.starts_with("sess_step\tshell rm -rf src"), "{log}");

    // Allowed, but no allow-once option on offer: cancel, never "always".
    let (request, answer) = step_request(
        "sess_step",
        read("src/a.rs"),
        &[PermissionOptionKind::AllowAlways],
    );
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(answered(answer).await, None);

    // The main session and a foreign workflow keep ordinary approval.
    let (main, _main_answer) = step_request("sess_main", read("src/a.rs"), &KAS_OPTIONS);
    assert!(app.route_review_permission(main).is_some());
    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
        "wf_other",
    )));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        "wf_other",
        "alpha",
        &SessionId::new("sess_other"),
    )));
    let (foreign, _foreign_answer) = step_request("sess_other", read("src/a.rs"), &KAS_OPTIONS);
    assert!(app.route_review_permission(foreign).is_some());
    let (unclaimed, _unclaimed_answer) =
        step_request("sess_nobody", read("src/a.rs"), &KAS_OPTIONS);
    assert!(app.route_review_permission(unclaimed).is_some());

    // One review at a time.
    app.handle_command_result(CommandResult::review());
    assert_eq!(
        last_message(&app),
        format!("review: {RUN} is still running — /workflow status {RUN}")
    );
    assert!(app.ui_state.review_form().is_none());

    fs::write(
        run_dir.join("findings.json"),
        serde_json::json!([{
            "id": "C01", "file": "src/a.rs", "line": 1, "summary": "panics",
            "failure_scenario": null, "verdict": "CONFIRMED", "angles": ["a-line-scan"]
        }])
        .to_string(),
    )
    .expect("findings.json");
    app.handle_notification(completion(WorkflowRunStatus::Completed));
    settle(&mut app).await;
    let report = last_message(&app);
    assert!(
        report.starts_with(
            "review complete: 1 finding(s) — 1 CONFIRMED\n[CONFIRMED] panics src/a.rs:1"
        ),
        "{report}"
    );
    assert!(
        report.contains("policy denied 1 request(s): shell rm -rf src ("),
        "{report}"
    );

    // A late request from the ended run is denied, not prompted.
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
    assert!(app.ui_state.approval().is_none());

    // And the next review may start.
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some());
}

#[tokio::test]
async fn missing_findings_are_an_error_not_zero_findings() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    let run_dir = launch(&mut app, &mut rx, &repo).await;
    app.handle_notification(completion(WorkflowRunStatus::Completed));
    settle(&mut app).await;
    assert_eq!(
        last_message(&app),
        format!(
            "review completed, but {} is missing: the review produced no findings file — {}",
            run_dir.join("findings.json").display(),
            run_dir.display()
        )
    );
}

#[tokio::test]
async fn pause_stays_armed_and_failure_disarms() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    let run_dir = launch(&mut app, &mut rx, &repo).await;

    app.handle_notification(completion(WorkflowRunStatus::Paused));
    assert_eq!(
        last_message(&app),
        format!(
            "review paused — /workflow resume {RUN} continues it ({})",
            run_dir.display()
        )
    );
    let (request, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(
        answered(answer).await.as_deref(),
        Some("AllowOnce"),
        "a paused run keeps its authorization"
    );
    app.handle_command_result(CommandResult::review());
    assert_eq!(
        last_message(&app),
        format!("review: {RUN} is paused — /workflow resume {RUN} continues it")
    );
    // A resume re-arms it; a repeated pause report is not announced twice.
    app.handle_notification(outcome(WorkflowCommandOutcome::Resumed {
        workflow_id: run_id(),
        status: Some(WorkflowRunStatus::Running),
    }));
    app.handle_command_result(CommandResult::review());
    assert!(last_message(&app).contains("is still running"));

    app.handle_notification(completion(WorkflowRunStatus::Failed));
    assert_eq!(
        last_message(&app),
        format!("review failed — {}", run_dir.display())
    );
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "a failed run frees /review"
    );
}

#[tokio::test]
async fn failed_steps_end_the_launch_and_withdraw_authorization() {
    // workflow/new fails: the launch ends and a new one may start.
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(matches!(
        next_new(&mut app, &mut rx).await,
        Ok(BridgeCommand::Workflow {
            op: WorkflowOp::New { .. },
            ..
        })
    ));
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow new".into(),
        workflow_id: None,
        code: Some(-32603),
        details: "boom".into(),
    }));
    assert!(
        app.ui_state.messages().iter().any(|message| matches!(
            message.kind(),
            ChatMessageKind::System(text)
                if text == "review: the workflow was not created; nothing was started"
        )),
        "the failure is reported"
    );
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "a new launch may start"
    );
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");

    // workflow/invoke fails: authorization is withdrawn; late requests are
    // denied rather than prompted.
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(next_new(&mut app, &mut rx).await.is_ok(), "New");
    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    settle(&mut app).await;
    assert!(rx.try_recv().is_ok(), "Invoke");
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow invoke".into(),
        workflow_id: None,
        code: None,
        details: "boom".into(),
    }));
    assert!(
        app.ui_state.messages().iter().any(|message| matches!(
            message.kind(),
            ChatMessageKind::System(text)
                if text == &format!("review: {RUN} did not start; its authorization is withdrawn")
        )),
        "the withdrawal is reported"
    );
    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(RUN)));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        RUN,
        "alpha",
        &SessionId::new("sess_step"),
    )));
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
}

#[tokio::test]
async fn an_unrecorded_run_is_never_invoked() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    let Ok(BridgeCommand::Workflow {
        op: WorkflowOp::New { inputs, .. },
        ..
    }) = next_new(&mut app, &mut rx).await
    else {
        panic!("Enter must send workflow/new");
    };
    let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
    // A directory where run.json belongs: the atomic rename cannot land.
    fs::create_dir(run_dir.join("run.json")).expect("block run.json");

    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    settle(&mut app).await;
    assert!(rx.try_recv().is_err(), "no Invoke without a persisted run");
    let text = last_message(&app);
    assert!(
        text.starts_with(&format!("review: {RUN} not started — cannot record it: "))
            && text.ends_with("; its authorization is withdrawn"),
        "{text}"
    );
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some(), "the launch is over");
}

/// KAS loads freshly installed agents from a file watcher. Until it has, a
/// `workflow/new` naming them fails; `/review` retries a bounded number of
/// times without reporting the interim failures, then reports the last one.
#[tokio::test]
async fn unloaded_agents_retry_workflow_new_then_report() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(
        rx.try_recv().is_err(),
        "a fresh install waits for KAS to load the agents before New"
    );
    let not_registered = || {
        outcome(WorkflowCommandOutcome::Failed {
            operation: "workflow new".into(),
            workflow_id: None,
            code: Some(-32603),
            details: "Workflow references custom agent 'cyril-review-clerk' which is not \
                      registered. Registered step agents: wf-coder."
                .into(),
        })
    };
    let failures = |app: &App| {
        app.ui_state
            .messages()
            .iter()
            .filter(|message| {
                matches!(message.kind(), ChatMessageKind::System(text)
                    if text.contains("not registered") || text.contains("was not created"))
            })
            .count()
    };
    for attempt in 0..=3 {
        assert!(
            matches!(
                next_new(&mut app, &mut rx).await,
                Ok(BridgeCommand::Workflow {
                    op: WorkflowOp::New { .. },
                    ..
                })
            ),
            "workflow/new attempt {attempt}"
        );
        app.handle_notification(not_registered());
        if attempt < 3 {
            assert_eq!(failures(&app), 0, "attempt {attempt} is retried silently");
        }
    }
    assert_eq!(
        last_message(&app),
        "review: the workflow was not created; nothing was started"
    );
    assert!(
        failures(&app) >= 2,
        "the agent's error and the review's line"
    );
    assert!(rx.try_recv().is_err(), "no fifth attempt");
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some(), "the launch is over");
}

/// Some endings only arrive as a fetched snapshot (`/workflow status`,
/// `attach`): they end the run like `run_complete`, once.
#[tokio::test]
async fn a_terminal_snapshot_ends_the_run_once() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    let run_dir = launch(&mut app, &mut rx, &repo).await;
    let ended = |app: &App| {
        app.ui_state
            .messages()
            .iter()
            .filter(|message| {
                matches!(message.kind(), ChatMessageKind::System(text)
                    if text.starts_with("review aborted"))
            })
            .count()
    };
    app.handle_notification(RoutedNotification::global(Notification::WorkflowSnapshot(
        Box::new(final_snapshot(WorkflowRunStatus::Aborted)),
    )));
    assert_eq!(
        last_message(&app),
        format!("review aborted — {}", run_dir.display())
    );
    app.handle_notification(completion(WorkflowRunStatus::Aborted));
    assert_eq!(
        ended(&app),
        1,
        "the run_complete after it is not a second ending"
    );
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "the run no longer blocks /review"
    );
}

/// A dead agent can neither finish the run nor answer the launch, so both
/// end; the run's late requests are still denied.
#[tokio::test]
async fn a_disconnect_disarms_the_run() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    launch(&mut app, &mut rx, &repo).await;
    app.handle_notification(RoutedNotification::global(
        Notification::BridgeDisconnected {
            reason: "process exited".into(),
        },
    ));
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some());
}

/// A newer review does not forget an older run: its stragglers stay denied.
#[tokio::test]
async fn an_older_runs_late_requests_stay_denied() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    launch(&mut app, &mut rx, &repo).await;
    fs::write(
        app.review_run_dir_for_tests(&run_id())
            .expect("run A is known")
            .join("findings.json"),
        "[]",
    )
    .expect("findings.json");
    app.handle_notification(completion(WorkflowRunStatus::Completed));
    settle(&mut app).await;

    // Run B is minted and armed.
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(next_new(&mut app, &mut rx).await.is_ok(), "New for run B");
    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: WorkflowId::try_from("wf_second".to_owned()).expect("id"),
        name: "cyril-review".into(),
    }));
    settle(&mut app).await;

    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
}

/// Esc after Enter abandons the launch; the launch steps' late result is
/// dropped rather than creating a workflow.
#[tokio::test]
async fn esc_while_preparing_abandons_the_launch() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    assert!(app.ui_state.review_form().is_none());
    assert!(last_message(&app).starts_with("review: abandoned"));
    settle(&mut app).await;
    app.review.flush_delays_for_tests().await;
    assert!(
        rx.try_recv().is_err(),
        "no workflow after an abandoned launch"
    );
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some());
}

/// Blocking work that panics reports back instead of leaving the form
/// waiting forever.
#[tokio::test]
async fn a_panicking_launch_step_is_reported() {
    let repo = repo(true);
    let (mut app, _rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    // A panic in work that belongs to no launch is logged, not fatal to one.
    app.review.spawn(|| panic!("unrelated"));
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "an unrelated panic abandons nothing"
    );
    let launch = app.review.current_launch().expect("a launch");
    app.review.spawn_step(launch, || panic!("boom"));
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    assert!(
        last_message(&app).starts_with("review: internal error"),
        "{}",
        last_message(&app)
    );
}

// --- cyril-305w: repository settings and the preflight check ---

/// Write `.cyril/config.toml` with a `[review]` table built from `pairs`.
fn configure(repo: &Repo, pairs: &[(&str, toml::Value)]) {
    let mut review = toml::Table::new();
    for (key, value) in pairs {
        review.insert((*key).to_owned(), value.clone());
    }
    let mut document = toml::Table::new();
    document.insert("review".to_owned(), toml::Value::Table(review));
    fs::create_dir_all(repo.root.join(".cyril")).expect(".cyril");
    fs::write(
        repo.root.join(".cyril/config.toml"),
        toml::to_string(&document).expect("toml"),
    )
    .expect("config");
}

fn check_cmd(command: &str) -> (&'static str, toml::Value) {
    ("check_cmd", toml::Value::String(command.to_owned()))
}

/// Prints `early`, then runs for 30 seconds.
fn slow_command() -> &'static str {
    if cfg!(windows) {
        "powershell -NoProfile -Command \"Write-Output early; Start-Sleep -Seconds 30\""
    } else {
        "sh -c 'echo early; sleep 30'"
    }
}

fn manifest(run_dir: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).expect("manifest"))
        .expect("manifest parses")
}

/// `/review`, the form settled, Enter pressed, and the launch steps done:
/// the check (if any) is running, or `workflow/new` is due.
async fn confirm(app: &mut App) {
    app.handle_command_result(CommandResult::review());
    settle(app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(app).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_form_shows_the_configured_check_and_scope_without_running_it() {
    let repo = repo(true);
    fs::write(repo.root.join("README.md"), "changed\n").expect("untracked");
    configure(
        &repo,
        &[
            check_cmd("git diff --name-only HEAD"),
            ("check_timeout_s", toml::Value::Integer(90)),
            (
                "scope",
                toml::Value::Array(vec![toml::Value::String("src".into())]),
            ),
        ],
    );
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    let form = app.ui_state.review_form().expect("form").clone();
    assert_eq!(form.scope, ["src"]);
    assert_eq!(form.file_count, Some(1));
    assert_eq!(
        form.check,
        cyril_ui::traits::ReviewCheck::WillRun {
            command: "git diff --name-only HEAD".into(),
            timeout_secs: 90,
        }
    );
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    assert!(
        !repo.root.join(".code-review").exists(),
        "opening ran nothing"
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clean_or_red_check_is_evidence_and_the_review_goes_on() {
    for (command, status) in [
        ("git diff --name-only HEAD", "clean"),
        ("git rev-parse --verify no-such-ref", "FAILED (exit 128)"),
    ] {
        let repo = repo(true);
        configure(&repo, &[check_cmd(command)]);
        let (mut app, mut rx) = review_app(&repo);
        confirm(&mut app).await;
        assert!(
            app.ui_state.review_form().is_some_and(|form| form.busy),
            "the form stays up while the check runs"
        );
        assert_eq!(last_message(&app), "review: running check…");
        settle(&mut app).await;
        assert!(app.ui_state.review_form().is_none());
        let Ok(BridgeCommand::Workflow {
            op: WorkflowOp::New { inputs, .. },
            ..
        }) = next_new(&mut app, &mut rx).await
        else {
            panic!("a {status} check still creates the workflow");
        };
        let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
        assert_eq!(manifest(&run_dir)["facts"]["diagnostics_status"], status);
        assert!(run_dir.join("facts/diagnostics.txt").is_file());
        assert!(
            app.ui_state.messages().iter().any(|message| matches!(
                message.kind(),
                ChatMessageKind::System(text)
                    if text == &format!("review: check {status} — recorded for the reviewers")
            )),
            "{status}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_timed_out_check_is_evidence_too() {
    let repo = repo(true);
    configure(
        &repo,
        &[
            check_cmd(slow_command()),
            ("check_timeout_s", toml::Value::Integer(1)),
        ],
    );
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    settle(&mut app).await;
    let Ok(BridgeCommand::Workflow {
        op: WorkflowOp::New { inputs, .. },
        ..
    }) = next_new(&mut app, &mut rx).await
    else {
        panic!("a timed-out check still creates the workflow");
    };
    let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
    assert_eq!(
        manifest(&run_dir)["facts"]["diagnostics_status"],
        "TIMED OUT"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_check_that_cannot_start_stops_before_any_workflow() {
    let repo = repo(true);
    configure(&repo, &[check_cmd("cyril-no-such-program-x")]);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    let text = last_message(&app);
    assert!(
        text.starts_with("review: the check could not run — ")
            && text.ends_with("; nothing was started"),
        "{text}"
    );
    app.review.flush_delays_for_tests().await;
    assert!(rx.try_recv().is_err(), "no workflow");
}

#[tokio::test(flavor = "multi_thread")]
async fn esc_during_the_check_kills_it_and_abandons_the_launch() {
    let repo = repo(true);
    configure(&repo, &[check_cmd(slow_command())]);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    assert_eq!(last_message(&app), "review: running check…");
    let started = std::time::Instant::now();
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    assert!(app.ui_state.review_form().is_none());
    // The check reports back promptly (killed, not run to its 30 s end), and
    // its late result creates nothing.
    settle(&mut app).await;
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the check was killed"
    );
    app.review.flush_delays_for_tests().await;
    assert!(rx.try_recv().is_err(), "no workflow after cancelling");
    let runs = repo.root.join(".code-review");
    let run_dir = fs::read_dir(&runs)
        .expect("runs")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .expect("a run directory");
    assert_eq!(
        manifest(&run_dir)["facts"]["diagnostics_status"],
        "CANCELLED"
    );
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_some(), "/review works again");
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_settings_refuse_the_review_by_name() {
    let repo = repo(true);
    configure(&repo, &[("chek_cmd", toml::Value::String("x".into()))]);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    assert_eq!(
        last_message(&app),
        "review: [review] in .cyril/config.toml: unknown key `chek_cmd`"
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn context_file_is_reread_for_every_review() {
    let repo = repo(true);
    configure(
        &repo,
        &[("context_file", toml::Value::String("REVIEW.md".into()))],
    );
    let (mut app, mut rx) = review_app(&repo);
    let mut contexts = Vec::new();
    for text in ["first authorities", "second authorities"] {
        fs::write(repo.root.join("REVIEW.md"), text).expect("context");
        confirm(&mut app).await;
        let Ok(BridgeCommand::Workflow {
            op: WorkflowOp::New { inputs, .. },
            ..
        }) = next_new(&mut app, &mut rx).await
        else {
            panic!("New");
        };
        contexts.push(inputs["context"].as_str().expect("context").to_owned());
        app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
            operation: "workflow new".into(),
            workflow_id: None,
            code: None,
            details: "end this launch".into(),
        }));
    }
    assert_eq!(contexts, ["first authorities", "second authorities"]);
}

/// An abandoned launch's late check result cannot drive a newer launch. A's
/// result is held back and delivered once B is checking: the ordering that
/// a slow kill (a grandchild holding the pipes) produces.
#[tokio::test(flavor = "multi_thread")]
async fn a_stale_check_result_cannot_drive_a_newer_launch() {
    let repo = repo(true);
    configure(&repo, &[check_cmd(slow_command())]);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    let stale = tokio::time::timeout(Duration::from_secs(30), app.review.rx.recv())
        .await
        .expect("A's cancelled check reports")
        .expect("open channel");
    confirm(&mut app).await;
    assert_eq!(last_message(&app), "review: running check…");
    app.handle_review_task(stale).await;
    assert!(
        app.ui_state.review_form().is_some_and(|form| form.busy),
        "launch B is still waiting for its own check"
    );
    app.review.flush_delays_for_tests().await;
    assert!(rx.try_recv().is_err(), "no workflow from A's late result");
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc B");
}

/// Esc while the settings are still being read closes the form; the late
/// result is dropped.
#[tokio::test(flavor = "multi_thread")]
async fn esc_while_opening_closes_and_drops_the_late_result() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    assert_eq!(
        app.ui_state.review_form().map(|form| form.check.clone()),
        Some(cyril_ui::traits::ReviewCheck::Reading)
    );
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    settle(&mut app).await;
    assert!(app.ui_state.review_form().is_none());
    assert!(rx.try_recv().is_err());
}

// --- cyril-9akg: HEAD-anchored targets ---

/// `main` holds a.rs and b.rs. `feature` (checked out) changes b.rs in one
/// commit and adds c.rs in the commit at HEAD; a.rs is edited uncommitted.
fn history() -> Repo {
    let repo = repo(false);
    fs::write(repo.root.join("src/b.rs"), "fn b() {}\n").expect("b.rs");
    git(&repo.root, &["add", "-A"]);
    git(&repo.root, &["commit", "-q", "-m", "b on main"]);
    git(&repo.root, &["checkout", "-q", "-b", "feature"]);
    fs::write(repo.root.join("src/b.rs"), "fn b() { todo!() }\n").expect("b.rs");
    git(&repo.root, &["commit", "-q", "-am", "branch change"]);
    fs::write(repo.root.join("src/c.rs"), "fn c() {}\n").expect("c.rs");
    git(&repo.root, &["add", "-A"]);
    git(&repo.root, &["commit", "-q", "-m", "head commit"]);
    fs::write(repo.root.join("src/a.rs"), "fn a() { /* wip */ }\n").expect("a.rs");
    repo
}

fn form(app: &App) -> cyril_ui::traits::ReviewForm {
    app.ui_state
        .review_form()
        .expect("the form is open")
        .clone()
}

/// Press `code` on the form and wait for the recount (or the problem).
async fn change(app: &mut App, code: KeyCode) {
    app.handle_key(key(code)).await.expect("key");
    assert_eq!(form(app).file_count, None, "a change recounts");
    for _ in 0..3 {
        let shown = form(app);
        if shown.file_count.is_some() || shown.problem.is_some() {
            return;
        }
        settle(app).await;
    }
    panic!("the recount never landed");
}

#[tokio::test(flavor = "multi_thread")]
async fn each_target_counts_gathers_and_sends_its_own_diff() {
    let repo = history();
    let (mut app, mut rx) = review_app(&repo);
    let expectations: [(ReviewTargetCase, &str, &[&str]); 4] = [
        (
            ReviewTargetCase::Auto,
            "auto",
            &["src/a.rs", "src/b.rs", "src/c.rs"],
        ),
        (
            ReviewTargetCase::Base,
            "main...HEAD",
            &["src/b.rs", "src/c.rs"],
        ),
        (ReviewTargetCase::Uncommitted, "HEAD", &["src/a.rs"]),
        (ReviewTargetCase::HeadCommit, "HEAD~1..HEAD", &["src/c.rs"]),
    ];
    for (presses, (case, spec, files)) in expectations.into_iter().enumerate() {
        app.handle_command_result(CommandResult::review());
        settle(&mut app).await;
        assert_eq!(form(&app).branches, ["main"], "feature is checked out");
        for _ in 0..presses {
            change(&mut app, KeyCode::Right).await;
        }
        let shown = form(&app);
        assert_eq!(shown.target.spec(), spec, "{case:?}");
        assert_eq!(shown.file_count, Some(files.len()), "{case:?} count");
        assert_eq!(shown.shows_base(), case == ReviewTargetCase::Base);

        app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
        settle(&mut app).await;
        let Ok(BridgeCommand::Workflow {
            op: WorkflowOp::New { inputs, .. },
            ..
        }) = next_new(&mut app, &mut rx).await
        else {
            panic!("{case:?} creates the workflow");
        };
        assert_eq!(
            inputs["target"], spec,
            "{case:?}: the workflow gets the form's target"
        );
        let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
        let gathered = fs::read_to_string(run_dir.join("changed-files.txt")).expect("files");
        assert_eq!(gathered.lines().collect::<Vec<_>>(), files, "{case:?}");
        assert_eq!(manifest(&run_dir)["requested_target"], spec, "{case:?}");
        app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
            operation: "workflow new".into(),
            workflow_id: None,
            code: None,
            details: "end this launch".into(),
        }));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewTargetCase {
    Auto,
    Base,
    Uncommitted,
    HeadCommit,
}

#[tokio::test(flavor = "multi_thread")]
async fn the_base_field_takes_focus_only_in_base_mode() {
    let repo = history();
    git(&repo.root, &["branch", "release", "main"]);
    let (mut app, _rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Tab)).await.expect("Tab");
    assert_eq!(
        form(&app).focus,
        cyril_ui::traits::ReviewField::Target,
        "no base field in auto mode"
    );
    change(&mut app, KeyCode::Right).await;
    assert_eq!(form(&app).target.spec(), "main...HEAD");
    app.handle_key(key(KeyCode::Tab)).await.expect("Tab");
    assert_eq!(form(&app).focus, cyril_ui::traits::ReviewField::Base);
    change(&mut app, KeyCode::Right).await;
    assert_eq!(form(&app).target.spec(), "release...HEAD");
    // Leaving base mode moves the focus back with it.
    app.handle_key(key(KeyCode::Tab)).await.expect("Tab");
    change(&mut app, KeyCode::Right).await;
    assert_eq!(form(&app).target.spec(), "HEAD");
    assert_eq!(form(&app).focus, cyril_ui::traits::ReviewField::Target);
    // Coming back to base mode keeps the branch picked before.
    change(&mut app, KeyCode::Left).await;
    assert_eq!(form(&app).target.spec(), "release...HEAD");
    assert!(
        !repo.root.join(".code-review").exists(),
        "changing modes writes nothing"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_target_that_cannot_be_diffed_is_reported_and_refused() {
    // One commit: HEAD has no parent, so "the commit at HEAD" has no diff base.
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    // auto → (no base branch: main is checked out) uncommitted → HEAD commit.
    change(&mut app, KeyCode::Right).await;
    change(&mut app, KeyCode::Right).await;
    let shown = form(&app);
    assert_eq!(shown.target.spec(), "HEAD~1..HEAD");
    let problem = shown.problem.expect("the problem is shown");
    assert!(problem.contains("HEAD~1"), "{problem}");
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    assert!(
        !form(&app).busy,
        "Enter is refused while the target has a problem"
    );
    app.review.flush_delays_for_tests().await;
    assert!(rx.try_recv().is_err());
    // Choosing a reviewable target clears it.
    change(&mut app, KeyCode::Left).await;
    assert_eq!(form(&app).problem, None);
    assert_eq!(form(&app).file_count, Some(1));
}

/// A target that ends at the HEAD commit says so when scope has uncommitted
/// changes the reviewers will read; auto and uncommitted include them.
#[tokio::test(flavor = "multi_thread")]
async fn commit_ending_targets_note_uncommitted_changes() {
    let repo = history();
    let (mut app, _rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert_eq!(form(&app).note, None, "auto includes the working tree");
    let mut notes = Vec::new();
    for _ in 0..3 {
        change(&mut app, KeyCode::Right).await;
        notes.push((form(&app).target.spec(), form(&app).note.is_some()));
    }
    assert_eq!(
        notes,
        [
            ("main...HEAD".to_owned(), true),
            ("HEAD".to_owned(), false),
            ("HEAD~1..HEAD".to_owned(), true),
        ]
    );
}

// --- cyril-2ruj: /review resume ---

/// A run directory with its record and a manifest, as a launch leaves it.
fn write_run(repo: &Repo, name: &str, workflow_id: &str) -> PathBuf {
    write_run_with(
        repo,
        name,
        workflow_id,
        prefix().as_str(),
        env!("CARGO_PKG_VERSION"),
    )
}

fn write_run_with(
    repo: &Repo,
    name: &str,
    workflow_id: &str,
    crtool_prefix: &str,
    cyril_version: &str,
) -> PathBuf {
    let dir = repo.root.join(".code-review").join(name);
    fs::create_dir_all(&dir).expect("run dir");
    cyril_core::review::run_record::RunRecord {
        workflow_id: workflow_id.to_owned(),
        crtool_prefix: crtool_prefix.to_owned(),
        cyril_version: cyril_version.to_owned(),
        target: "main...HEAD".to_owned(),
        scope: vec!["src".to_owned()],
    }
    .write(&dir)
    .expect("run.json");
    fs::write(
        dir.join("manifest.json"),
        r#"{"total_files": 2, "requested_target": "main...HEAD"}"#,
    )
    .expect("manifest");
    dir
}

fn runs(rows: &[(&str, WorkflowRunStatus)]) -> RoutedNotification {
    outcome(WorkflowCommandOutcome::Runs {
        runs: rows
            .iter()
            .map(|(id, status)| cyril_core::types::WorkflowRunSummary {
                workflow_id: WorkflowId::try_from((*id).to_owned()).expect("id"),
                name: "cyril-review".to_owned(),
                status: *status,
                created_at: None,
                updated_at: None,
                started_at: None,
                ended_at: None,
                parent_session_id: None,
            })
            .collect(),
        skipped: 0,
    })
}

/// `/review resume <selector>` through the listing: returns the chat's
/// last line, and asserts the run table was not shown.
async fn resume_to_form(
    app: &mut App,
    rx: &mut tokio::sync::mpsc::Receiver<BridgeCommand>,
    selector: Option<&str>,
    listing: &[(&str, WorkflowRunStatus)],
) {
    app.handle_command_result(CommandResult::review_resume(selector.map(str::to_owned)));
    settle(app).await;
    assert!(
        matches!(
            rx.try_recv(),
            Ok(BridgeCommand::Workflow {
                op: WorkflowOp::ListRuns,
                ..
            })
        ),
        "resume asks the agent for the runs' status"
    );
    let before = app.ui_state.messages().len();
    app.handle_notification(runs(listing));
    assert_eq!(
        app.ui_state.messages().len(),
        before,
        "the listing is absorbed, not shown"
    );
    settle(app).await;
}

fn resume_view(app: &App) -> Option<cyril_ui::traits::ReviewResumeView> {
    app.ui_state
        .review_form()
        .and_then(|form| form.resume.clone())
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_picks_the_newest_failed_or_paused_run_and_retries_it_after_loading() {
    let repo = repo(true);
    let _paused = write_run(&repo, "20261001-090000-aaaa", "wf_a");
    let failed = write_run(&repo, "20261001-100000-bbbb", "wf_b");
    write_run(&repo, "20261001-120000-cccc", "wf_c");
    let manifest_before = fs::read(failed.join("manifest.json")).expect("manifest");
    let (mut app, mut rx) = review_app(&repo);
    resume_to_form(
        &mut app,
        &mut rx,
        None,
        &[
            ("wf_a", WorkflowRunStatus::Paused),
            ("wf_b", WorkflowRunStatus::Failed),
            ("wf_c", WorkflowRunStatus::Completed),
        ],
    )
    .await;
    let view = resume_view(&app).expect("the read-only resume form");
    assert_eq!(
        view.run, "20261001-100000-bbbb",
        "newest resumable, not newest dir"
    );
    assert_eq!(view.status, WorkflowRunStatus::Failed);
    assert_eq!(view.target, "main...HEAD");
    assert_eq!(form(&app).file_count, Some(2));
    assert!(
        !repo.home.join(".kiro").exists(),
        "nothing installed before Enter"
    );

    // Arrows do nothing on a read-only form.
    app.handle_key(key(KeyCode::Right)).await.expect("Right");
    assert_eq!(
        resume_view(&app).map(|view| view.run),
        Some(view.run.clone())
    );

    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await; // installed
    settle(&mut app).await; // proceed after the agent-load wait
    let Ok(BridgeCommand::Workflow {
        op: WorkflowOp::Load { id },
        ..
    }) = rx.try_recv()
    else {
        panic!("a run this process does not hold is loaded first");
    };
    assert_eq!(id.as_str(), "wf_b");
    assert!(
        repo.home
            .join(".kiro/workflows/cyril-review.workflow.json")
            .is_file(),
        "assets installed after consent"
    );
    let wf_b = WorkflowId::try_from("wf_b".to_owned()).expect("id");
    app.handle_notification(outcome(WorkflowCommandOutcome::Loaded {
        workflow_id: wf_b.clone(),
        status: WorkflowRunStatus::Failed,
    }));
    settle(&mut app).await;
    assert!(
        matches!(
            rx.try_recv(),
            Ok(BridgeCommand::Workflow { op: WorkflowOp::Retry { id }, .. }) if id == wf_b
        ),
        "a failed run is retried"
    );
    app.handle_notification(outcome(WorkflowCommandOutcome::Retried {
        workflow_id: wf_b.clone(),
        status: Some(WorkflowRunStatus::Running),
    }));
    assert_eq!(
        last_message(&app),
        "review: wf_b continues — /workflow status wf_b follows it"
    );
    assert_eq!(
        fs::read(failed.join("manifest.json")).expect("manifest"),
        manifest_before,
        "nothing is gathered again"
    );
    // The resumed run is armed with its stored run directory.
    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
        "wf_b",
    )));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        "wf_b",
        "alpha",
        &SessionId::new("sess_b"),
    )));
    let (request, answer) = step_request("sess_b", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(answered(answer).await.as_deref(), Some("AllowOnce"));
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_refusals_name_their_reason() {
    let repo = repo(true);
    write_run(&repo, "20261001-120000-cccc", "wf_c");
    write_run_with(
        &repo,
        "20261001-130000-dddd",
        "wf_d",
        "\"/elsewhere/cyril\" crtool",
        env!("CARGO_PKG_VERSION"),
    );
    write_run_with(
        &repo,
        "20261001-140000-eeee",
        "wf_e",
        prefix().as_str(),
        "0.0.1",
    );
    fs::create_dir_all(repo.root.join(".code-review/20261001-150000-ffff")).expect("dir");
    fs::write(
        repo.root.join(".code-review/20261001-150000-ffff/run.json"),
        "{",
    )
    .expect("corrupt");
    let (mut app, mut rx) = review_app(&repo);
    let listing = [
        ("wf_c", WorkflowRunStatus::Completed),
        ("wf_d", WorkflowRunStatus::Failed),
        ("wf_e", WorkflowRunStatus::Paused),
    ];
    for (selector, expected) in [
        (
            "wf_c",
            "review resume: wf_c is completed; there is nothing to resume",
        ),
        (
            "wf_d",
            "review resume: run was started by a different cyril binary (\"/elsewhere/cyril\" crtool); start a new /review",
        ),
        ("20261001-150000-ffff", "is corrupt"),
        (
            "nope",
            "review resume: no run under .code-review/ matches \"nope\"",
        ),
    ] {
        resume_to_form(&mut app, &mut rx, Some(selector), &listing).await;
        assert!(resume_view(&app).is_none(), "{selector}: no form");
        assert!(
            last_message(&app).contains(expected),
            "{selector}: {}",
            last_message(&app)
        );
    }
    resume_to_form(&mut app, &mut rx, Some("wf_e"), &listing).await;
    assert!(
        last_message(&app).starts_with("review resume: run was started by cyril 0.0.1"),
        "{}",
        last_message(&app)
    );
    assert!(!repo.home.join(".kiro").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_run_is_reconfirmed_in_process_and_resumed() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    let run_dir = launch(&mut app, &mut rx, &repo).await;
    app.handle_notification(completion(WorkflowRunStatus::Paused));
    let name = run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name")
        .to_owned();

    // Esc leaves the paused authorization as it was.
    resume_to_form(&mut app, &mut rx, None, &[(RUN, WorkflowRunStatus::Paused)]).await;
    assert_eq!(resume_view(&app).map(|view| view.run), Some(name.clone()));
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    assert!(app.ui_state.review_form().is_none());
    let (request, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    settle(&mut app).await;
    assert_eq!(
        answered(answer).await.as_deref(),
        Some("AllowOnce"),
        "still armed"
    );

    // Enter loads it (KAS answers a load for a run it holds; it makes this
    // session the parent) and resumes it by the status the load reports.
    resume_to_form(
        &mut app,
        &mut rx,
        Some(&name),
        &[(RUN, WorkflowRunStatus::Paused)],
    )
    .await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    settle(&mut app).await;
    assert!(
        matches!(
            rx.try_recv(),
            Ok(BridgeCommand::Workflow { op: WorkflowOp::Load { id }, .. }) if id == run_id()
        ),
        "the run is always loaded"
    );
    app.handle_notification(outcome(WorkflowCommandOutcome::Loaded {
        workflow_id: run_id(),
        status: WorkflowRunStatus::Paused,
    }));
    settle(&mut app).await;
    assert!(
        matches!(
            rx.try_recv(),
            Ok(BridgeCommand::Workflow { op: WorkflowOp::Resume { id }, .. }) if id == run_id()
        ),
        "a paused run is resumed"
    );
    // Another run's failure says nothing about this one.
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow resume".into(),
        workflow_id: Some(WorkflowId::try_from("wf_other".to_owned()).expect("id")),
        code: None,
        details: "not ours".into(),
    }));
    assert!(!last_message(&app).contains("withdrawn"));
    // A refused continuation withdraws the authorization (as the ticket asks).
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow resume".into(),
        workflow_id: Some(run_id()),
        code: None,
        details: "no".into(),
    }));
    assert!(last_message(&app).contains("its authorization is withdrawn"));
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_refuses_while_another_review_is_running() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    launch(&mut app, &mut rx, &repo).await;
    write_run(&repo, "20200101-000000-0000", "wf_old");
    resume_to_form(
        &mut app,
        &mut rx,
        Some("wf_old"),
        &[
            (RUN, WorkflowRunStatus::Running),
            ("wf_old", WorkflowRunStatus::Failed),
        ],
    )
    .await;
    assert!(resume_view(&app).is_none());
    assert_eq!(
        last_message(&app),
        format!("review resume: {RUN} is still running — /workflow status {RUN}")
    );
}

/// A retry refused only because KAS has not loaded the review agents yet is
/// sent again, not reported as a failure.
#[tokio::test(flavor = "multi_thread")]
async fn resume_resends_when_the_review_agents_are_not_loaded_yet() {
    let repo = repo(true);
    write_run(&repo, "20261001-100000-bbbb", "wf_b");
    let (mut app, mut rx) = review_app(&repo);
    resume_to_form(
        &mut app,
        &mut rx,
        None,
        &[("wf_b", WorkflowRunStatus::Failed)],
    )
    .await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    settle(&mut app).await;
    assert!(rx.try_recv().is_ok(), "Load");
    let wf_b = WorkflowId::try_from("wf_b".to_owned()).expect("id");
    app.handle_notification(outcome(WorkflowCommandOutcome::Loaded {
        workflow_id: wf_b.clone(),
        status: WorkflowRunStatus::Failed,
    }));
    settle(&mut app).await;
    assert!(rx.try_recv().is_ok(), "Retry");
    let before = app.ui_state.messages().len();
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow retry".into(),
        workflow_id: Some(wf_b.clone()),
        code: Some(-32603),
        details: "Workflow references custom agent 'cyril-review-clerk' which is not registered."
            .into(),
    }));
    assert_eq!(
        app.ui_state.messages().len(),
        before,
        "absorbed, not reported"
    );
    settle(&mut app).await;
    assert!(
        matches!(
            rx.try_recv(),
            Ok(BridgeCommand::Workflow { op: WorkflowOp::Retry { id }, .. }) if id == wf_b
        ),
        "sent again"
    );
}

// --- cyril-4o1u: /review cancel ---

fn cancel_sent(rx: &mut tokio::sync::mpsc::Receiver<BridgeCommand>) -> Option<WorkflowId> {
    match rx.try_recv() {
        Ok(BridgeCommand::Workflow {
            op: WorkflowOp::Cancel { id },
            ..
        }) => Some(id),
        _ => None,
    }
}

/// Deliver the scheduled cancel send.
async fn deliver_cancel(app: &mut App) {
    settle(app).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_during_the_check_kills_it_and_starts_nothing() {
    let repo = repo(true);
    configure(&repo, &[check_cmd(slow_command())]);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    assert_eq!(last_message(&app), "review: running check…");
    let started = std::time::Instant::now();
    app.handle_command_result(CommandResult::review_cancel());
    assert!(app.ui_state.review_form().is_none());
    assert_eq!(last_message(&app), "review: cancelled; nothing was started");
    settle(&mut app).await; // the killed check reports, and is dropped
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the check was killed"
    );
    app.review.flush_delays_for_tests().await;
    assert!(rx.try_recv().is_err(), "no workflow");
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "/review is usable again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_while_minting_cancels_the_workflow_when_it_appears() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    let Ok(BridgeCommand::Workflow {
        op: WorkflowOp::New { inputs, .. },
        ..
    }) = next_new(&mut app, &mut rx).await
    else {
        panic!("New");
    };
    let run_dir = native(inputs["rundir"].as_str().expect("rundir"));
    app.handle_command_result(CommandResult::review_cancel());
    assert_eq!(
        last_message(&app),
        "review: cancelled before the workflow started"
    );
    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    deliver_cancel(&mut app).await;
    assert_eq!(
        cancel_sent(&mut rx),
        Some(run_id()),
        "the orphan is cancelled"
    );
    assert!(rx.try_recv().is_err(), "and never invoked");
    assert!(!run_dir.join("run.json").exists(), "nor armed or recorded");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_between_record_and_invoke_withdraws_and_cancels() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    confirm(&mut app).await;
    assert!(next_new(&mut app, &mut rx).await.is_ok(), "New");
    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    settle(&mut app).await; // recorded → Invoke sent
    assert!(rx.try_recv().is_ok(), "Invoke");
    app.handle_command_result(CommandResult::review_cancel());
    deliver_cancel(&mut app).await;
    assert_eq!(cancel_sent(&mut rx), Some(run_id()));
    // A late Invoked starts nothing, and the run's requests are denied.
    app.handle_notification(outcome(WorkflowCommandOutcome::Invoked {
        workflow_id: run_id(),
    }));
    assert!(!last_message(&app).contains("is running"));
    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(RUN)));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        RUN,
        "alpha",
        &SessionId::new("sess_step"),
    )));
    let (late, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, late).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_of_a_running_review_reports_what_the_agent_says() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    launch(&mut app, &mut rx, &repo).await;
    app.handle_command_result(CommandResult::review_cancel());
    assert_eq!(
        last_message(&app),
        format!("review: cancelling {RUN}; its authorization is withdrawn")
    );
    deliver_cancel(&mut app).await;
    assert_eq!(cancel_sent(&mut rx), Some(run_id()));
    // Withdrawn at once: a request racing the cancel is denied.
    let (request, answer) = step_request("sess_step", read("src/a.rs"), &KAS_OPTIONS);
    decide(&mut app, request).await;
    assert_eq!(answered(answer).await.as_deref(), Some("RejectOnce"));
    // The main session keeps ordinary approval.
    let (main, _main_answer) = step_request("sess_main", read("src/a.rs"), &KAS_OPTIONS);
    assert!(app.route_review_permission(main).is_some());

    // A failed cancel is reported as a failure, not success.
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow cancel".into(),
        workflow_id: Some(run_id()),
        code: Some(-32603),
        details: "boom".into(),
    }));
    assert_eq!(
        last_message(&app),
        format!("review: cancelling {RUN} failed — boom; its authorization stays withdrawn")
    );
    app.handle_command_result(CommandResult::review_cancel());
    assert_eq!(last_message(&app), "review: nothing to cancel");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_of_a_paused_review_succeeds_when_the_agent_confirms() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    launch(&mut app, &mut rx, &repo).await;
    app.handle_notification(completion(WorkflowRunStatus::Paused));
    app.handle_command_result(CommandResult::review_cancel());
    deliver_cancel(&mut app).await;
    assert_eq!(cancel_sent(&mut rx), Some(run_id()));
    app.handle_notification(outcome(WorkflowCommandOutcome::Cancelled {
        workflow_id: run_id(),
        previous_status: Some(WorkflowRunStatus::Paused),
    }));
    assert_eq!(last_message(&app), format!("review: {RUN} cancelled"));
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    assert!(
        app.ui_state.review_form().is_some(),
        "a new review may start"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_with_nothing_running_says_so() {
    let repo = repo(true);
    let (mut app, mut rx) = review_app(&repo);
    app.handle_command_result(CommandResult::review_cancel());
    assert_eq!(last_message(&app), "review: nothing to cancel");
    assert!(rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_during_a_resume_drops_it() {
    let repo = repo(true);
    write_run(&repo, "20261001-100000-bbbb", "wf_b");
    let (mut app, mut rx) = review_app(&repo);
    resume_to_form(
        &mut app,
        &mut rx,
        None,
        &[("wf_b", WorkflowRunStatus::Failed)],
    )
    .await;
    assert!(resume_view(&app).is_some());
    app.handle_command_result(CommandResult::review_cancel());
    assert!(app.ui_state.review_form().is_none());
    assert_eq!(
        last_message(&app),
        "review resume: cancelled; nothing was continued"
    );
    assert!(!repo.home.join(".kiro").exists());
}
