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

/// `run_complete` for [`RUN`] with the claimed `alpha` step.
fn completion(status: WorkflowRunStatus) -> RoutedNotification {
    let node_status = match status {
        WorkflowRunStatus::Paused => WorkflowNodeStatus::Paused,
        WorkflowRunStatus::Completed => WorkflowNodeStatus::Completed,
        WorkflowRunStatus::Failed => WorkflowNodeStatus::Failed,
        _ => WorkflowNodeStatus::Aborted,
    };
    let completion_status = match status {
        WorkflowRunStatus::Paused => WorkflowCompletionStatus::Paused,
        WorkflowRunStatus::Completed => WorkflowCompletionStatus::Completed,
        WorkflowRunStatus::Failed => WorkflowCompletionStatus::Failed,
        _ => WorkflowCompletionStatus::Aborted,
    };
    let snapshot = WorkflowSnapshot::new(
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
    );
    let completed =
        WorkflowRunCompleted::new(run_id(), completion_status, snapshot).expect("valid completion");
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
    }) = rx.try_recv()
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
        app.ui_state.review_form().is_some(),
        "the form opens while counting"
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
    assert_eq!(form.target, "auto");
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
        format!("review paused — /review resume ({})", run_dir.display())
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
        rx.try_recv(),
        Ok(BridgeCommand::Workflow {
            op: WorkflowOp::New { .. },
            ..
        })
    ));
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow new".into(),
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
    assert!(
        app.ui_state.review_form().is_some(),
        "a new launch may start"
    );
    app.handle_key(key(KeyCode::Esc)).await.expect("Esc");
    settle(&mut app).await;

    // workflow/invoke fails: authorization is withdrawn; late requests are
    // denied rather than prompted.
    app.handle_command_result(CommandResult::review());
    settle(&mut app).await;
    app.handle_key(key(KeyCode::Enter)).await.expect("Enter");
    settle(&mut app).await;
    assert!(rx.try_recv().is_ok(), "New");
    app.handle_notification(outcome(WorkflowCommandOutcome::Minted {
        workflow_id: run_id(),
        name: "cyril-review".into(),
    }));
    settle(&mut app).await;
    assert!(rx.try_recv().is_ok(), "Invoke");
    app.handle_notification(outcome(WorkflowCommandOutcome::Failed {
        operation: "workflow invoke".into(),
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
    }) = rx.try_recv()
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
    assert!(app.ui_state.review_form().is_some(), "the launch is over");
}
