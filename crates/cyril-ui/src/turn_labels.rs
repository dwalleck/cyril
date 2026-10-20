//! Header, footer and notice text for agent-initiated turns and
//! engine-injected messages (cyril-lki9 spec B1/B3/B4/B5; repeat-cap wakes
//! cyril-6lql; credit footers cyril-2lh8).
//!
//! One pure function per concern, like `workflow_format`: `UiState` and the
//! subagent streams turn each announcement or injection into exactly one
//! system line built here. Every header/notice string cyril renders for these
//! lives in this module (module-shape fence C21 R2).

use cyril_core::types::{AgentInitiation, WorkflowRunStatus};
use cyril_core::workflow::{AgentTurnClose, CompletionNotice, StepAnswer, StepQuestion, WakeLabel};

/// The header line that opens an agent-initiated turn.
///
/// - workflow completion, run known → `─── ⚙ workflow "<name>" <status> · agent follow-up ───` (B1)
/// - workflow completion, run unknown → `─── ⚙ workflow ended · agent follow-up ───` (B4)
/// - a run paused at a repeat's iteration cap (KAS announces it as a message
///   wake, cyril-6lql) → `─── ⚙ workflow "<name>" paused · repeat "<node>"
///   reached its iteration limit (<max>) · agent follow-up ───`
/// - any other reason → `─── ⚙ agent-initiated · <reason> ───`, `unspecified`
///   when the engine gave none (B3)
#[must_use]
pub(crate) fn header_text(origin: &AgentInitiation, label: Option<&WakeLabel>) -> String {
    if let Some(label) = label
        && let Some(step) = label.asking_step()
    {
        return format!(
            "─── ⚙ workflow \"{}\" · step \"{step}\" asked a question · agent follow-up ───",
            label.name()
        );
    }
    if let Some(label) = label
        && let Some(repeat_cap) = label.repeat_cap()
    {
        return format!(
            "─── ⚙ workflow \"{}\" {} · repeat \"{}\" reached its iteration limit ({}) · agent follow-up ───",
            label.name(),
            status_word(label.status()),
            repeat_cap.node_id(),
            repeat_cap.max_iterations()
        );
    }
    if !origin.is_workflow_completion() {
        return format!(
            "─── ⚙ agent-initiated · {} ───",
            origin.reason().unwrap_or("unspecified")
        );
    }
    match label {
        Some(label) => format!(
            "─── ⚙ workflow \"{}\" {} · agent follow-up ───",
            label.name(),
            status_word(label.status())
        ),
        None => "─── ⚙ workflow ended · agent follow-up ───".to_owned(),
    }
}

/// The notice for a workflow completion KAS injected into a busy turn (B5):
/// `⚙ workflow "<name>" <status> · run so far <total> credits (noted
/// mid-turn):` — a running total, since no follow-up wake exists yet to
/// close it and the same run's later lines may show more (cyril-2lh8); the
/// segment is left out when nothing was metered — or
/// `⚙ workflow ended (noted mid-turn):` when the run is unknown — followed by
/// the engine's text verbatim on the next line(s), when it sent any.
#[must_use]
pub(crate) fn workflow_notice_text(
    notice: Option<&CompletionNotice>,
    content: Option<&str>,
) -> String {
    let head = match notice {
        Some(notice) => {
            let mut head = format!(
                "⚙ workflow \"{}\" {}",
                notice.label().name(),
                status_word(notice.label().status())
            );
            if let Some(total) = notice.run_total() {
                head.push_str(&format!(
                    " · {}",
                    run_total_text("run so far", total, notice.unmetered_turns())
                ));
            }
            head.push_str(" (noted mid-turn):");
            head
        }
        None => "⚙ workflow ended (noted mid-turn):".to_owned(),
    };
    match content.filter(|c| !c.is_empty()) {
        Some(content) => format!("{head}\n{content}"),
        None => head,
    }
}

/// The notice for a step verdict KAS relayed into a turn (B5):
/// `⚙ workflow step · <severity>: <message>`. Severity defaults to `info`.
/// The message arrives bare — the converter removes KAS's own severity
/// prefix (a dialect literal) before it reaches the UI.
#[must_use]
pub(crate) fn step_notice_text(severity: Option<&str>, content: Option<&str>) -> String {
    let severity = severity.filter(|s| !s.is_empty()).unwrap_or("info");
    format!("⚙ workflow step · {severity}: {}", content.unwrap_or(""))
}

