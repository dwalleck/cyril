//! Cyril's native `/workflow` command family (cyril-0qe6, ADR-0011).
//!
//! The client-owned control plane over `_kiro/workflow/*`: the workflow gate
//! is never set, the model never gains `run_workflow`, and every mutating
//! act below is user-issued. Registered only when the bound engine is KAS —
//! v2 has no workflow surface.

use std::path::PathBuf;

use super::{Command, CommandContext, CommandResult};
use crate::types::{BridgeCommand, WorkflowId, WorkflowOp, parse_run_inputs, parse_run_target};

const USAGE: &str = "Usage: /workflow recipes | list | run <ref> [key=value …] | attach <id> | \
     status [<id>] | cancel <id> | resume <id>\n\
     Refs: a bare name is a bundled recipe; bundled://… and generated://… pass \
     verbatim; anything with a / or a .workflow.json suffix is a recipe file \
     (arguments are whitespace-separated — paths with spaces are not supported).";

/// `/workflow` — drive KAS workflow runs as persisted workspace objects.
pub struct WorkflowCommand {
    /// Sent as `workspacePaths`; the session's `--cwd`, not the process cwd
    /// (same reasoning as `KasHooksCommand`).
    workspace_root: PathBuf,
}

impl WorkflowCommand {
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    fn workspace_paths(&self) -> Vec<PathBuf> {
        vec![self.workspace_root.clone()]
    }

    /// Renders known runs from the tracker — no wire round-trip (C14): after
    /// an attach, status keeps working even while the agent is busy.
    fn tracker_summary(ctx: &CommandContext<'_>) -> CommandResult {
        let Some(tracker) = ctx.workflow_tracker else {
            tracing::error!("workflow tracker missing from CommandContext");
            return CommandResult::system_message(
                "Workflow state is unavailable in this build.".into(),
            );
        };
        if tracker.iter().len() == 0 {
            return CommandResult::system_message(
                "No runs known here yet — /workflow list asks the agent, \
                 /workflow attach <id> follows one."
                    .into(),
            );
        }
        let mut lines: Vec<String> = tracker
            .iter()
            .map(|(id, run)| {
                let status = run
                    .status()
                    .map_or_else(|| "unknown".to_owned(), |status| status.to_string());
                format!("  {id}  {status}  {}", run.workflow_name())
            })
            .collect();
        lines.sort();
        let mut text = format!("Known workflow runs ({}):\n", lines.len());
        text.push_str(&lines.join("\n"));
        text.push_str("\n(/workflow status <id> refreshes one from the agent)");
        CommandResult::system_message(text)
    }
}

/// Parses an id argument or explains what was expected.
fn parse_id(argument: Option<&str>, verb: &str) -> Result<WorkflowId, CommandResult> {
    let Some(raw) = argument else {
        return Err(CommandResult::system_message(format!(
            "Which run? Usage: /workflow {verb} <workflow-id>"
        )));
    };
    WorkflowId::try_from(raw.to_owned()).map_err(|error| {
        CommandResult::system_message(format!("Invalid workflow id {raw:?}: {error}"))
    })
}

#[async_trait::async_trait]
impl Command for WorkflowCommand {
    fn name(&self) -> &str {
        "workflow"
    }

