//! `/answer` — reply to a KAS workflow step parked on a question
//! (cyril-9104, ADR-0013 decision 3). Registered beside `/workflow` (KAS only).
//!
//! The parked steps come from the `WorkflowTracker` (run-level `attention[]`).
//! The answer goes to the step's own session as `session/prompt`, which is
//! how KAS resumes a parked step. A step that is not parked is refused: a
//! prompt to a running step would pre-empt its turn.

use super::{Command, CommandContext, CommandResult};
use crate::types::BridgeCommand;
use crate::workflow::{StepQuestion, WorkflowTracker};

const USAGE: &str = "Usage: /answer [@<step>] <your answer> — replies to a workflow step \
     waiting for your answer; /answer alone lists them.";

/// `/answer` — answer a workflow step's question inline.
pub struct AnswerCommand;

#[async_trait::async_trait]
impl Command for AnswerCommand {
    fn name(&self) -> &str {
        "answer"
    }

    fn description(&self) -> &str {
        "Answer a workflow step that is waiting for you: /answer [@<step>] <your answer>"
    }

    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult> {
        let Some(tracker) = ctx.workflow_tracker else {
            tracing::error!("workflow tracker missing from CommandContext");
            return Ok(CommandResult::system_message(
                "Workflow state is unavailable in this build.".into(),
            ));
        };
        let parked = tracker.step_questions();
        let args = args.trim();
        if args.is_empty() {
            return Ok(CommandResult::system_message(listing(&parked)));
        }
        let (selector, answer) = match args.strip_prefix('@') {
            Some(rest) => {
                let (selector, answer) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                (Some(selector), answer.trim())
            }
            None => (None, args),
        };
        if answer.is_empty() {
            return Ok(CommandResult::system_message(USAGE.to_owned()));
        }
        let target = match selector {
            Some(selector) => {
                let matching: Vec<&StepQuestion> = parked
                    .iter()
                    .filter(|question| {
                        question.selector() == selector
                            || format!("{}/{}", question.workflow_id(), question.node_id())
                                == selector
                    })
                    .collect();
                match matching.as_slice() {
                    [one] => *one,
                    _ => {
                        return Ok(CommandResult::system_message(not_waiting(
                            tracker, selector, &parked,
                        )));
                    }
                }
            }
            None => match parked.as_slice() {
                [one] => one,
                [] => return Ok(CommandResult::system_message(listing(&parked))),
                _ => {
                    return Ok(CommandResult::system_message(format!(
                        "Several workflow steps are waiting — pick one with /answer @<step> <your answer>.\n{}",
                        listing(&parked)
                    )));
                }
            },
        };
        ctx.bridge
            .send(BridgeCommand::AnswerWorkflowStep {
                session_id: target.session_id().clone(),
                answer: answer.to_owned(),
            })
            .await?;
        Ok(CommandResult::system_message(format!(
            "Sent your answer to step \"{}\" of workflow \"{}\".",
            target.node_id(),
            target.run_name()
        )))
    }
}

/// The parked steps, one line each with its selector, run and question.
fn listing(parked: &[StepQuestion]) -> String {
    if parked.is_empty() {
        return "No workflow step is waiting for an answer.".to_owned();
    }
    let mut text = format!("Workflow steps waiting for your answer ({}):", parked.len());
    for question in parked {
        text.push_str(&format!(
            "\n  @{} — workflow \"{}\"",
            question.selector(),
            question.run_name()
        ));
        if let Some(asked) = question.question().and_then(|q| q.lines().next()) {
            text.push_str(&format!(": {}", clip(asked.trim())));
        }
    }
    text.push_str("\n(/answer @<step> <your answer> replies to one)");
    text
}

/// Why `selector` names no parked step: a step it names that is not parked
/// (with its state when the tracker knows it), or one ambiguous between runs.
fn not_waiting(tracker: &WorkflowTracker, selector: &str, parked: &[StepQuestion]) -> String {
    if parked
        .iter()
        .filter(|question| question.node_id().as_str() == selector)
        .count()
        > 1
    {
        return format!(
            "Several waiting steps are named \"{selector}\" — use its run-qualified name.\n{}",
            listing(parked)
        );
    }
    let state = tracker
        .iter()
        .flat_map(|(_, run)| run.nodes())
        .find(|(_, node)| node.node_id().as_str() == selector)
        .and_then(|(_, node)| node.status());
    match state {
        Some(status) => {
            format!("No workflow step \"{selector}\" is waiting for an answer (it is {status}).")
        }
        None => format!("No workflow step \"{selector}\" is waiting for an answer."),
    }
}