/// The line that closes an agent-initiated turn (cyril-2lh8), shown after
/// its `turn_end`:
///
/// - the wake that follows a run's end — the run's completion line →
///   `╰─ ⚙ workflow "<name>" <status> · run <total> credits · follow-up <turn> credits`
///   (`run ≥ <total> credits (<n> turns unmetered)` when the total is partial)
/// - any other agent-initiated turn → `╰─ ⚙ agent follow-up · <turn> credits`
///
/// A figure the engine never metered is left out — never shown as 0 — and a
/// close with no figure at all is `None`: no line.
#[must_use]
pub(crate) fn footer_text(close: &AgentTurnClose) -> Option<String> {
    let follow_up = close.credits().map(credits_text);
    match (close.run(), follow_up) {
        (Some(run), follow_up) => {
            let mut text = format!(
                "╰─ ⚙ workflow \"{}\" {} · {}",
                run.label().name(),
                status_word(run.label().status()),
                run_total_text("run", run.total(), run.unmetered_turns())
            );
            if let Some(follow_up) = follow_up {
                text.push_str(&format!(" · follow-up {follow_up}"));
            }
            Some(text)
        }
        (None, Some(follow_up)) => Some(format!("╰─ ⚙ agent follow-up · {follow_up}")),
        (None, None) => None,
    }
}

/// A turn's credits, floored to two decimals as Kiro's TUI shows a turn
/// (`kiro-tui-2.32.0.js` `yae`: `(Math.floor(e*100)/100).toFixed(2)`).
fn credits_text(credits: f64) -> String {
    let floored = (credits * 100.0).floor() / 100.0;
    format!("{floored:.2} credits")
}

/// A run's credits, floored as Kiro's TUI shows a workflow's total
/// (`kiro-tui-2.32.0.js` `Gz`: `t=e*100; Math.floor(t+Number.EPSILON*
/// Math.abs(t))/100`, then `toFixed(2)`).
fn run_credits_text(credits: f64) -> String {
    let hundredths = credits * 100.0;
    let floored = (hundredths + f64::EPSILON * hundredths.abs()).floor() / 100.0;
    format!("{floored:.2} credits")
}

/// `<lead> <total> credits`, or — when some of the run's turns reported no
/// credits — the unmistakably partial `<lead> ≥ <total> credits (<n>
/// turn(s) unmetered)` (cyril-2lh8 review: an incomplete total is not a
/// total). `lead` is `run` for an ended run's completion line, `run so far`
/// for a total the run may still add to.
fn run_total_text(lead: &str, total: f64, unmetered_turns: u32) -> String {
    let credits = run_credits_text(total);
    match unmetered_turns {
        0 => format!("{lead} {credits}"),
        1 => format!("{lead} ≥ {credits} (1 turn unmetered)"),
        n => format!("{lead} ≥ {credits} ({n} turns unmetered)"),
    }
}

fn status_word(status: WorkflowRunStatus) -> &'static str {
    status.as_str()
}

/// The transcript line for a workflow step parked on a question
/// (cyril-9104 D1):
///
/// ```text
/// ⚙ workflow "<run>" · step "<node>" is waiting for your answer:
/// <what it asked>
/// ↳ /answer @<selector> <your answer>
/// ```
///
/// What it asked is the question's own text (its `send_message`, or its
/// captured output), else `fallback` — the step's latest reply from its
/// stream. With neither, the first line ends in a period and no text line.
#[must_use]
pub(crate) fn step_question_text(question: &StepQuestion, fallback: Option<&str>) -> String {
    let head = format!(
        "⚙ workflow \"{}\" · step \"{}\" is waiting for your answer",
        question.run_name(),
        question.node_id()
    );
    fn nonblank(text: Option<&str>) -> Option<&str> {
        text.map(str::trim).filter(|text| !text.is_empty())
    }
    let asked = nonblank(question.question()).or_else(|| nonblank(fallback));
    let hint = format!("↳ /answer @{} <your answer>", question.selector());
    match asked {
        Some(asked) => format!("{head}:\n{asked}\n{hint}"),
        None => format!("{head}.\n{hint}"),
    }
}