    fn description(&self) -> &str {
        "Drive KAS workflow runs: recipes, list, run, attach, status, cancel, resume"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let mut parts = args.split_whitespace();
        let Some(subcommand) = parts.next() else {
            return Ok(CommandResult::system_message(USAGE.into()));
        };

        // Tracker-only path first: no session, no wire (C14).
        if subcommand == "status" && parts.clone().next().is_none() {
            return Ok(Self::tracker_summary(ctx));
        }

        let op = match subcommand {
            "recipes" => WorkflowOp::ListRecipes,
            "list" => WorkflowOp::ListRuns,
            "run" => {
                let Some(reference) = parts.next() else {
                    return Ok(CommandResult::system_message(format!(
                        "Which recipe? {USAGE}"
                    )));
                };
                let target = parse_run_target(&self.workspace_root, reference);
                let inputs = match parse_run_inputs(parts) {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        return Ok(CommandResult::system_message(format!("{error}. {USAGE}")));
                    }
                };
                WorkflowOp::Run { target, inputs }
            }
            "attach" => match parse_id(parts.next(), "attach") {
                Ok(id) => WorkflowOp::Attach { id },
                Err(message) => return Ok(message),
            },
            "status" => match parse_id(parts.next(), "status") {
                Ok(id) => WorkflowOp::Status { id },
                Err(message) => return Ok(message),
            },
            "cancel" => match parse_id(parts.next(), "cancel") {
                Ok(id) => WorkflowOp::Cancel { id },
                Err(message) => return Ok(message),
            },
            "resume" => match parse_id(parts.next(), "resume") {
                Ok(id) => WorkflowOp::Resume { id },
                Err(message) => return Ok(message),
            },
            other => {
                return Ok(CommandResult::system_message(format!(
                    "Unknown /workflow action {other:?}. {USAGE}"
                )));
            }
        };

        let Some(session_id) = ctx.session.id().cloned() else {
            return Ok(CommandResult::system_message(
                "No active session — workflow commands need one.".into(),
            ));
        };
        ctx.bridge
            .send(BridgeCommand::Workflow {
                session_id,
                workspace_paths: self.workspace_paths(),
                op,
            })
            .await?;
        Ok(CommandResult::dispatched())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::commands::CommandResultKind;
    use crate::session::SessionController;
    use crate::types::{Notification, WorkflowRunTarget};
    use crate::workflow::WorkflowTracker;

    struct Harness {
        session: SessionController,
        tracker: WorkflowTracker,
        rx: tokio::sync::mpsc::Receiver<BridgeCommand>,
        bridge: crate::protocol::bridge::BridgeSender,
    }

    fn harness(with_session: bool) -> Harness {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let mut session = SessionController::new();
        if with_session {
            session.apply_notification(&Notification::SessionCreated {
                session_id: crate::types::SessionId::new("sess_x"),
                current_mode: None,
                current_model: None,
                available_modes: Vec::new(),
                available_models: Vec::new(),
            });
        }
        Harness {
            session,
            tracker: WorkflowTracker::new(),
            rx,
            bridge: crate::protocol::bridge::BridgeSender::from_sender(tx),
        }
    }

    fn command() -> WorkflowCommand {
        WorkflowCommand::new(PathBuf::from("/ws"))
    }

    async fn run(harness: &mut Harness, args: &str) -> CommandResult {
        let ctx = CommandContext {
            workspace: std::path::Path::new("."),
            session: &harness.session,
            bridge: &harness.bridge,
            subagent_tracker: None,
            workflow_tracker: Some(&harness.tracker),
            memory_status: None,
        };
        match command().execute(&ctx, args).await {
            Ok(result) => result,
            Err(error) => panic!("command must not error: {error}"),
        }
    }

    fn message_text(result: &CommandResult) -> &str {
        match &result.kind {
            CommandResultKind::SystemMessage(text) => text,
            other => panic!("expected a system message, got {other:?}"),
        }
    }

    fn assert_nothing_sent(harness: &mut Harness) {
        assert!(
            harness.rx.try_recv().is_err(),
            "no BridgeCommand may be sent on this path"
        );
    }

    #[tokio::test]
    async fn empty_and_unknown_subcommands_usage_without_sending() {
        let mut harness = harness(true);
        for args in ["", "frobnicate"] {
            let result = run(&mut harness, args).await;
            assert!(message_text(&result).contains("Usage: /workflow"));
            assert_nothing_sent(&mut harness);
        }
    }