/// A question's first line, cut at 100 characters.
fn clip(text: &str) -> String {
    const LIMIT: usize = 100;
    match text.char_indices().nth(LIMIT) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandResultKind;
    use crate::session::SessionController;
    use crate::types::{
        Notification, SessionId, WorkflowAttention, WorkflowAttentionKind, WorkflowEvent,
        WorkflowId, WorkflowNodeId, WorkflowNodePath, WorkflowPaused, WorkflowRunStarted,
    };

    struct Harness {
        session: SessionController,
        tracker: WorkflowTracker,
        rx: tokio::sync::mpsc::Receiver<BridgeCommand>,
        bridge: crate::protocol::bridge::BridgeSender,
    }

    fn harness() -> Harness {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let mut session = SessionController::new();
        session.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("sess_main"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });
        Harness {
            session,
            tracker: WorkflowTracker::new(),
            rx,
            bridge: crate::protocol::bridge::BridgeSender::from_sender(tx),
        }
    }

    fn id(raw: &str) -> WorkflowId {
        WorkflowId::try_from(raw.to_owned()).unwrap_or_else(|e| panic!("fixture id: {e}"))
    }

    fn node(raw: &str) -> WorkflowNodeId {
        WorkflowNodeId::try_from(raw.to_owned()).unwrap_or_else(|e| panic!("fixture node: {e}"))
    }

    /// Opens run `run` (recipe `name`) and parks `steps` — `(node, step
    /// session, since)` — on questions.
    fn park(harness: &mut Harness, run: &str, name: &str, steps: &[(&str, &str, &str)]) {
        let workflow = id(run);
        let apply = |tracker: &mut WorkflowTracker, event| {
            if let Err(error) = tracker.apply_event(event) {
                panic!("fixture event rejected: {error}");
            }
        };
        apply(
            &mut harness.tracker,
            WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                workflow.clone(),
                name.to_owned(),
                serde_json::json!({}),
                Vec::new(),
                Some(SessionId::new("sess_main")),
            )),
        );
        let attention = steps
            .iter()
            .map(|(step, session, since)| {
                let path =
                    WorkflowNodePath::try_new(&workflow, vec![run.to_owned(), (*step).to_owned()])
                        .unwrap_or_else(|e| panic!("fixture path: {e}"));
                WorkflowAttention::new(
                    WorkflowAttentionKind::Question,
                    (*since).to_owned(),
                    node(step),
                    path,
                    SessionId::new(*session),
                )
            })
            .collect();
        apply(
            &mut harness.tracker,
            WorkflowEvent::Paused(
                WorkflowPaused::new(workflow, "waiting".to_owned()).with_attention(attention),
            ),
        );
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
        match AnswerCommand.execute(&ctx, args).await {
            Ok(result) => result,
            Err(error) => panic!("command must not error: {error}"),
        }
    }

    fn text(result: &CommandResult) -> &str {
        match &result.kind {
            CommandResultKind::SystemMessage(text) => text,
            other => panic!("expected a system message, got {other:?}"),
        }
    }

    fn sent(harness: &mut Harness) -> Option<(String, String)> {
        match harness.rx.try_recv() {
            Ok(BridgeCommand::AnswerWorkflowStep { session_id, answer }) => {
                Some((session_id.as_str().to_owned(), answer))
            }
            Ok(other) => panic!("unexpected bridge command {other:?}"),
            Err(_) => None,
        }
    }

    /// cyril-9104 D4: the one parked step is answered without a selector;
    /// the answer goes to the STEP session, verbatim.
    #[tokio::test]
    async fn the_only_parked_step_is_answered_directly() {
        let mut harness = harness();
        park(&mut harness, "wf_1", "park", &[("ask", "sess_step", "t0")]);
        let result = run(&mut harness, "Edit src/lib.rs, not main.rs").await;
        assert_eq!(
            text(&result),
            "Sent your answer to step \"ask\" of workflow \"park\"."
        );
        assert_eq!(
            sent(&mut harness),
            Some((
                "sess_step".to_owned(),
                "Edit src/lib.rs, not main.rs".to_owned()
            ))
        );
    }

    /// cyril-9104 D4: with several parked steps an unaddressed answer is not
    /// guessed — the steps are listed — and `@<selector>` picks one; a node
    /// id two runs share needs its run-qualified selector.
    #[rstest::rstest]
    #[case::unaddressed("yes", None)]
    #[case::by_node("@review yes", Some("sess_r"))]
    #[case::shared_node_needs_its_run("@ask yes", None)]
    #[case::run_qualified("@wf_2/ask yes", Some("sess_b"))]
    #[tokio::test]
    async fn several_parked_steps_need_a_selector(
        #[case] args: &str,
        #[case] want_session: Option<&str>,
    ) {
        let mut harness = harness();
        park(
            &mut harness,
            "wf_1",
            "one",
            &[("ask", "sess_a", "t0"), ("review", "sess_r", "t2")],
        );
        park(&mut harness, "wf_2", "two", &[("ask", "sess_b", "t1")]);
        let result = run(&mut harness, args).await;
        match want_session {
            Some(session) => {
                assert_eq!(
                    sent(&mut harness),
                    Some((session.to_owned(), "yes".to_owned()))
                );
            }
            None => {
                assert_eq!(sent(&mut harness), None, "nothing is sent");
                let listing = text(&result);
                for selector in ["@wf_1/ask", "@wf_2/ask", "@review"] {
                    assert!(listing.contains(selector), "{selector} listed: {listing}");
                }
            }
        }
    }

    /// cyril-9104 D4: a step that is not parked is refused with what is
    /// known about it, and nothing is sent; with nothing parked the user is
    /// told so; an empty answer gets the usage.
    #[rstest::rstest]
    #[case::nothing_parked("yes", false, "No workflow step is waiting for an answer.")]
    #[case::unknown_step(
        "@nope yes",
        true,
        "No workflow step \"nope\" is waiting for an answer."
    )]
    #[case::empty_answer("@ask", true, "Usage: /answer")]
    #[case::blank("   ", false, "No workflow step is waiting for an answer.")]
    #[tokio::test]
    async fn refusals_send_nothing(#[case] args: &str, #[case] parked: bool, #[case] want: &str) {
        let mut harness = harness();
        if parked {
            park(&mut harness, "wf_1", "park", &[("ask", "sess_step", "t0")]);
        }
        let result = run(&mut harness, args).await;
        assert!(text(&result).starts_with(want), "{}", text(&result));
        assert_eq!(sent(&mut harness), None);
    }

    /// cyril-9104 D4: once the step resumes (the agent answered it, or the
    /// user did), it is no longer parked and an answer is refused.
    #[tokio::test]
    async fn a_resumed_step_is_refused() {
        let mut harness = harness();
        park(&mut harness, "wf_1", "park", &[("ask", "sess_step", "t0")]);
        if let Err(error) = harness.tracker.apply_event(WorkflowEvent::Paused(
            WorkflowPaused::new(id("wf_1"), "resumed".to_owned()).with_attention(Vec::new()),
        )) {
            panic!("fixture event rejected: {error}");
        }
        let result = run(&mut harness, "@ask yes").await;
        assert_eq!(
            text(&result),
            "No workflow step \"ask\" is waiting for an answer."
        );
        assert_eq!(sent(&mut harness), None);
    }

    /// cyril-9104 D4: `/answer` alone lists every parked step with its run,
    /// selector and question text.
    #[tokio::test]
    async fn bare_answer_lists_the_parked_steps() {
        let mut harness = harness();
        park(&mut harness, "wf_1", "park", &[("ask", "sess_step", "t0")]);
        let result = run(&mut harness, "").await;
        assert_eq!(
            text(&result),
            "Workflow steps waiting for your answer (1):\n  @ask — workflow \"park\"\n(/answer @<step> <your answer> replies to one)"
        );
        assert_eq!(sent(&mut harness), None);
    }
}