/// The line for the parent model answering a step itself (cyril-9104 D3):
/// `⚙ answered "<node>" for you: <message>`, or `⚙ answered a workflow step
/// for you: <message>` when the step is not known.
#[must_use]
pub(crate) fn parent_answer_text(answer: &StepAnswer) -> String {
    match answer.step() {
        Some(step) => format!("⚙ answered \"{step}\" for you: {}", answer.message()),
        None => format!("⚙ answered a workflow step for you: {}", answer.message()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::{WorkflowNodeId, WorkflowRepeatCap};
    use cyril_core::workflow::RunCredits;

    fn origin(reason: Option<&str>, workflow_completion: bool) -> AgentInitiation {
        AgentInitiation::new(reason.map(str::to_owned), workflow_completion)
    }

    fn label(name: &str, status: WorkflowRunStatus) -> WakeLabel {
        WakeLabel::new(name.to_owned(), status)
    }

    /// B1 / B3 / B4 exact strings (spec, approved literals).
    #[test]
    fn header_strings() {
        let wake = origin(Some("workflow-complete-wake"), true);
        assert_eq!(
            header_text(
                &wake,
                Some(&label(
                    "audit-channels-2.26.0",
                    WorkflowRunStatus::Completed
                ))
            ),
            "─── ⚙ workflow \"audit-channels-2.26.0\" completed · agent follow-up ───"
        );
        assert_eq!(
            header_text(
                &wake,
                Some(&label("«review» \"x\"", WorkflowRunStatus::Failed))
            ),
            "─── ⚙ workflow \"«review» \"x\"\" failed · agent follow-up ───",
            "unicode and quotes in a run name are shown as given"
        );
        assert_eq!(
            header_text(&wake, None),
            "─── ⚙ workflow ended · agent follow-up ───"
        );
        assert_eq!(
            header_text(&origin(Some("send-message-wake"), false), None),
            "─── ⚙ agent-initiated · send-message-wake ───"
        );
        assert_eq!(
            header_text(&origin(None, false), None),
            "─── ⚙ agent-initiated · unspecified ───"
        );
        assert_eq!(
            header_text(
                &origin(Some("send-message-wake"), false),
                Some(&label("ignored", WorkflowRunStatus::Completed))
            ),
            "─── ⚙ agent-initiated · send-message-wake ───",
            "a label never renames a non-workflow wake"
        );
    }

    fn repeat_cap(name: &str, repeat: &str, max: u32) -> WakeLabel {
        let node = WorkflowNodeId::try_from(repeat.to_owned())
            .unwrap_or_else(|e| panic!("node id fixture: {e}"));
        WakeLabel::paused_at_repeat_cap(name.to_owned(), WorkflowRepeatCap::new(node, max, max))
    }

    /// cyril-6lql: a repeat-cap wake (KAS 0.66.43 announces it as a message
    /// wake) names the run and the repeat that hit its iteration limit.
    #[rstest::rstest]
    #[case::send_message_wake(
        origin(Some("send-message-wake"), false),
        repeat_cap("loop", "loop", 1),
        "─── ⚙ workflow \"loop\" paused · repeat \"loop\" reached its iteration limit (1) · agent follow-up ───"
    )]
    #[case::reasonless_wake(
        origin(None, false),
        repeat_cap("feature-pipeline", "review-loop", 2),
        "─── ⚙ workflow \"feature-pipeline\" paused · repeat \"review-loop\" reached its iteration limit (2) · agent follow-up ───"
    )]
    fn repeat_cap_header_strings(
        #[case] origin: AgentInitiation,
        #[case] label: WakeLabel,
        #[case] want: &str,
    ) {
        assert_eq!(header_text(&origin, Some(&label)), want);
    }

    fn close(credits: Option<f64>, run: Option<(&str, WorkflowRunStatus, f64)>) -> AgentTurnClose {
        partial_close(credits, run, 0)
    }

    fn partial_close(
        credits: Option<f64>,
        run: Option<(&str, WorkflowRunStatus, f64)>,
        unmetered_turns: u32,
    ) -> AgentTurnClose {
        AgentTurnClose::new(
            credits,
            run.map(|(name, status, total)| {
                RunCredits::new(label(name, status), total).with_unmetered_turns(unmetered_turns)
            }),
        )
    }

    /// cyril-2lh8: the line that closes an agent-initiated turn shows what it
    /// cost; the wake that follows a run's end is the run's completion line
    /// and shows the run's total. Missing metering shows no figure — never 0
    /// — and a close with no figure at all renders no line.
    #[rstest::rstest]
    #[case::generic_wake(
        close(Some(0.2227906), None),
        Some("╰─ ⚙ agent follow-up · 0.22 credits")
    )]
    #[case::run_completion(
        close(Some(0.2378933), Some(("audit-linear", WorkflowRunStatus::Completed, 0.6214264))),
        Some("╰─ ⚙ workflow \"audit-linear\" completed · run 0.62 credits · follow-up 0.23 credits")
    )]
    #[case::run_completion_unmetered_wake(
        close(None, Some(("loop", WorkflowRunStatus::Aborted, 0.38))),
        Some("╰─ ⚙ workflow \"loop\" aborted · run 0.38 credits")
    )]
    #[case::partial_total_one_turn(
        partial_close(Some(0.25), Some(("audit-linear", WorkflowRunStatus::Completed, 0.62)), 1),
        Some("╰─ ⚙ workflow \"audit-linear\" completed · run ≥ 0.62 credits (1 turn unmetered) · follow-up 0.25 credits")
    )]
    #[case::partial_total_two_turns(
        partial_close(None, Some(("loop", WorkflowRunStatus::Failed, 0.5)), 2),
        Some("╰─ ⚙ workflow \"loop\" failed · run ≥ 0.50 credits (2 turns unmetered)")
    )]
    #[case::nothing_metered(close(None, None), None)]
    fn footer_strings(#[case] close: AgentTurnClose, #[case] want: Option<&str>) {
        assert_eq!(footer_text(&close).as_deref(), want);
    }

    /// cyril-2lh8 review: credits are floored to two decimals exactly as
    /// Kiro's TUI shows them (`kiro-tui-2.32.0.js`). Turn credits use `yae`,
    /// `Math.floor(e*100)/100`. Run totals use `Gz`, which adds
    /// `Number.EPSILON*|e*100|` before flooring. Hence 0.29, whose `*100`
    /// is 28.999999999999996, shows 0.28 as a turn figure and 0.29 as a
    /// run total, as it does in Kiro.
    #[rstest::rstest]
    #[case::linear_follow_up(0.237_893_331_840_796_03, "0.23", "0.23")]
    #[case::bgwatch_total(0.175_088_340_132_670_1, "0.17", "0.17")]
    #[case::repeat_cap_wake(0.222_790_630_514_096_2, "0.22", "0.22")]
    #[case::exact_hundredth(0.38, "0.38", "0.38")]
    #[case::binary_edge(0.29, "0.28", "0.29")]
    #[case::sub_hundredth(0.004, "0.00", "0.00")]
    fn credits_floor_like_kiro(#[case] credits: f64, #[case] turn: &str, #[case] run: &str) {
        assert_eq!(credits_text(credits), format!("{turn} credits"));
        assert_eq!(
            run_total_text("run", credits, 0),
            format!("run {run} credits")
        );
    }

    /// B5 exact strings for both injection kinds, with the stress rows.
    #[test]
    fn notice_strings() {
        let wake_text = "A workflow you launched (\"lki9-quick\") completed. Review its results and continue if you were waiting on it.";
        assert_eq!(
            workflow_notice_text(
                Some(&CompletionNotice::new(
                    label("lki9-quick", WorkflowRunStatus::Completed),
                    None
                )),
                Some(wake_text)
            ),
            format!("⚙ workflow \"lki9-quick\" completed (noted mid-turn):\n{wake_text}"),
            "an unmetered run shows no figure (cyril-2lh8)"
        );
        assert_eq!(
            workflow_notice_text(
                Some(&CompletionNotice::new(
                    label("lki9-quick", WorkflowRunStatus::Completed),
                    Some(0.0516190)
                )),
                Some(wake_text)
            ),
            format!(
                "⚙ workflow \"lki9-quick\" completed · run so far 0.05 credits (noted mid-turn):\n{wake_text}"
            ),
            "the run's credits so far (cyril-2lh8)"
        );
        assert_eq!(
            workflow_notice_text(
                Some(
                    &CompletionNotice::new(
                        label("lki9-quick", WorkflowRunStatus::Completed),
                        Some(0.0516190)
                    )
                    .with_unmetered_turns(1)
                ),
                None
            ),
            "⚙ workflow \"lki9-quick\" completed · run so far ≥ 0.05 credits (1 turn unmetered) (noted mid-turn):",
            "a partial total says so (cyril-2lh8 review)"
        );
        assert_eq!(
            workflow_notice_text(None, Some(wake_text)),
            format!("⚙ workflow ended (noted mid-turn):\n{wake_text}")
        );
        assert_eq!(
            workflow_notice_text(None, Some("")),
            "⚙ workflow ended (noted mid-turn):",
            "empty content adds no blank line"
        );
        assert_eq!(
            step_notice_text(Some("success"), Some("OK")),
            "⚙ workflow step · success: OK"
        );
        assert_eq!(
            step_notice_text(Some("error"), Some("plain message, no prefix")),
            "⚙ workflow step · error: plain message, no prefix"
        );
        assert_eq!(
            step_notice_text(None, Some("OK")),
            "⚙ workflow step · info: OK",
            "absent severity defaults to info"
        );
        assert_eq!(step_notice_text(Some(""), None), "⚙ workflow step · info: ");
    }

    fn node(id: &str) -> WorkflowNodeId {
        WorkflowNodeId::try_from(id.to_owned()).unwrap_or_else(|e| panic!("node id fixture: {e}"))
    }

    /// cyril-9104: the parent's wake for a step's question names the run and
    /// the step that asked.
    #[test]
    fn step_question_header_string() {
        let label =
            WakeLabel::step_question("workflow-from-prompt".to_owned(), node("create-workflow"));
        assert_eq!(
            header_text(&origin(Some("send-message-wake"), false), Some(&label)),
            "─── ⚙ workflow \"workflow-from-prompt\" · step \"create-workflow\" asked a question · agent follow-up ───"
        );
    }

    fn question(node_id: &str, text: Option<&str>) -> StepQuestion {
        use cyril_core::types::{
            SessionId, WorkflowAttention, WorkflowAttentionKind, WorkflowId, WorkflowNodePath,
        };
        let run = WorkflowId::try_from("wf_1".to_owned()).unwrap_or_else(|e| panic!("{e}"));
        let path = WorkflowNodePath::try_new(&run, vec!["wf_1".to_owned(), node_id.to_owned()])
            .unwrap_or_else(|e| panic!("{e}"));
        let attention = WorkflowAttention::new(
            WorkflowAttentionKind::Question,
            "t".to_owned(),
            node(node_id),
            path,
            SessionId::new("sess_step"),
        );
        StepQuestion::new(run, "park".to_owned(), &attention, text.map(str::to_owned))
    }

    /// cyril-9104 D1: a parked question names its run and step, shows what
    /// the step asked — the step's own reply when cyril has no other text —
    /// and how to answer it.
    #[rstest::rstest]
    #[case::send_message_text(
        Some("Which file should I edit?"),
        Some("ignored stream text"),
        "⚙ workflow \"park\" · step \"ask\" is waiting for your answer:\nWhich file should I edit?\n↳ /answer @ask <your answer>"
    )]
    #[case::stream_fallback(
        None,
        Some("WAITING"),
        "⚙ workflow \"park\" · step \"ask\" is waiting for your answer:\nWAITING\n↳ /answer @ask <your answer>"
    )]
    #[case::no_text(
        None,
        None,
        "⚙ workflow \"park\" · step \"ask\" is waiting for your answer.\n↳ /answer @ask <your answer>"
    )]
    #[case::blank_note_falls_back_to_the_reply(
        Some("  "),
        Some("WAITING"),
        "⚙ workflow \"park\" · step \"ask\" is waiting for your answer:\nWAITING\n↳ /answer @ask <your answer>"
    )]
    #[case::blank_text_is_absent(
        Some("  "),
        Some(""),
        "⚙ workflow \"park\" · step \"ask\" is waiting for your answer.\n↳ /answer @ask <your answer>"
    )]
    fn step_question_strings(
        #[case] text: Option<&str>,
        #[case] fallback: Option<&str>,
        #[case] want: &str,
    ) {
        assert_eq!(step_question_text(&question("ask", text), fallback), want);
    }

    /// cyril-9104 D3: the parent's answer to a step reads as answered for
    /// the user; an unknown step is still shown, without a name.
    #[test]
    fn parent_answer_strings() {
        assert_eq!(
            parent_answer_text(&StepAnswer::new(
                None,
                Some(node("create-workflow")),
                "Please proceed with that plan.".to_owned()
            )),
            "⚙ answered \"create-workflow\" for you: Please proceed with that plan."
        );
        assert_eq!(
            parent_answer_text(&StepAnswer::new(None, None, "Yes.".to_owned())),
            "⚙ answered a workflow step for you: Yes."
        );
    }
}
