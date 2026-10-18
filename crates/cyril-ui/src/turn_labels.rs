//! Header, footer and notice text for agent-initiated turns and
//! engine-injected messages (cyril-lki9 spec B1/B3/B4/B5; repeat-cap wakes
//! cyril-6lql; credit footers cyril-2lh8).
//!
//! One pure function per concern, like `workflow_format`: `UiState` and the
//! subagent streams turn each announcement or injection into exactly one
//! system line built here. Every header/notice string cyril renders for these
//! lives in this module (module-shape fence C21 R2).

use cyril_core::types::{AgentInitiation, WorkflowRunStatus};
use cyril_core::workflow::{AgentTurnClose, CompletionNotice, WakeLabel};

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
/// `⚙ workflow "<name>" <status> · run <total> credits (noted mid-turn):` —
/// the run's credits so far, since no follow-up wake exists to close it
/// (cyril-2lh8); the segment is left out when nothing was metered — or
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
                head.push_str(&format!(" · run {}", credits_text(total)));
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
                "╰─ ⚙ workflow \"{}\" {} · run {}",
                run.label().name(),
                status_word(run.label().status()),
                credits_text(run.total())
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

fn credits_text(credits: f64) -> String {
    format!("{credits:.2} credits")
}

fn status_word(status: WorkflowRunStatus) -> &'static str {
    status.as_str()
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
        AgentTurnClose::new(
            credits,
            run.map(|(name, status, total)| RunCredits::new(label(name, status), total)),
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
        Some("╰─ ⚙ workflow \"audit-linear\" completed · run 0.62 credits · follow-up 0.24 credits")
    )]
    #[case::run_completion_unmetered_wake(
        close(None, Some(("loop", WorkflowRunStatus::Aborted, 0.38))),
        Some("╰─ ⚙ workflow \"loop\" aborted · run 0.38 credits")
    )]
    #[case::nothing_metered(close(None, None), None)]
    fn footer_strings(#[case] close: AgentTurnClose, #[case] want: Option<&str>) {
        assert_eq!(footer_text(&close).as_deref(), want);
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
                "⚙ workflow \"lki9-quick\" completed · run 0.05 credits (noted mid-turn):\n{wake_text}"
            ),
            "the run's credits so far (cyril-2lh8)"
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
}