    #[tokio::test]
    async fn id_verbs_without_id_usage_without_sending() {
        let mut harness = harness(true);
        for args in ["attach", "cancel", "resume", "run"] {
            let result = run(&mut harness, args).await;
            let text = message_text(&result);
            assert!(
                text.contains("Usage: /workflow") || text.contains("Which"),
                "usage guidance for {args:?}: {text}"
            );
            assert_nothing_sent(&mut harness);
        }
    }

    #[tokio::test]
    async fn no_session_is_a_message_not_a_send() {
        let mut harness = harness(false);
        let result = run(&mut harness, "list").await;
        assert!(message_text(&result).contains("No active session"));
        assert_nothing_sent(&mut harness);
    }

    /// C14: no-argument status renders from the tracker with zero sends —
    /// the buggy implementation (status always round-trips) breaks offline
    /// status after an attach.
    #[tokio::test]
    async fn status_no_arg_renders_tracker_without_sending() {
        let mut harness = harness(true);
        let result = run(&mut harness, "status").await;
        assert!(message_text(&result).contains("No runs known here yet"));
        assert_nothing_sent(&mut harness);
    }

    /// Slice 8's read-through: a run seeded into the tracker is visible to
    /// no-arg status through CommandContext.
    #[tokio::test]
    async fn status_no_arg_lists_seeded_runs() {
        let mut harness = harness(true);
        let opening = crate::types::WorkflowRunStarted::new(
            match WorkflowId::try_from("wf_seeded".to_owned()) {
                Ok(id) => id,
                Err(error) => panic!("fixture id: {error}"),
            },
            "recipe-x".to_owned(),
            serde_json::json!({}),
            Vec::new(),
            None,
        );
        if let Err(error) = harness
            .tracker
            .apply_event(crate::types::WorkflowEvent::RunStarted(opening))
        {
            panic!("seed must apply: {error}");
        }
        let result = run(&mut harness, "status").await;
        let text = message_text(&result);
        assert!(text.contains("wf_seeded"), "seeded run listed: {text}");
        assert!(text.contains("recipe-x"));
        assert_nothing_sent(&mut harness);
    }

    #[tokio::test]
    async fn run_maps_ref_and_inputs_into_one_bridge_op() {
        let mut harness = harness(true);
        let result = run(&mut harness, "run ralph k=v").await;
        assert!(matches!(result.kind, CommandResultKind::Dispatched));
        let sent = match harness.rx.try_recv() {
            Ok(command) => command,
            Err(error) => panic!("exactly one BridgeCommand expected: {error}"),
        };
        let BridgeCommand::Workflow {
            session_id,
            workspace_paths,
            op: WorkflowOp::Run { target, inputs },
        } = sent
        else {
            panic!("expected a Workflow Run op, got {sent:?}");
        };
        assert_eq!(session_id.as_str(), "sess_x");
        assert_eq!(workspace_paths, vec![PathBuf::from("/ws")]);
        assert_eq!(
            target,
            WorkflowRunTarget::Reference("bundled://ralph".into())
        );
        assert_eq!(inputs["k"], "v");
        assert_nothing_sent(&mut harness);
    }

    #[tokio::test]
    async fn bad_input_token_is_a_message_not_a_send() {
        let mut harness = harness(true);
        let result = run(&mut harness, "run ralph oops").await;
        assert!(message_text(&result).contains("is not key=value"));
        assert_nothing_sent(&mut harness);
    }

    #[tokio::test]
    async fn status_with_id_dispatches_inspect_op() {
        let mut harness = harness(true);
        let result = run(&mut harness, "status wf_1").await;
        assert!(matches!(result.kind, CommandResultKind::Dispatched));
        let sent = match harness.rx.try_recv() {
            Ok(command) => command,
            Err(error) => panic!("one BridgeCommand expected: {error}"),
        };
        assert!(matches!(
            sent,
            BridgeCommand::Workflow {
                op: WorkflowOp::Status { ref id },
                ..
            } if id.as_str() == "wf_1"
        ));
    }
}
